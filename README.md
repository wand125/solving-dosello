# DOSELLO is Solved

DOSELLOの標準初期局面のゲーム理論値は **黒 +2**。最善初手は **f3-f4 / c5-c6**（180°回転対称）、確認したPVの先頭は **f3-f4 e6-f6 d7-e7** です。

独自Rust/JavaScriptエンジン、保存済み分散探索の証明木、日英の研究ページ、新規にデザインしたWASM対局デモをまとめた公開準備リポジトリです。現状は **ローカルステージングのみ**。GitHubリポジトリ作成・push・Pages公開は実施していません。

- 論文ページ（公開予定）：https://wand125.github.io/dosello/
- 対局デモ（公開予定）：https://wand125.github.io/dosello/play/
- [証明の形式と検査](proof/README.md)・[検証報告](proof/verification.txt)
- [公開準備の検査結果](VALIDATION.md)

## Build / test / play

Rust、Python 3、Node.js 22以降が必要です。外部crate・npmパッケージ依存はありません。リポジトリのルートで実行します。

```sh
export CARGO_TARGET_DIR="$PWD/rust/target"
cargo build --release --offline --manifest-path rust/Cargo.toml
cargo test --release --offline --manifest-path rust/Cargo.toml
DOSELLO_OFFLINE=1 npm test
python3 proof/verify.py
python3 -m http.server 8000 --directory docs
```

別ターミナルで `python3 test/http-smoke.py` を実行すると全静的ファイルのHTTP 200を確認できます。

`http://localhost:8000/` が研究ページ、`http://localhost:8000/play/` がデモです。AI対局（黒・白）、2人対局、各合法手の評価、ヒント、待った、新しい対局に対応します。値は**現在の手番側**から表示し、「確定」は完全値、`≤ / ≥` は証明済み境界、`≈` は推定値です。黄色は最善候補で、推定に基づく候補は最善の保証ではありません。

WASMは同梱済みです。再ビルドにはインストール済みの `wasm32-unknown-unknown` Rustターゲットを使います。

```sh
bash rust/tools/build-wasm.sh
```

Worker内の通常WASMを使い、共有メモリ・COOP/COEP・外部CDNを必要としません。GitHub Pagesには `docs/` のみで配置でき、相対パスで動きます。ルールを編集した場合も上のコマンドでデモ用コピーを更新できます。小型定石は元プロジェクトで生成済みのもの（約5.6 MB）で、巨大なジャーナル・分割ブックは含めません。公開されていない定石の全学習過程は、このセットだけから完全再現できません。証明由来の小型ブックだけなら、`rust/target/release/book_export --book exports/missing.jsonl --output exports/proof-only.json --copy exports/proof-only-copy.json` で作れます（同梱ブック全体と同一ではありません）。

## Proof and measurements

2026-10-05 13:00–23:03 JSTの保存証明を収録しています。758局面、456ジョブ（331種類の対象局面）、保存失敗0、6,736,132,485,239探索ノード、約237.34割当スレッド時間です。探索機はRyzen 9 9950X（22スレッド）、Core i9-9980HK（14）、Apple M1（6）。別のcoordinatorは探索しません。

`proof/verify.py` はRustの合法手検査とnegamax区間の再構成を行います。**葉の探索結果を独立に再求解するものではなく、保存されたsolver出力を信頼します。** 374個の未知区間の葉が残りますが、根の確定値を妨げません。生のジョブ結果は証明JSONに含まれます。ホスト名を匿名化したためファイルのハッシュは元の保存物とは異なります。

```sh
python3 proof/summarize.py
rust/target/release/game_stats 100000 5
```

10万局のランダム対局は平均24.86827配置。棋譜数の分岐数積は約10²⁴·⁴〜10²⁴·⁶の粗い推定です。局面数は5手目まで全列挙し、その後の参考外挿は信頼できる全局面数とは区別しています。[生集計と方法](docs/data/README.md)を参照してください。

対原作レベル5の194勝0分6敗（500ms/手）は依頼時に提示された成績で、生ファイルは確認できていません。別条件の保存結果191勝1分8敗（100ms/手）を同梱しています。論文のOthello比較値も依頼時提示値で、今回のオフライン作業では外部原典の再取得・照合をしていません。

## Distributed solving

まず完全にローカルな小局面テストができます。SSHを使用しません。

```sh
python3 rust/dist/selftest.py --binary rust/target/release/job
python3 rust/dist/test_dist.py
python3 rust/dist/test_tree.py
```

自身の計算機で分散実行する場合は `rust/dist/hosts.example.json` をgitignoredの `hosts.json` へコピーし、ホスト名・利用可能なメモリとスレッド数を設定します。サンプルは3台に5枠（2×11、2×7、1×6）です。

```sh
cp rust/dist/hosts.example.json rust/dist/hosts.json
# hosts.json を編集してから実行する（以下のdeployはSSH/rsyncを使用）
bash rust/dist/deploy.sh host-a
bash rust/dist/deploy.sh host-b
bash rust/dist/deploy.sh host-c
python3 rust/dist/coordinator.py --hosts rust/dist/hosts.json --dry-run
python3 rust/dist/coordinator.py --hosts rust/dist/hosts.json \
  --state rust/dist/state.json --proof rust/dist/computed-proof.json \
  --split-depth 3 --split-empties 50
python3 rust/dist/status.py --state rust/dist/state.json
```

上記は利用者向け手順で、このステージング作業では実行していません。同じstateで再開できます。coordinatorの出力先は配布証明と分けてあります。`tree.py` / `tree_status.py` は根の解決後に両色の解答木を拡張する仕組みです。パラメータは各ツールの `--help` を参照。書き出したブックや状態ファイルは公開前に別途容量・プライバシーを確認してください。

## Optional original comparison

[原作DOSELLO](https://game2.raku-watanabe.com) は第三者の作品です。原作の `game.js`、HTML、CSS、favicon、その派生ページ・CPU複製は配布しません。

任意のオラクル照合ツールは、実行時に原作の `game.js` をgitignoredの `.cache/original/` へ取得します。取得できないときは正常にSKIPします。`DOSELLO_OFFLINE=1` は取得を禁止します。公開準備時のテストはすべてこの設定で実行しました。

```sh
# ネットワークを利用する任意の照合（この準備作業では未実行）
DOSELLO_OFFLINE=0 bash rust/tools/crosscheck.sh
```

`test/original-helper.js` は取得したコードをVMで呼び出すテスト専用アダプタです。取得物は再配布・コミットしないでください。原作が変更されると照合条件も変わります。

## Layout and licensing

```text
engine/       独自JavaScriptルール・AI
rust/         Rust crate、学習済み重み、分散ツール、テスト
proof/        匿名化した証明、生ジョブ結果、検査・集計スクリプト
docs/         静的論文ページ、新規WASM対局デモ、集計データ
test/         独自コードの検査と任意の外部オラクル照合
```

独自コード・文書は [MIT](LICENSE)。著作権者は **wand125（公開前のユーザー確認待ち）** としています。差し替える場合はLICENSEの著作権行とページの表記を更新してください。原作DOSELLOの権利は原作者に帰属し、**このMITライセンスには含まれません**。原作との提携・公認を意味しません。

## English summary

The standard initial position of DOSELLO has value **+2 for Black**. The optimal first moves are **f3-f4 / c5-c6**. This staging repository contains an independent Rust/JS engine, a sanitized proof archive, a Japanese research page with an English abstract, and a new playable WASM demo. The consistency checker validates legal transitions and interval propagation but trusts the saved leaf solver results. Original third-party assets are not distributed. MIT applies to this project's own work only. This repository has been prepared locally; nothing has been published or pushed.
