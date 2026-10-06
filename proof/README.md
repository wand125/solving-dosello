# Initial-position proof

`initial-proof.json` is a saved best-first distributed negamax certificate. It contains the raw JSON result from every saved solver job, not a complete trace of all searched positions.

- `nodes`: map from SHA-256 of compact, sorted-key position JSON to a node.
- A position contains an 8×8 `board` (1 Black, −1 White, 0 empty), an 8×8 `shape` (`h-left/h-right/v-top/v-bottom`, empty strings), and `turn` (1 or −1).
- Node `lower` / `upper` are inclusive terminal disc-difference bounds, **from the side-to-move perspective**. `children` groups equivalent moves by canonical child position. `info` records native inspection data. A pass switches perspective but adds no placed stones.
- `jobs` stores position ID, host capacity, attempt ID, type (`exact` or bound search), threshold, timing, and raw `result` with bounds, node count and completion. Several jobs may target one position.
- `root`, `value`, `bestMoves`, `complete` describe the solved initial position. Empty `pending` / `running` describe the saved completion state.

The JSON was serialized compactly. Only host identity strings have been replaced with generic host-a/b/c names (and numbered worker slots). Position IDs, boards, intervals, nodes, and timings are unchanged. This changes the archive's SHA-256; [verification.txt](verification.txt) identifies the sanitized archive.

## Check

From the repository root, after building the Rust binaries:

```sh
python3 proof/verify.py
python3 proof/summarize.py
```

The verifier invokes `rust/target/release/job --type inspect` on every saved position. It checks identity, board legality through the Rust inspector, every legal move and canonical successor, job-result consistency, and negamax interval propagation. Starting with job evidence and rule terminals, it reconstructs all node bounds. It requires root `[2,2]`, best moves `c5-c6 / f3-f4`, the specified reply intervals, and the PV prefix `f3-f4 e6-f6 d7-e7`. The report is overwritten only in this public copy.

Pass `--binary PATH`, `--proof PATH`, and `--report PATH` to select other local files. Run with ordinary Python (not `python -O`, which disables assertions). The program rejects optimized Python explicitly.

## Scope

The certificate has 758 nodes and 456 completed jobs over 331 distinct positions, all at 50 empties. Some leaves remain `[-64,64]`: their exact values are unnecessary for the root bound. The verifier does **not** independently re-solve leaves and does **not** validate 6.736 trillion individual search steps. It trusts the saved full-depth solver bounds. Its rule inspector uses the same Rust engine; this is a consistency check, not independent certification by a separate solver.

No network access or remote processes are needed. The actual computation's private logs, large journals and machine identities are excluded.
