# DOSELLO is Solved

The standard initial position of DOSELLO is **Black +2** with perfect play. The optimal openings are **f3-f4 / c5-c6**, related by 180° rotation. One full perfect-play line ends **Black 30–White 28**, with six isolated empty cells (25 placements, no passes).

This repository, **wand125/solving-dosello**, contains an independent Rust/JavaScript engine, a sanitized distributed-search certificate, a fully bilingual research paper and a WebAssembly analysis tool. This revision is local only; no remote repository, push or Pages deployment was performed.

- Paper: https://wand125.github.io/solving-dosello/
- Analyzer: https://wand125.github.io/solving-dosello/play/
- [Proof format and verification](proof/README.md), [saved verification report](proof/verification.txt)
- [Validation](VALIDATION.md), [measurement provenance](docs/data/README.md)

## Build, test and run locally

Use Rust, Python 3 and Node.js 22 or newer. There are no external crate or npm dependencies. Run from the repository root:

```sh
export CARGO_TARGET_DIR="$PWD/rust/target"
cargo build --release --offline --manifest-path rust/Cargo.toml
cargo test --release --offline --manifest-path rust/Cargo.toml
DOSELLO_OFFLINE=1 npm test
python3 proof/verify.py
(cd docs && python3 -m http.server 8000 --bind ::1)
```

For local use, open `http://localhost:8000/` or `http://localhost:8000/play/`. In another terminal run `python3 test/http-smoke.py` to check HTTP 200 for all assets and both routes. Optional headless browser validation uses an installed Chromium browser: `node test/browser-smoke.js /path/to/chromium http://[::1]:8000/` while the local server is running.

Both pages default to English. **EN | 日本語** switches languages, persists in localStorage and updates page language, title and description. `?lang=ja` / `?lang=en` overrides the saved preference. The English paper remains readable without JavaScript.

The analyzer uses a slim toolbar, a large SVG domino board and a dense sortable move table. It supports AI play as either color, two players, analysis without AI moves, hints, evaluation overlays, undo/redo, clickable numbered game records and sequence import/copy. Reviewing a record pauses AI; making a move or changing mode resumes the selected mode. Sequence import accepts placements such as `f3-f4 e6-f6 d7-e7`; forced passes are automatic and do not consume a placement number. **N** new, **U / ←** undo, **→** redo, **H** hint, **E** overlay; input fields retain normal typing behavior.

Values are **mover-perspective final disc differences**. Exact values, proven bounds (`≤`, `≥`, intervals) and estimates (`≈`) remain distinct. Yellow outlines mark proven best moves; otherwise the top estimate is labeled **Recommended**, without a guarantee. Current position values use the book when available. Blue outlines identify the last placement.

The bundled WASM runs in a Worker without shared memory, COOP/COEP, external dependencies or tracking. Both pages have a self-only CSP, including `wasm-unsafe-eval` for WebAssembly compilation. Inline styles are permitted only for the paper's existing stylesheet; scripts remain external and self-only. Deploy only `docs/` to Pages; all runtime asset paths are relative.

Rebuild the bundled WASM with an already installed `wasm32-unknown-unknown` Rust target:

```sh
bash rust/tools/build-wasm.sh
```

This also updates the demo rules copy. The approximately 5.6 MB compact book was previously generated; large journals and sharded books are excluded. The full book-learning process is not reproducible from this subset. A proof-only book can be regenerated with `rust/target/release/book_export --book exports/missing.jsonl --output exports/proof-only.json --copy exports/proof-only-copy.json`; it is not identical to the bundled full compact book.

## Perfect-play line

```text
f3-f4 e6-f6 d7-e7 g4-g5 b6-c6 c8-d8 g6-h6 f7-f8 c3-d3 e1-e2 c1-c2 a3-b3 h3-h4 a5-b5 b7-c7 a1-b1 g2-h2 a7-a8 a4-b4 g1-h1 f1-f2 d1-d2 a2-b2 h7-h8 g7-g8
```

The first three moves come from the distributed proof. Each later move was proven optimal for the mover on 2026-10-06 using `analyze --prove-best` with 22 threads; other optimal moves may exist. [Saved per-ply values and timings](docs/data/perfect-line.jsonl) start at zero-based ply 3 (the fourth move). The paper includes a numbered table and an interactive 25-move replay.

## Proof and measurements

The certificate records computation on October 5, 2026, 13:00–23:03 JST: 758 positions, 456 jobs covering 331 distinct targets, zero recorded failures, 6,736,132,485,239 searched nodes and about 237.34 allocated thread-hours. Search machines were Ryzen 9 9950X (22 threads), Core i9-9980HK (14) and Apple M1 (6), with a separate non-searching coordinator.

`proof/verify.py` checks legal transitions through Rust and reconstructs negamax intervals. **It trusts saved leaf solver results and does not independently re-solve them.** The 374 unknown-interval leaves do not prevent the root from being exact. Raw job results are included. Host anonymization changed the archive hash; position IDs, search values, nodes and times were preserved.

```sh
python3 proof/summarize.py
rust/target/release/game_stats 100000 5
```

The 100,000 random games average 24.86827 placements; lengths and branching factors are descriptive statistics. Positions include colors, domino pairing and side to move; ply counts forced passes. Counts are exact through ply 7, with unbiased multiplicity-corrected MC estimates for plies 8–11 (ply 11: 2.2775e12 [2.1087e12, 2.4462e12], 95% CI). At ≥12, only bounds are available because multiplicity counting is intractable. The total distinct reachable-position count is unknown: exact lower bound 132,473,902, proven combinatorial upper bound 2.2575e21 raw (5.6437e20 up to the eight D4 board symmetries). Knuth sampling of 1,000,000 complete playouts, seed 20261006, estimates all record prefixes at 1.0336e26 [1.0001e26, 1.0672e26] and completed games at 3.5573e25 [3.4409e25, 3.6736e25] (95% CI). See [final count data](docs/data/position-counts.json).

## Strength: reproduced by command

The **194–0–6** result against the original site's level-5 CPU was produced in the source project with:

```sh
match_eval 200 500 site5 4
```

This means **200 games, 500 ms/move, 4 threads, first four plies random, paired colors, no opening book**. The experiment owner confirms that the site CPU is a faithful reimplementation of the original level-5 `choose()`, cross-checked against original `game.js`. This is a reproduced-by-command result. The separately preserved [100 ms result](docs/data/match-site5-100ms.json) is 191–1–8.

The public equivalent harness uses the original CPU directly through the optional test adapter, avoiding redistribution of a CPU clone:

```sh
DOSELLO_OFFLINE=1 node test/match_eval.js 200 500 site5 4
# Optional, network-enabled original-code acquisition by the user:
DOSELLO_OFFLINE=0 node test/match_eval.js 200 500 site5 4 > match-result.json
```

It uses the native `rust/target/release/analyze` binary with the requested time and threads, no book, a fixed seed and paired openings. It skips successfully if the original code is unavailable. The original experiment's precise random stream is not archived here; timing, random choices and use of the original adapter may change the score. The full match was not rerun during this offline revision. Fixed-depth site CPU versus time-limited search is not an equal-time comparison, and match results do not prove the solved value.

## Distributed solving

Small local tests do not use SSH:

```sh
python3 rust/dist/selftest.py --binary rust/target/release/job
python3 rust/dist/test_dist.py
python3 rust/dist/test_tree.py
```

For your own machines, copy the generic five-slot configuration (2×11, 2×7, 1×6) and edit hosts, memory and thread budgets. The following deployment commands use SSH/rsync and were not run during this revision:

```sh
cp rust/dist/hosts.example.json rust/dist/hosts.json
# Edit hosts.json before deploying.
bash rust/dist/deploy.sh host-a
bash rust/dist/deploy.sh host-b
bash rust/dist/deploy.sh host-c
python3 rust/dist/coordinator.py --hosts rust/dist/hosts.json --dry-run
python3 rust/dist/coordinator.py --hosts rust/dist/hosts.json \
  --state rust/dist/state.json --proof rust/dist/computed-proof.json \
  --split-depth 3 --split-empties 50
python3 rust/dist/status.py --state rust/dist/state.json
```

Resume with the same state. Generated certificates use a different path from the shipped proof. `tree.py` / `tree_status.py` extend solution trees for both colors after solving the root; consult each tool's `--help`. Check generated books and state files for size and private configuration before publication.

## Optional original comparison and licensing

[Original DOSELLO](https://game2.raku-watanabe.com) is a third-party work. Its game code, HTML, CSS, images, favicon and CPU clone are not distributed. The test-only adapter fetches `game.js` into ignored `.cache/original/` only when online mode is allowed. `DOSELLO_OFFLINE=1` prohibits acquisition; missing original assets produce SKIP. Never commit downloaded assets. Changes to the original can affect compatibility.

```sh
# Optional network-enabled comparison, not run online in this revision:
DOSELLO_OFFLINE=0 bash rust/tools/crosscheck.sh
```

```text
engine/  Independent JavaScript rules and AI
rust/    Rust engine, inference weights, distributed tools and tests
proof/   Sanitized certificate, raw job results, verifier and summaries
docs/    Bilingual paper, analyzer and measurement data
test/    Independent tests and optional original-code comparison
```

Original code and documentation in this project use [MIT](LICENSE), with **wand125** as the current copyright identity pending publication confirmation. Original DOSELLO remains its creator's work and is outside this license. No affiliation or endorsement is implied. Othello comparison values are supplied reports attributed to the cited paper and were not fetched again during offline staging.

---

# 日本語

DOSELLOの標準初期局面のゲーム理論値は **黒 +2**。最善初手は **f3-f4 / c5-c6**（180°回転対称）、完全プレイの一例は25配置・パスなしで **黒30対白28**、孤立した空きマス6個で終局します。

独自Rust/JavaScriptエンジン、保存済み分散探索の証明木、日英の研究ページ、新規にデザインしたWASM対局デモをまとめた公開準備リポジトリです。現状は **ローカルステージングのみ**。GitHubリポジトリ作成・push・Pages公開は実施していません。

- 論文ページ（公開予定）：https://wand125.github.io/solving-dosello/
- 対局デモ（公開予定）：https://wand125.github.io/solving-dosello/play/
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
(cd docs && python3 -m http.server 8000 --bind ::1)
```

英語が既定で、右上の EN | 日本語 で切り替えます。選択は保存され、`?lang=ja` / `?lang=en` が優先します。解析専用モード、Redo、手数付き棋譜の選択、着手列の設定・コピーも利用できます。

別ターミナルで `python3 test/http-smoke.py` を実行すると全静的ファイルのHTTP 200を確認できます。

`http://localhost:8000/` が研究ページ、`http://localhost:8000/play/` がデモです。AI対局（黒・白）、2人対局、各合法手の評価、ヒント、待った、新しい対局に対応します。値は**現在の手番側**から表示し、「確定」は完全値、`≤ / ≥` は証明済み境界、`≈` は推定値です。黄色は最善候補で、推定に基づく候補は最善の保証ではありません。

WASMは同梱済みです。再ビルドにはインストール済みの `wasm32-unknown-unknown` Rustターゲットを使います。

```sh
bash rust/tools/build-wasm.sh
```

Worker内の通常WASMを使い、共有メモリ・COOP/COEP・外部CDNを必要としません。GitHub Pagesには `docs/` のみで配置でき、相対パスで動きます。ルールを編集した場合も上のコマンドでデモ用コピーを更新できます。小型定石は元プロジェクトで生成済みのもの（約5.6 MB）で、巨大なジャーナル・分割ブックは含めません。公開されていない定石の全学習過程は、このセットだけから完全再現できません。証明由来の小型ブックだけなら、`rust/target/release/book_export --book exports/missing.jsonl --output exports/proof-only.json --copy exports/proof-only-copy.json` で作れます（同梱ブック全体と同一ではありません）。

## 完全プレイの棋譜

```text
f3-f4 e6-f6 d7-e7 g4-g5 b6-c6 c8-d8 g6-h6 f7-f8 c3-d3 e1-e2 c1-c2 a3-b3 h3-h4 a5-b5 b7-c7 a1-b1 g2-h2 a7-a8 a4-b4 g1-h1 f1-f2 d1-d2 a2-b2 h7-h8 g7-g8
```

先頭3手は分散証明に由来し、以後は2026-10-06に22スレッドの `analyze --prove-best` で各着手側の最善手を証明しました。他の最善手も存在しえます。[保存した各手の値・時間](docs/data/perfect-line.jsonl)は0始まりのply 3（4手目）からです。論文には全25手の表と操作可能な盤面を掲載しています。

## Proof and measurements

2026-10-05 13:00–23:03 JSTの保存証明を収録しています。758局面、456ジョブ（331種類の対象局面）、保存失敗0、6,736,132,485,239探索ノード、約237.34割当スレッド時間です。探索機はRyzen 9 9950X（22スレッド）、Core i9-9980HK（14）、Apple M1（6）。別のcoordinatorは探索しません。

`proof/verify.py` はRustの合法手検査とnegamax区間の再構成を行います。**葉の探索結果を独立に再求解するものではなく、保存されたsolver出力を信頼します。** 374個の未知区間の葉が残りますが、根の確定値を妨げません。生のジョブ結果は証明JSONに含まれます。ホスト名を匿名化したためファイルのハッシュは元の保存物とは異なります。

```sh
python3 proof/summarize.py
rust/target/release/game_stats 100000 5
```

10万局のランダム対局は平均24.86827配置で、長さ・分岐数は記述統計です。局面には色・ドミノの組・手番を含み、手数は強制パスを数えます。0〜7手は全列挙、8〜11手は経路多重度を補正した不偏MC推定（11手：2.2775e12 [2.1087e12, 2.4462e12]、95%信頼区間）。12手以降は多重度計数が困難なため境界のみ。全到達局面数は未確定で、厳密下界132,473,902、証明済み組合せ論的上界2.2575e21（最大8つの盤面対称性D4で同一視すると5.6437e20）。100万完全対局・シード20261006のKnuth推定は全棋譜接頭辞1.0336e26 [1.0001e26, 1.0672e26]、終局棋譜3.5573e25 [3.4409e25, 3.6736e25]（95%信頼区間）。[生集計と方法](docs/data/README.md)を参照してください。

対原作レベル5の194勝0分6敗は元プロジェクトの `match_eval 200 500 site5 4` による再現結果です。200局、500ms/手、4スレッド、初めの4手をランダム化した色入替ペア対局、定石なし。相手は原作 `choose()` レベル5の忠実な再実装で、原作 `game.js` と照合済みと実験実施者が確認しています。公開版の同等ツールは `DOSELLO_OFFLINE=1 node test/match_eval.js 200 500 site5 4`。原作がない場合はSKIPします。利用者がネットワークを許可する場合だけ `DOSELLO_OFFLINE=0` で任意取得できます。今回は200局の再実行はしていません。元の乱数列は同梱しておらず、時間・乱数条件により結果は変動します。別条件の保存結果191勝1分8敗（100ms/手）を同梱しています。論文のOthello比較値も依頼時提示値で、今回のオフライン作業では外部原典の再取得・照合をしていません。

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
