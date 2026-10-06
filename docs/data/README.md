# Measurement provenance

- `computation.json`: derived by `python3 proof/summarize.py` from the sanitized certificate. Thread-hours are elapsed job wall time multiplied by allocated threads, not measured CPU time. 456 jobs cover 331 distinct position IDs. The absence of a failures list is reported as zero recorded failures, consistent with the original verification report.
- `game-stats.json`: generated with `rust/target/release/game_stats 100000 5`. Fixed xorshift seed 1940669073. Each legal move is selected uniformly (modulo the negligible PRNG modulo bias). A placement consumes two empty cells; forced passes do not increment placement depth. Branching means exclude passes and terminal positions. Samples are random paths, not uniform positions or uniform complete game records.
- `match-site5-100ms.json`: original saved result, copied unchanged. 191 wins, 1 draw, 8 losses at 100ms/move. This is a different experiment from the task-supplied 194/0/6 at 500ms/move, whose raw data was not located.

The product of the conditional mean branching factors is a heuristic scale estimate, **not an unbiased estimate of the full game tree**. `full28BranchProductLog10` multiplies all 28 means. `meanLengthBranchProductLog10` multiplies up to the floor of the mean length and uses a fractional last factor. Neither accounts exactly for early termination or branching correlations.

`distinctByDepth` is exhaustive through depth 5, with full colors, pair geometry and side to move as the key. No spatial symmetry quotient is applied. `recordsByDepth` counts all legal paths to that depth (terminal branches have no children). In later depths forced passes are normalized before expansion; depth is placement count. These are exact early-ply counts.

Two illustrative extrapolations expose model sensitivity:

1. `naivePositionExtrapolationLog10` keeps the final measured ratio of distinct positions constant up to the mean game length. This implausibly exceeds the branching-product game-record estimate and is not used as a position count.
2. `collisionAdjustedPositionProxyLog10` assumes the depth-5 ratio `distinct / records` repeats every five placements, and applies it to the typical-length branching product. It is approximately 24.12. This is an unvalidated terminal-layer proxy, **not a measured or reliable estimate of all reachable positions**. It has no claimed confidence interval and omits earlier layers and evolving collision rates.

The paper's engine speed, training MAE and parallel scaling are inherited benchmark reports checked against the source project's READMEs. Raw training data and full benchmark archives are omitted. Othello comparison numbers and search hardware model names were supplied with the task; no external paper or website was downloaded during staging.
