# ADR 0002: Self-play experiment runner

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- No external job framework evaluated; candidate was an internal rayon-based runner against the game-agnostic `MatchEngine`/`MatchConfig{games,seed,max_plies}` traits/types.

- Throughput, 200 games/config, 3 repeats (median), master seed 20260820 (artifacts/benchmarks/selfplay-throughput.md § Games per second): depth 9 games/s — serial 977.2, threads_1 961.5, threads_2 1717.6, threads_4 3164.9, threads_8 4518.2, threads_16 5409.0 (speedup 5.54x vs serial). Depth 2: serial 32071.3 -> threads_16 116083.3 (3.62x). Nodes/s depth 9: serial 4054533 -> threads_16 22443375 (artifacts/benchmarks/selfplay-throughput.md § Nodes per second).

- Search state (`Rc`/`RefCell`-based, per-call transposition table) is constructed inside each rayon task; nothing shared across threads (artifacts/benchmarks/selfplay-throughput.md § Header, command notes).

- Per-game seed derived from (master_seed, game_index) (artifacts/benchmarks/selfplay-throughput.md § Settings).

- Serial-vs-parallel reproducibility (artifacts/reproducibility/selfplay-serial-vs-parallel.md § Result): all 15 combinations (depth {2,4,9} x threads {1,2,4,8,16}) — ordered results identical to serial: YES, hash identical per depth (e.g. depth 9: serial `0x2cabf5ad8b16976e` == parallel at every thread count).

## Decision

- Self-play/experiment runner is internal: rayon per-game tasks, no external job framework.

- Per-game seed = f(master_seed, game_index); all engine state constructed inside the task (no cross-task sharing).

- Results collected in `game_index` order regardless of thread count or completion order.

- Invariant, required and evidenced above: a parallel batch with the same seeds is byte-identical to a serial run.

## Consequences

- Lands in `src/discovery/` (Phase 4/5), built on the `MatchEngine`/`MatchConfig` traits already defined in `src/core`.

- Corpus generation and benchmark opposition (Phase 4/5) inherit the ordering and seeding rule; any future change to task granularity or seed derivation must re-run the serial-vs-parallel determinism check.

- Thread count is a performance knob only, never a correctness or reproducibility knob — CI and local runs may use different thread counts without changing output.
