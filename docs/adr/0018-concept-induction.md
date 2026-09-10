# ADR 0018: Concept induction

## Status

Accepted — 2026-09-10 (Milestone 3 / plan Phase 9)

## Context

- Tier 3 (`Tier::Invented`) exists in the feature-tier vocabulary but nothing populates
  it: no stage searches the existing `FeatureExpr` algebra for predicates worth naming
  as concepts.

- Any search must reuse the algebra as-is — `FeatureExpr` itself must not change — and
  must produce concepts that are self-contained, named, and stackable across rounds, in
  keeping with the framework's "rule forms are core, vocabulary is game-side" split.

- The search space over `FeatureExpr` is large; a naive enumeration mixing every
  operator, every atom type, and set-valued candidates would need class-scoring
  machinery that does not yet exist, and would blur which operators actually carry the
  compression value worth measuring in v1.

## Decision

- **Grammar v1** enumerates only `Cmp`, `Count`, `And`, `Or`, `Not`, plus
  `Cmp(Gt, atomA, atomB)` behind the off-by-default `compare_atoms` flag. `Arith`,
  `SetOp`, and `Contains` remain available to hand-written definitions but are not
  enumerated: boolean structure over comparisons and counts is what carries the
  tree-compression value the probe (below) measures, and set-valued candidates would
  need their own class-scoring machinery — deferred to a v2 grammar.

- **Enumeration and atoms.** Numeric atoms are `Ref(int column)` and
  `Count(Ref(set column))`; `side_to_move` is excluded as a confound control. Predicate
  atoms are `Ref(bool column)`, which is how later rounds stack new candidates on
  already-promoted concepts.

  - Level 1: predicate atoms, then per numeric atom, thresholds drawn from distinct
    train values (first `max_thresholds`), with ops `Eq` then `Ge`. `Ge` skips the
    minimum threshold, since a `Ge`-minimum comparison is constant-true on train; the
    remaining `Ge` candidates are single-split-equivalent to `Eq` and exist mainly as
    combination material for level 2.
  - Level 2: the `beam` best level-1 survivors by train gain, then `Not` of each, and
    pairwise `And` / `Or` across them.

- **Dedupe** compares full-row bit vectors: a candidate is dropped if it is constant, a
  duplicate of an existing bool column, a duplicate of an earlier kept candidate, or a
  duplicate of a previously promoted concept. First occurrence wins, deterministically.

- **Scoring.** Information gain (base-2 entropy) is computed against both the row's
  game value and its action label; `train_gain` is the max of the two. Filters
  `min_train_gain` and `min_holdout_gain` apply, with the holdout split drawn from a
  seeded `ChaCha8Rng` shuffle at `holdout_fraction`. The shortlist is the best `top_k`
  candidates ranked by holdout gain then train gain.

- **Promotion** is a downstream probe, not a bare threshold: a single `cart` fit (never
  `linfa-trees`, which is not byte-stable) at `probe_depth`, run once over all rows
  without the trial column and once with it appended. A candidate is promoted only if
  both hold:

  - the probe's emitted heuristic actually references the trial concept, and
  - rules strictly shrink with soundness held within `soundness_tolerance`, OR rules
    tie in size with soundness improved by at least `min_soundness_gain`.

  The `cart` tie-break rule (first column wins exact ties) is what makes this a sound
  gate: an appended column can only appear in the fitted tree where it strictly
  improves a split over every existing column, so a bare copy of an existing column can
  never be promoted.

- **Rounds.** Within a round, promotion is sequential and greedy, up to `max_promoted`
  candidates. Each new round re-enumerates over the vocabulary as augmented by prior
  promotions, so later rounds can build on earlier concepts. Search stops when a round
  promotes nothing or `rounds` is reached.

- **Naming and labels.** Promoted concepts are named `concept_{n}` from a global
  1-based promotion counter, stable across runs by deterministic construction.
  Definitions are auto-generated readable strings via `FeatureDef` `Display`, e.g.:

  ```text
  concept_1 [tier-3] = (...) -- invented concept (round 1)
  ```

  An optional `label_map` TOML table attaches human-readable labels at render time
  only; it never changes the stored definition.

- **Similarity.** For each promoted concept, extensional agreement (fraction of rows
  equal after boolifying: Int/Float greater than zero, Set non-empty) is computed
  against every game-supplied tier-2 feature and reported sorted descending. This is
  informational only, never a promotion gate — re-inventing a hand-authored feature
  such as a threat or fork count is reported when it happens, not required to happen.

- **Analyzer chain and file handoffs.** `dataset` produces the feature dataset, then
  `concepts` searches and promotes, writing `concepts.json`; `mine` reads the promoted
  definitions, appends them to the vocabulary so emitted heuristics carry their own
  definitions, and writes `heuristics.json`; `vocabulary` reports given-versus-discovered
  usage fractions in `vocabulary.json`. The `discover` command forces exactly this
  relative order whenever induction is enabled, and prints one `concepts=` summary line.

- **Provenance.** `DiscoveryProvenance` gains an optional, skip-if-none `concepts`
  field: `ConceptsProvenance { params_hash, withhold_tier2, rounds_run, promoted,
  concepts, seed }`. `params_hash` hashes the effective `InductionParams` actually used
  for the run. The manifest's `config_hash` deliberately continues to hash "config as
  loaded" — CLI overrides such as `--induce` and `--withhold-tier2` are applied after
  that hash is taken, so `config_hash` reflects the file on disk, not the effective
  run. Every previously written document is byte-unchanged, and `SCHEMA_VERSION` stays
  at 1.

## Consequences

- Concept induction slots into the existing analyzer/file-handoff pattern with one new
  stage and one new provenance field; nothing about `FeatureExpr`, existing analyzers,
  or `SCHEMA_VERSION` changes.

- The cart-based promotion gate ties "is this concept worth naming" to a measurable,
  byte-stable downstream effect (smaller or more sound decision lists) rather than to
  an arbitrary gain threshold, at the cost of one extra `cart` fit per candidate round.

- Because grammar v1 omits `Arith`, `SetOp`, and `Contains` from enumeration, induced
  concepts are boolean combinations of comparisons and counts only; richer set-valued
  concepts (and the class-scoring machinery they need) are explicitly deferred, not
  precluded — hand-written definitions can still use the full algebra.

- Reporting similarity against hand-authored tier-2 features (including threat and fork
  counts) without gating on it keeps promotion criteria purely internal to soundness and
  gain, while still giving reviewers a direct signal of when the search reconstructs a
  known game concept from primitives alone.

- `config_hash` as loaded versus `params_hash` as effective is a deliberate asymmetry:
  reproducing a manifest's config_hash confirms the config file is unchanged, while
  reproducing a concepts run requires the effective params, which the dedicated
  `params_hash` field records.
