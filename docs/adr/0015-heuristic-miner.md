# ADR 0015: Heuristic miner

## Status

Accepted — 2026-08-23 (Milestone 2 / plan Phase 8)

## Context

- Mining must turn the ADR 0014 dataset into playable, human-readable ordered decision
  lists (`HeuristicStrategy`), under the project's byte-determinism constraint: identical
  inputs must produce byte-identical mined archives across runs and platforms.

- ADR 0006 (rule induction) had selected `linfa-trees` as the induction engine for the
  spike. Re-reading `linfa-trees` 0.8.1's `src/decision_trees/algorithm.rs` while building
  the real miner surfaced internals that were not visible at spike time.

- `linfa-trees`'s leaf label is `find_modal_class` over a `HashMap`: ties between equally
  frequent classes resolve in `HashMap` iteration order, which is randomized per process.
  Its Gini impurity sums `HashMap` values in the same nondeterministic iteration order, so
  exactly tied split scores can select different splits from run to run. It also fits
  thresholds with `x <= split_value` over midpoints but predicts with `x < split_value`, an
  asymmetry between train-time and predict-time semantics.

- None of this is a `linfa-trees` bug; it is undocumented behavior that conflicts with the
  project's determinism constraint, so `linfa-trees` cannot remain the default induction
  path.

## Decision

- Two induction engines sit behind one `MineParams { engine, depths, min_leaf, seed,
  holdout_fraction }`: the in-crate `cart` engine (DEFAULT) and an opt-in `linfa-trees`
  adapter, selected via `[mine] engine = "cart" | "linfa-trees"`. One candidate is produced
  per `depths` entry (`0` means unlimited depth).

- `cart` is the default because it is deterministic by construction: pure integer
  arithmetic, exact Gini gain compared as rationals via `u128` cross-multiplication,
  data-value thresholds, and first-candidate (column index, then threshold ascending)
  tie-breaks. This ADR SUPERSEDES the "default = `linfa-trees`" reading of ADR 0006 without
  editing ADR 0006's text; `linfa-trees` stays available for cross-checking and is never on
  a determinism-evidence path.

- Split search uses the single `label` column only: Gini over label indices, with `"none"`
  counted as its own class.

- Leaf LABELING is soundness-driven and shared by both engines, independent of split search:
  a leaf's class is the class (in vocabulary order, `none` excluded) that appears in the
  `qualifying` list of the most leaf rows; ties go to the lowest class index; if the maximum
  count is `0`, the leaf labels `none`. `linfa-trees`'s own `prediction()` output is
  discarded — its train rows are re-routed through the converted tree using the fit-time
  `<=` predicate (not its inconsistent `<` predict path), then relabeled by this same rule.

- Tree to ordered decision list: leaves are visited depth-first, left before right, and
  numbered by leaf ordinal `k` (1-based over all leaves). A `none` leaf emits no rule — its
  rows fall through to the fallback and are counted as `fallback_rows`. Every other leaf
  emits `rule{k}` with priority `leaves - k` and action `TargetIn { <the class feature> }`.
  The rule's condition is the conjunction of path predicates, one per split node on the root
  to leaf path:

  - `int` column: `<= v` / `> v`.
  - `set` column: the same comparison applied to `count(column)`.
  - `bool` column: `not(column)` / `column`.
  - `float` column: `Le`/`Gt` on a data value for `cart`; `Lt`/`Ge` on the midpoint for
    `linfa-trees`. Midpoints are floored to integers for `int`/`set`/`bool` columns so both
    engines emit identical thresholds on integral data.

  The fallback action is `AnyLegal`. The emitted `HeuristicStrategy` is SELF-CONTAINED: its
  `definitions` are the closure of every feature referenced by any rule, and the miner
  asserts `validate()` before returning.

- Per-rule mining evidence is recorded in the report: `support` (train rows reaching the
  leaf), `sound` / `soundness` (rows whose qualifying set contains the leaf's class),
  `label_matches`, `occurrences`, and W/D/L outcome tallies from the mover's perspective.
  Candidate naming is `mined-d{depth}-l{min_leaf}` for `cart` and
  `mined-linfa-d{depth}-l{min_leaf}` for `linfa-trees`, where `depth` is the REQUESTED depth,
  not the tree's realized depth.

- An optional seeded holdout split (`ChaCha8Rng`, shuffle, last `floor(rows * fraction)`
  rows held out) reports holdout accuracy and soundness alongside the training-set figures.

- `Provenance` gains one deliberate exception to "no new fields on existing records":
  `#[serde(default, skip_serializing_if = "Option::is_none")] discovery:
  Option<DiscoveryProvenance>`, recording experiment name, config hash, corpus run id,
  annotations mode, miner id (`cart-v1` / `linfa-trees-0.8.1`), engine/depth/min_leaf
  params, mine seed, holdout fraction, dataset tiers and row count, and candidate name —
  enough to rerun a discovery from the archive entry alone. `SCHEMA_VERSION` stays `1`: the
  field is optional and skip-if-none, so every previously written document round-trips
  byte-unchanged (verified against the Milestone 1 pinned `evaluate` hashes) and old
  documents still parse. Bumping `SCHEMA_VERSION` would have invalidated every pinned
  `run_id`.

## Consequences

- The default `cart` path is deterministic end to end, satisfying the project's
  byte-identity constraint without depending on `HashMap` iteration order anywhere on the
  evidence path.

- Cross-engine agreement between `cart` and `linfa-trees` is checkable on tie-free data,
  giving an independent correctness check on the in-crate induction logic without making
  `linfa-trees` load-bearing for reproducibility.

- Archive entries are self-sufficient for rerunning a discovery: `Provenance.discovery`
  plus the self-contained `HeuristicStrategy` mean no external mining config is required to
  reproduce a candidate.

- Shallow, low-`min_leaf` candidates (for example `depth = 0`, `min_leaf = 1`) can memorize
  the training corpus rather than generalize; this is an expected tuning trade-off exposed
  via `depths`/`min_leaf`, not a defect in the induction logic.

- Evidence: `src/discovery/tree.rs`, `src/discovery/mine.rs`, `src/io/schema.rs`.
