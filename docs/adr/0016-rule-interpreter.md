# ADR 0016: Rule interpreter

## Status

Accepted — 2026-08-23 (Milestone 2 / plan Phase 8)

## Context

- The heuristic-rule DSL and its interpreter stub predate this milestone: `heuristic-rules`
  was registered as a strategy kind but `choose` returned `StrategyError::Unimplemented`.
  Nothing turned a decision list into moves.

- Mined and hand-written decision lists must actually play, in the same match engine and
  harness as every other strategy, and must do so game-neutrally: the interpreter cannot
  hard-code tic-tac-toe (or any game's) feature set.

- Symmetric positions must produce symmetric choices — a heuristic trained on canonical
  rows only is useless if evaluation at play time is orientation-sensitive.

## Decision

- `Featurizer<G>` is the one game-neutral way to turn a state into its feature environment.
  It always builds the mechanical tier-1 `PrimitiveFeatures` itself (the game never lists
  them), prepends a native `side_to_move` feature (index of the player to move in turn
  order), appends the game's optional supplied (tier-2) extractor, and owns the resulting
  ordered, duplicate-checked `FeatureVocabulary`. It also exposes the canonical frame:
  `canonical_frame(state) -> (canonical_state, permutation)` through the bundle's
  canonicalizer (identity when the game has none).

- Provider construction validates early: `heuristic.validate()` (references resolve, rules
  well-formed) and every NATIVE definition the heuristic carries must be a name the game's
  featurizer actually produces, else the dedicated `DslError::UnavailableFeatures(names)` —
  a heuristic mined for one game fails fast on another instead of misplaying.
  `heuristic-rules` on a bundle without a featurizer is now a clear registry error
  (`requires a feature context`); `StrategyError::Unimplemented` is no longer produced for
  this kind.

- `choose` evaluates in the CANONICAL frame: canonicalize the state, extract natives there,
  evaluate the heuristic's derived definitions over them; rule conditions and selector
  expressions all see canonical-frame values; set-valued targets map back to the raw board
  through the permutation's INVERSE, so a rule fires identically on every symmetric image of
  a position.

- Selector semantics:
  - `AnyLegal` — all legal actions.
  - `TargetIn { expr }` — the legal actions whose canonical position lies in the
    expression's set.
  - `Maximize { expr }` — apply each legal action, canonicalize the successor, evaluate the
    expression there, keep the argmax set under `f64::total_cmp`; the successor's features
    are computed relative to the NEXT mover, the opponent, so `Maximize` scores the position
    the opponent receives. This is documented, intended semantics, not a bug: a "leave the
    opponent little" expression maximizes over opponent-perspective values.

- Rule skipping: a fired condition whose selector yields no candidate (empty target
  intersection, or actions without positions) does not consume the move; evaluation falls
  through to the next rule in priority order; after the last rule, the fallback selector; a
  fallback that yields nothing degrades to a uniform seeded choice over all legal actions.

- Tie-break policy mirrors minimax (ADR 0008): exactly one candidate returns without
  touching the RNG; otherwise a uniform choice via the strategy's own `ChaCha8Rng` seeded
  from the per-player seed at construction — same seed and same position sequence yield the
  same moves; serial equals parallel.

- Error mapping: feature-evaluation failures surface as `StrategyError::Other` with context;
  empty legal-action lists as `NoLegalActions`; construction-time problems as `DslError`
  (converted by the registry). Every returned action is an element of `legal`.

## Consequences

- Mined heuristics play in the same match engine and harness as every other strategy; no
  special-casing is needed elsewhere in the pipeline.

- Canonical-frame evaluation is what lets the miner train on canonical rows only and still
  have the resulting heuristic play correctly on any orientation at match time.

- Interpreters are cheap to clone per game: the featurizer is `Arc`-shared, so spinning up
  many `heuristic-rules` strategies (e.g. across a rayon match batch) does not re-derive the
  vocabulary.

- The opponent-perspective `Maximize` convention must be kept in mind when writing
  heuristics by hand, and is also documented in the README so heuristic authors do not
  mistake it for scoring their own position.
