# ADR 0017: Mechanical tier-1 extension

## Status

Accepted — 2026-09-10 (Milestone 3 / plan Phase 9)

## Context

- Tier-1 features (`PrimitiveFeatures`) are meant to fall out mechanically from a game's
  line structure and symmetry group, never be hand-coded. The original `new` constructor
  covers positional and symmetry-derived primitives only; it has no notion of occupancy
  counted against line structure.

- Concept induction (ADR 0018) needs a richer numeric and set-valued vocabulary to search
  over, but any addition to tier-1 must stay game-neutral: no game-specific names, no
  reliance on symmetry input, derivable purely from `lines()` and current occupancy.

- Every existing dataset, pinned fixture, and mined heuristic depends on the current
  tier-1 vocabulary staying exactly as it is unless a caller opts in.

## Decision

- `PrimitiveFeatures::new` is unchanged: same 47 definitions, same behavior, for every
  existing caller.

- `PrimitiveFeatures::extended` adds two mechanical feature families, derived only from
  `lines()` and occupancy, with no symmetry input and no game-specific naming:

  - **Family A — line-signature counts (Int).** For every signature `(a, b)` with
    `a + b <= L` (`L` = max line length) realizable by some line, in `(a` ascending,
    `b` ascending`)` order, `lines.mine{a}.theirs{b}` counts lines with exactly `a`
    positions held by the player to move and `b` held by opponents. Tic-tac-toe yields
    10 such definitions.

  - **Family B — cell-membership sets (Set).** For every signature `(a, b)` with some
    line of length `> a + b`, and every `k` in `1..=Kmax` (`Kmax` = max lines through
    any position), `cells.mine{a}.theirs{b}.ge{k}` is the set of empty positions lying on
    at least `k` lines whose signature is exactly `(a, b)`. Tic-tac-toe has 6 qualifying
    signatures times 4 values of `k`, for 24 definitions.

- All values are mover-relative ("mine" = player to move, "theirs" = opponents), matching
  every other tier-1 primitive.

- For tic-tac-toe, extended tier-1 is 47 + 34 = 81 definitions. With the 22 supplied
  tier-2 definitions and `side_to_move`, the extended play-time vocabulary is 104
  definitions; the withheld-tier-2 variant is 82.

- Play-time featurizers (both engine-bundle construction sites) always use `extended`.
  This is behavior-neutral for existing heuristics: a heuristic only evaluates the
  definitions it carries, so a larger ambient vocabulary changes nothing it can see.

- Dataset featurizers use `extended` only when induction is enabled AND
  `extended_tier1 = true` in configuration, via `FeaturizerSpec { include_supplied,
  extended }`. All pre-existing datasets and pinned outputs are byte-unchanged, since
  their featurizer specs never request the extension.

- Rationale: the two families are the mechanical closure of "count or locate lines by
  occupancy signature" — there is no other axis of information available from `lines()`
  and occupancy alone. Hand-authored tier-2 concepts such as threats and forks fall out
  extensionally (for example, one Family A member coincides with tic-tac-toe's hand
  threat count) without the framework ever naming or special-casing them. That
  coincidence is asserted only in game-side tests; core code never encodes it.

## Consequences

- The tier-1 vocabulary can grow without any change to `GamePrimitives` or per-game
  code: Family A and Family B are entirely mechanical functions of `lines()` and
  occupancy.

- Concept induction gets a strictly richer numeric and set-valued search space when
  enabled, while every existing dataset, benchmark, and mined heuristic remains
  byte-identical because `new` is untouched and dataset extension is opt-in.

- Because play-time featurizers are always extended, a heuristic mined with
  `extended_tier1 = true` and one mined without it are both playable through the same
  engine-bundle path with no branching in the interpreter.

- The mechanical rule (derive only from `lines()` and occupancy) becomes the standing
  test for any future tier-1 addition: if a candidate family needs symmetry input or a
  game-specific name to define, it belongs in tier 2, not tier 1.
