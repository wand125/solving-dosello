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

### How to play

Analysis is off by default: legal hints stay visible, but evaluations are hidden. Hint outlines one best move until the next move, without values. Turn Analysis on in any mode to show values, best moves, PV and sources. The preference is saved; ?analysis=1 or ?analysis=0 overrides it. Toggling keeps the game and AI search running. On phones, the same Analysis checkbox is also available beside the action buttons, with a 44px touch target.

Drag from an empty cell toward an orthogonally adjacent cell, then release to place a legal domino. The live preview uses the side’s color for a legal move and a red outline for an illegal move; releasing an illegal placement or cancelling the gesture makes no move. A single tap selects a cell; tapping a second adjacent cell remains available as a fallback. With an empty cell focused, an arrow key places in that direction if legal; Enter/Space do nothing. You can also play a move from the analysis table. Board input is enabled only for the human side (both sides in two-player mode).

On phones, New / Undo / Redo / Hint sit below the board. Settings contains mode, Analysis and help. The analysis summary shows the position value and best move or recommendation; expand **Show all moves** for the table and **Game record** for the record and sequence. Board dragging does not scroll the page; scroll from outside the board.

The analyzer uses a slim toolbar, a large SVG domino board and a dense sortable move table. It supports AI play as either color, two players, hints, optional analysis, undo/redo, clickable numbered game records and sequence import/copy. Reviewing a record pauses AI; making a move or changing mode resumes the selected mode. Sequence import accepts placements such as `f3-f4 e6-f6 d7-e7`; forced passes are automatic and do not consume a placement number. **N** new, **U / ←** undo, **→** redo, **H** hint, **A** analysis; input fields retain normal typing behavior.

The AI plays proven moves from TOOL immediately; otherwise it searches with eval3 for 3 seconds per move. Analysis-off hints also prioritize TOOL. DISPLAY supplies the analysis table, overlay and post-game review, preserving exact/bound/estimate distinctions. Each missing book falls back independently to search.

**Review** opens automatically at game end, or on demand for all moves played. With Analysis off, an ongoing game asks once before revealing answers. Book proofs appear first, then targeted WASM endgame proofs, then estimates (about 1 second per position on desktop / 0.5 seconds on phones). Exact attempts have a 2-second / 1-second position budget; timeouts retain bounds or fall back to estimates. The exact threshold defaults to 30 empty cells (26 on phones); `?reviewEmpties=24` overrides it. Review never automatically runs during play.

Each row shows best and played values from the mover’s perspective, best move(s), and disc loss: exact, proven bound, or `≈` estimate. A proven positive loss counts as a mistake. Color uses the guaranteed minimum for bounds; estimates use hatching. Side totals are proven minimum losses, excluding estimates; the first decisive mistake requires proven win/draw/loss categories before and after the move. Select a row to return **before** the move and retry, with yellow outlines even when Analysis is off; **Back to end** returns to the last position. Your moves are bold in AI games. On phones the collapsible review occupies the full width below the board. Closing it, starting a new game, importing a record, or retrying cancels the Worker. Completed and partial results are cached for the page session by D4-canonical position (including domino pairing and side) and played move; completed reviews reopen immediately.

Values are **mover-perspective final disc differences**. Exact values, proven bounds (`≤`, `≥`, intervals) and estimates (`≈`) remain distinct. Yellow outlines mark proven best moves; otherwise the top estimate is labeled **Recommended**, without a guarantee. Current position values use the book when available. Blue outlines identify the last placement.

The bundled WASM runs in a Worker without shared memory, COOP/COEP, external dependencies or tracking. Both pages have a self-only CSP, including `wasm-unsafe-eval` for WebAssembly compilation. Inline styles are permitted only for the paper's existing stylesheet; scripts remain external and self-only. Deploy only `docs/` to Pages; all runtime asset paths are relative.

Rebuild the bundled WASM with an already installed `wasm32-unknown-unknown` Rust target:

```sh
bash rust/tools/build-wasm.sh
```

This also updates the demo rules copy. The two pre-generated books total approximately 2.62 MB; large journals and sharded books are excluded. The full book-learning process is not reproducible from this subset. A proof-only book can be regenerated with `rust/target/release/book_export --book exports/missing.jsonl --output exports/proof-only.json --copy exports/proof-only-copy.json`; it is not identical to the bundled full compact book.

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

It uses the native `rust/target/release/analyze` binary with the requested time and threads, no book, a fixed seed and paired openings. It skips successfully if the original code is unavailable. The original experiment's precise random stream is not archived here; timing, random choices and use of the original adapter may change the score. The match was run on 2026-10-06. Fixed-depth site CPU versus time-limited search is not an equal-time comparison, and match results do not prove the solved value.

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

Original code and documentation in this project use [MIT](LICENSE), copyright © 2026 wand125. Original DOSELLO remains its creator's work and is outside this license. No affiliation or endorsement is implied. Othello comparison values are quoted from the cited paper (arXiv version).

---

# 日本語

DOSELLOの標準初期局面のゲーム理論値は **黒 +2**。最善初手は **f3-f4 / c5-c6**（180°回転対称）、完全プレイの一例は25配置・パスなしで **黒30対白28**、孤立した空きマス6個で終局します。

独自Rust/JavaScriptエンジン、保存済み分散探索の証明木、日英の研究ページ、新規にデザインしたWASM対局デモをまとめた公開準備リポジトリです。現状は **ローカルステージングのみ**。GitHubリポジトリ作成・push・Pages公開は実施していません。

- 論文ページ：https://wand125.github.io/solving-dosello/
- 対局デモ：https://wand125.github.io/solving-dosello/play/
- [証明の形式と検査](proof/README.md)・[検証報告](proof/verification.txt)
- [公開準備の検査結果](VALIDATION.md)

## eval3 update

The demo loads `eval3-r2.bin` (about 1.9 MB): an independent, incrementally updated linear pattern evaluator. Plain eval3 is used, with ProbCut and eval-guided ordering disabled. Proven book moves still take priority; elsewhere the AI gets 3000 ms per move. Weight-loading failure quietly falls back to the previous evaluator. The bilingual paper reports the source project's training, accuracy, speed and paired-match measurements; the demo is not a perfect player.

Native tools support `--eval eval3 --weights rust/data/eval3-r2.bin`: `analyze`, `match_eval2` (against `old` or `eval3`), `eval_metrics`, and `bench_eval3`. `train_eval3` accepts `--data FILE --output FILE`; full training labels and benchmark suites are not bundled. The native site-CPU clone is excluded; the existing optional original-code adapter remains test-only.

## eval3 更新

デモは独立した線形パターン評価 `eval3-r2.bin`（約1.9 MB）を読み込み、差分更新で評価します。ProbCutと評価値による着手順序付けは無効です。証明済み定石手を優先し、それ以外は1手3000ms。重みを読み込めない場合は従来の評価に戻ります。日英の論文に元プロジェクトの学習・精度・速度・ペア対戦の測定結果を掲載しました。デモは完全なプレイヤーではありません。

ネイティブの `analyze`、`match_eval2`（対 `old` / `eval3`）、`eval_metrics`、`bench_eval3` は `--eval eval3 --weights rust/data/eval3-r2.bin` に対応します。`train_eval3` は `--data FILE --output FILE` で学習します。学習教師全体やベンチマーク用スイート、原作CPUの複製は同梱しません。既存の原作照合アダプタはテスト専用です。

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

英語が既定で、右上の EN | 日本語 で切り替えます。選択は保存され、`?lang=ja` / `?lang=en` が優先します。解析チェックボックス、Redo、手数付き棋譜の選択、着手列の設定・コピーも利用できます。

別ターミナルで `python3 test/http-smoke.py` を実行すると全静的ファイルのHTTP 200を確認できます。

`http://localhost:8000/` が研究ページ、`http://localhost:8000/play/` がデモです。AI対局（黒・白）、2人対局、各合法手の評価、ヒント、待った、新しい対局に対応します。値は**現在の手番側**から表示し、「確定」は完全値、`≤ / ≥` は証明済み境界、`≈` は推定値です。黄色は最善候補で、推定に基づく候補は最善の保証ではありません。

WASMは同梱済みです。再ビルドにはインストール済みの `wasm32-unknown-unknown` Rustターゲットを使います。

```sh
bash rust/tools/build-wasm.sh
```

Worker内の通常WASMを使い、共有メモリ・COOP/COEP・外部CDNを必要としません。GitHub Pagesには `docs/` のみで配置でき、相対パスで動きます。ルールを編集した場合も上のコマンドでデモ用コピーを更新できます。TOOL・DISPLAY定石は元プロジェクトで生成済みのもの（合計約2.62 MB）で、巨大なジャーナル・分割ブックは含めません。公開されていない定石の全学習過程は、このセットだけから完全再現できません。証明由来の小型ブックだけなら、`rust/target/release/book_export --book exports/missing.jsonl --output exports/proof-only.json --copy exports/proof-only-copy.json` で作れます（同梱ブック全体と同一ではありません）。

### 遊び方

解析は既定でオフです。合法手の目印は表示し、評価は隠します。ヒントは次の着手まで最善候補1手だけを黄色で囲み、値は表示しません。どのモードでも解析をオンにすると評価・最善手・主変化・情報源を表示します。設定は保存され、?analysis=1 / ?analysis=0 が優先します。切り替えても対局やAI探索は継続します。スマートフォンでは設定内と操作ボタンのそばの両方に「解析」があり、44px以上のタッチ領域で操作できます。ショートカットは N 新規、U / ← 戻す、→ 進む、H ヒント、A 解析です。

空きマスから縦横に隣接するマスへドラッグし、離すと合法なドミノを置きます。プレビューは合法なら手番の色、不合法なら赤枠です。不合法な位置で離す、または操作をキャンセルすると着手しません。1回のタップはマスの選択のみで、隣接する2マスを順にタップする代替操作も使えます。空きマスにフォーカスして矢印キーを押すと、その方向の合法手を置きます。Enter/Spaceは何もしません。解析表の手を選んでも着手できます。盤面入力は人間の手番のみ有効です（2人対局では両色）。

スマートフォンでは盤面の下に「新規・戻す・進む・ヒント」を配置しています。「設定」でモード、解析、ヘルプを開きます。解析は局面評価と最善手・推奨手を表示し、「すべての手を表示」で表を、「棋譜」で着手履歴と着手列を開けます。盤面のドラッグではページをスクロールしないため、スクロールは盤面の外で行ってください。

AIは定石の外では1手3秒考えます。証明済みの定石手は即座に指します。

「振り返り」は終局時に自動で開き、対局途中でもボタンから開けます。解析オフの対局途中では、最善手を表示する前に一度確認します。定石の証明を先に表示し、次にWASMの終盤求解、最後に推定（PCでは1局面約1秒、スマートフォンでは約0.5秒）を計算します。終盤求解の上限時間は1局面2秒／1秒で、時間切れなら境界値を保持するか推定に移ります。求解対象は空き30マス以下（スマートフォンは26以下）で、`?reviewEmpties=24` で変更できます。対局中に自動実行はしません。

各行には手番側から見た最善値・着手の値・最善手・損失を表示し、確定・証明済み区間・推定（≈）を区別します。正の損失を証明できた手をミスと数え、境界値の色は保証された最小損失を使い、推定は斜線で区別します。色別合計は推定を除く証明済み最小損失、最初の勝敗変化は着手前後の証明済みの勝ち／引き分け／負けから判定します。行を選ぶと着手前に戻って再挑戦でき、解析オフでも黄色の候補を表示します。「棋譜の最後へ」で戻れます。AI対局の自分の手は太字です。スマートフォンでは盤面下の全幅の折り畳み欄になります。閉じる・新規対局・棋譜読み込み・再挑戦で計算を中止します。結果は盤面対称性・ドミノの組・手番を含む局面と着手をキーにページ内で保存し、完了した振り返りは即座に再表示します。

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

対原作レベル5の194勝0分6敗は元プロジェクトの `match_eval 200 500 site5 4` による再現結果です。200局、500ms/手、4スレッド、初めの4手をランダム化した色入替ペア対局、定石なし。相手は原作 `choose()` レベル5の忠実な再実装で、原作 `game.js` と照合済みと実験実施者が確認しています。公開版の同等ツールは `DOSELLO_OFFLINE=1 node test/match_eval.js 200 500 site5 4`。原作がない場合はSKIPします。利用者がネットワークを許可する場合だけ `DOSELLO_OFFLINE=0` で任意取得できます。今回は200局の再実行はしていません。元の乱数列は同梱しておらず、時間・乱数条件により結果は変動します。別条件の保存結果191勝1分8敗（100ms/手）を同梱しています。オセロの比較値は引用論文（arXiv版）から引用しています。

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

独自コード・文書は [MIT](LICENSE)。著作権者は wand125 です。原作DOSELLOの権利は原作者に帰属し、**このMITライセンスには含まれません**。原作との提携・公認を意味しません。

## Split books

`docs/play/wasm/tool-book.bin` (DSTOOL01, 516,576 bytes) holds **14,349 proven decision positions**, including adversarial hardening additions. `display-book.bin` (canonical DSBOOK03, 2,229,033 bytes) holds **15,808 analysis positions**; estimates were recomputed with eval3 at 1 s/position while exact values and bounds were retained. Both loaders restore moves from canonical symmetry to the current board. The old `opening-book.bin` is no longer used or shipped.

The [bilingual paper](docs/index.html#computation) gives counts by empty cells, collection/proof methodology and the supplied 400-game deviation benchmark: with TOOL **381–2–17 (95.5% score)**, without **369–5–26 (92.9%)**, against the previous AI at 3 s/move. These source-project measurements were not rerun here and are not a fresh benchmark of the final hardened file. Full collection journals are not included. Rust's `tool_book_build --input proof-book.bin --output tool-book.bin` validates DSBOOK02/03 proof coverage and emits proven-only DSTOOL01; it cannot reproduce the full collection from this repository alone.

TOOLは追加強化を含む証明済み14,349局面で、AIと解析オフ時のヒントに使用します。未収録・読込失敗時のAIはeval3で3秒探索します。DISPLAYは解析・盤面表示・対局後レビュー用の15,808局面で、推定値をeval3の1秒/局面で更新し、確定値・境界は保持しています。DISPLAYの読込失敗時も探索で動作します。空き数別集計・証明方法・提供対戦成績は日英論文に掲載しました。提供成績は今回の再測定ではなく、同梱の最終強化版の新たな評価でもありません。
