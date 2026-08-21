# ADR 0001: game-player minimax adapter

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- Repo owner also authors `game-player`; project already git-pins it (`https://github.com/jambolo/game-player.git`, commit `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`, crate version 0.4.0).

- `game_player::minimax::search` returns only `Option<S::Action>` (artifacts/benchmarks/annotation-per-position.md § Capability findings):

- Per-position value exposed: NO (artifacts/benchmarks/annotation-per-position.md § Capability findings)
- Best-move set exposed: NO (artifacts/benchmarks/annotation-per-position.md § Capability findings)
- Tie enumeration exposed: NO (artifacts/benchmarks/annotation-per-position.md § Capability findings)

- Value, best-move set, and tie enumeration live on the private `Response` type and inside the private `search_recursive` function; ties are resolved by stable sort order, not exposed to callers.

- Per-position search timing, depth 9, all 5478 reachable positions (artifacts/benchmarks/annotation-per-position.md § Per-position search timing): mean 6.5 us, median 0.6 us, p99 98.4 us, total 35.7 ms. Canonical 765 subset: mean 10.4 us, total 8.0 ms.

- Framework-side exhaustive solver (spike-local negamax with memoization over `GameRules`, artifacts/benchmarks/annotation-per-position.md § Exhaustive solver): 5478 positions solved in 3.5 ms; labels all_positions X wins 2936 / draw 1068 / O wins 1474.

- Cross-check (artifacts/benchmarks/annotation-per-position.md § Cross-check): game-player's returned move is in the solver's optimal set for every non-terminal position — 0 disagreements at depth 9, 0 at depth 10.

- Determinism (artifacts/reproducibility/annotation-determinism.md § Result): 2 full annotation runs byte-identical YES, fnv1a64 `0x03f7b8ede193af19` both runs.

- Plan "Further Considerations" item 4: exhaustive analysis is a tic-tac-toe special case (~5,478 positions); exhaustive modes must be flagged as small-game accelerants so no discovery method silently depends on enumeration larger games cannot provide.

## Decision

- `game-player` minimax is the settled search engine for three roles:

  - corpus generation
  - benchmark opposition
  - per-position annotation (move selection only, cross-checked — see below)

- Phase 5 annotation approach: framework-side exhaustive solver (memoized negamax over GameRules), cross-checked against game-player search; flagged as a small-game accelerant.

- MCTS (`game_player::mcts`) not evaluated; out of scope for this ADR.

- Upstream `game-player` API addition (value + best-move set) documented as a future option, not pursued now (repo owner also authors `game-player`).

## Consequences

- Phase 4 lands the `game-player` adapter (`ResponseGenerator`/`StaticEvaluator`) in `src/strategy/`; `State` impl already exists in `src/games/tictactoe/engine.rs`.

- Phase 5 lands the exhaustive solver in `src/discovery/`, keyed by canonical state, producing game-theoretic value (win/draw/loss for side to move) plus the full optimal-action set.

- Phase 5 annotation must cross-check game-player's `search` move against the solver's optimal set (per this ADR's evidence, expect 0 disagreements) rather than trust `search` alone for value/best-move data.

- Larger games than tic-tac-toe cannot use the exhaustive solver; the documented path is an upstream `game-player` API addition (value + best set) or a depth-limited solver — deferred until a second game is added.

- MCTS remains unevaluated; revisit only if a future game or phase needs it.
