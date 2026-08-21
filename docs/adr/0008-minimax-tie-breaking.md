# ADR 0008: Minimax tie-breaking by principal-variation replay

## Status

Accepted — 2026-08-21 (Phase 4)

## Context

- `game_player::minimax::search(sef, rg, state, max_depth) -> Option<Action>` (pin `0b8ce6f`, crate 0.4.0; ADR 0001) exposes only the chosen action — no root value, no best-move set, no tie enumeration — and is called from `src/strategy/minimax.rs`.

- `search`'s internal tie-break is "highest static value, then generation order" (ADR 0001, Context): deterministic but unseedable; value, best-move set and tie enumeration live on `game-player`'s private `Response` type and inside its private `search_recursive`, not exposed to callers.

- `docs/plan.md` Phase 4 requires seeded tie-breaking among equal-valued root moves so perfect-vs-perfect play varies across seeds — a corpus of identical perfect-play draws is a failure — and a 32-seed draw check.

- ADR 0002 requires per-game and per-player seeds to be derived from a master seed by a documented formula, with parallel batches byte-identical to serial runs; this ADR records that formula as implemented in `src/discovery/match_engine.rs`, whose module doc already cross-references ADR 0002.

## Decision

- `src/strategy/minimax.rs::root_values(rules, evaluator, state, legal, depth) -> Vec<(Action, f32)>` recovers the Alice-perspective depth-`depth` value of every root action by principal-variation replay: for each legal `a`, `child = apply(state, a)`, then repeatedly `a' = search(child, k)` and `child = apply(child, a')` with `k` decreasing from `depth - 1` to `0` (or until `child` is terminal), finally returning the static (terminal-aware) evaluation of the resulting leaf.

- Rationale (source doc comment, `src/strategy/minimax.rs`): `search(s, k)`'s chosen move has a depth-`(k - 1)` subtree value equal to `s`'s depth-`k` value, so replaying that move to a leaf and evaluating it recovers the depth-`k` minimax value of the root action's child.

- `TieBreak` enum (`src/strategy/minimax.rs`, `#[serde(rename_all = "kebab-case")]`): `Engine` defers to `search`'s own choice, touches no RNG, and is kept for annotation cross-checks against ADR 0001's adapter; `SeededUniform` (`#[default]`) collects every root action whose `root_values` entry equals the best value (max for Alice, min for Bob; compared with `f32::total_cmp`) and draws uniformly from that tie set using the strategy's `ChaCha8Rng` (one draw).

- Epsilon-random play (`MinimaxConfig::epsilon`) is rolled before the search on every ply, so RNG consumption per ply is fixed regardless of tie-break outcome; a single legal action is returned directly, without touching the RNG.

- Engine glue lives in `src/strategy/engine.rs`: `pub const WIN_VALUE: f32 = 100.0`, Alice win `+100`, Bob win `-100`, draw `0.0`; non-terminal heuristic values are clamped to `[-99, 99]` (`WIN_VALUE - 1.0`) so a heuristic can never masquerade as a terminal result; `AliceEvaluator` lets the framework own terminal values while a game's `StateEvaluator` supplies only the non-terminal heuristic; `RulesResponseGenerator::generate` is exactly `GameRules::legal_actions`, empty iff the state is terminal.

- `trait EngineGame` (`player_id`, `player_from_id`, `winner`) is the only game-side glue `game-player` needs; tic-tac-toe implements it in `src/games/tictactoe/engine.rs` as `X` ↔ `Alice`, `O` ↔ `Bob` (`X` is the maximizing side).

- Seed derivation lives in `src/discovery/match_engine.rs` (ADR 0002 cross-reference — the formula ADR 0002 required a documented derivation for):

  ```text
  splitmix64(x):
      z = x.wrapping_add(0x9E3779B97F4A7C15)
      z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9)
      z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB)
      return z ^ (z >> 31)

  game_seed(master, i) = splitmix64(master ^ (i as u64).wrapping_mul(0x9E3779B97F4A7C15))

  player_seed(game_seed, slot) = splitmix64(game_seed ^ (slot as u64 + 1).wrapping_mul(0xD1B54A32D192ED03))
  ```

  `slot` is the player's index in the `players` slice passed to `MatchEngine::run`; `MatchRecord.seed` stores the game seed (`game_seed`), not any player seed; every RNG on this path is `ChaCha8Rng::seed_from_u64`.

## Consequences

- Evidence (Phase 1 unit tests, `src/strategy/minimax.rs`; exhaustive checks land in `tests/minimax_tactics.rs`): at depth 9 the engine's own move lies in the `SeededUniform` tie set, and the tie set is a subset of the exhaustive solver's optimal set for every non-terminal tic-tac-toe position (0 disagreements at decomposition time); alpha-beta transposition-table bounds did not perturb move optimality (`artifacts/benchmarks/annotation-per-position.md`).

- Cost per move is `b · (d - 1)` extra searches of decreasing depth (`b` = branching factor) — negligible for tic-tac-toe (whole 5478-position sweep ≈1.4 s in a debug build), but quadratic-ish for larger games.

- Documented alternative: an upstream `game-player` API returning root candidate values (the repo owner also authors `game-player`), which would plug in behind the same `root_values` signature / `TieBreak` seam without touching callers — not pursued in Phase 4, pin unchanged (ADR 0001).

- `root_values` is `pub` because Phase 5 annotation and Phase 7 agreement metrics reuse it (`src/strategy/minimax.rs` doc comment).

- `TieBreak::Engine` remains available so annotation can cross-check against the raw engine, mirroring ADR 0001's search-vs-solver cross-check.

- Guarded by `src/strategy/minimax.rs` tests `engine_tie_break_matches_search_on_fixtures`, `seeded_uniform_picks_only_best_valued_actions`, `seeded_uniform_is_reproducible_and_varies_across_seeds`, and `perfect_self_play_draws_for_a_few_seeds`; any change to `root_values` or `TieBreak` must keep these green.
