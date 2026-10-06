# Local validation — 2026-10-06

Revision of **wand125/solving-dosello**, intended Pages root `https://wand125.github.io/solving-dosello/` and analyzer `/solving-dosello/play/`. No remote repository, push, deployment, external download or distributed solve was performed. The source project was not modified or used to supply files for this revision. Only the requested local Python server and local browser communicated over loopback.

## Checks performed for this revision

| Check | Result |
|---|---|
| `CARGO_TARGET_DIR="$PWD/rust/target" cargo build --release --offline --manifest-path rust/Cargo.toml` | PASS |
| Same target directory, `cargo test --release --offline --manifest-path rust/Cargo.toml` | 39 passed, 0 failed, 25 test targets |
| `DOSELLO_OFFLINE=1 npm test` | 15 passed, 2 optional original-dependent tests skipped, 0 failed |
| DOM-less localization checks | Identical English/Japanese key sets, every HTML dictionary binding present; URL parameter overrides storage, then English default |
| Paper translations | Every paragraph, section heading, table heading, caption and reference annotation has Japanese text; default visible article has no Japanese prose |
| Sequence and value semantics | Canonical import, invalid-ply rejection, complete review timeline, exact/bound/estimate separation; missing evaluations cannot prove a best move |
| Bundled WASM smoke | Legal no-book initial search; all 20 initial moves; initial book exact +2 and f3-f4/c5-c6 exact +2; e6-f6 reply −2 from White’s perspective |
| Rules and certificate book coverage | WASM/JS agree through 12 seeded complete games; all 758 certificate positions retain their bounds |
| `python3 proof/verify.py` | PASS: 758 positions, 760 child groups, 770 legal transitions, 456 jobs, root [2,2], both optimal openings and first 3 PV moves |
| Optional original tools | crosscheck, perft, generator and `match_eval.js` exit successfully with explicit SKIP in offline mode |
| Static HTTP | Python `http.server` started from `docs/`, bound only to IPv6 loopback; 20 assets and both page routes return 200 |
| Real Chromium smoke | PASS: Worker/WASM under CSP, 20 rows, book +2, both yellow best rows and board outlines, both AI colors, import, record jumps, undo/redo including keyboard redo |
| Language in Chromium | English/Japanese paper and analyzer, translated PV controls/caption, shared preference persistence, URL override, localized document title |
| Default without JavaScript | English paper readable with page script execution disabled |
| Responsive layout | At 390 px, no horizontal overflow and analysis panel below board |
| Browser errors | No page exceptions or CSP errors in smoke test |
| Privacy / runtime URLs | Whole-directory text/binary marker scan clean, including hidden files after removing generated build targets; runtime URL scan finds only optional original-code download |
| Whitespace | `git diff --check` clean |

Browser smoke command (local server must already be running):

```sh
(cd docs && python3 -m http.server 8000 --bind ::1)
python3 test/http-smoke.py
node test/browser-smoke.js /path/to/chromium http://[::1]:8000/
```

The browser harness uses a fresh temporary profile, blocks external hostname resolution, disables background networking and routes non-loopback traffic to a closed loopback proxy. No screenshots are required. The server and browser need loopback access, which was blocked by the default execution sandbox; the same local checks succeeded with that restriction lifted.

## UI and security

The analyzer now has a compact system-font toolbar, large gray SVG domino board, last-move outline, score/turn line and a sortable monospace Move / Value / Kind / Depth / PV table. Yellow outlines identify proven best moves, or recommendations when proof is unavailable. The panel also provides current position value, a numbered clickable game record and sequence import/copy. A bottom status bar reports engine state, source, nodes, depth and time. Record review pauses AI. Undo preserves the forward record; a new branch replaces its future. Help documents keyboard shortcuts.

Both pages default to English and use the shared language controller. The paper policy is:

```text
default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'
```

The analyzer uses the same policy with the stricter `style-src 'self'`. Inline styles remain permitted only for the paper stylesheet. JavaScript is external and same-origin. WebAssembly compilation and same-origin module Workers were exercised in Chromium. The paper's English content is ordinary HTML; its Japanese text is stored in local data attributes. No CDN, external font or tracking is used.

## Strength provenance and limits

**194–0–6 is a reproduced-by-command result:** the experiment owner confirms the source-project command `match_eval 200 500 site5 4` (200 games, 500 ms/move, 4 threads, four random opening plies, paired colors, no book). Its level-5 CPU faithfully reimplements original `choose()` and was cross-checked against original `game.js`.

The public equivalent is `node test/match_eval.js 200 500 site5 4`. It uses native `analyze` and the optional original-code adapter directly; original assets or a CPU clone are not redistributed. The full match was not rerun here, and its online adapter path could not be exercised without the optional original assets. The original experiment's random stream is not archived; randomization and wall-clock budgets can change the score. The separately preserved 100 ms experiment remains 191–1–8.

The certificate verifier trusts saved leaf solver results; it is not independent leaf re-solving. Its 374 unknown leaves do not affect the root interval. Othello reference figures were not rechecked externally. The shipped WASM and compact book were exercised but not rebuilt because their Rust source did not change. Browser checks are smoke coverage, not an exhaustive browser matrix. Copyright identity confirmation remains a publication decision; no publication was attempted.

Build targets were created inside the gitignored `rust/target/` and removed after validation. The Cargo lockfile was restored to its original contents. No private build paths are shipped.

## Size and final scan

The public working files total approximately **23.35 MB** (decimal); `docs/` is **10.79 MB**. The paper HTML is **40,960 bytes**, play HTML **3,123 bytes**, analyzer CSS **3,182 bytes**, WASM **4,986,884 bytes**, and compact book **5,713,710 bytes**. The largest file remains the approximately **7.27 MB** certificate; no file exceeds 50 MB. Native build products are not included in these totals.

A final case-insensitive `rg -a --hidden --no-ignore` scan over the entire directory (including binary bytes and Git metadata) found **zero matches** for the requested private path, person, machine, VPN/domain, private IPv4 and email markers. The local-host token appears only in README and paper instructions for running locally. The runtime source URL scan finds only the existing optional original-game download URL; paper reference links are intentional. Repository and Pages references use `solving-dosello` throughout.
