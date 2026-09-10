# ADR 0014: Feature dataset

## Status

Accepted — 2026-08-23 (Milestone 2 / plan Phase 8)

## Context

- Milestone 2's miners need a numeric feature matrix with per-row action labels, built from
  annotated corpora, not a bespoke per-analyzer extraction pass repeated for every miner.

- Annotation records (`annotations.jsonl`), not raw position records, are the right source:
  annotations carry engine ground truth — the game-theoretic value and the full optimal-action
  set — per state, whereas positions are raw play traces biased by whichever strategy produced
  them.

- Annotations cover each distinct state exactly once; a dataset built on canonical states can
  therefore collapse symmetric duplicates deterministically instead of re-deriving equivalence
  from move traces.

- The same input format is produced by both corpus-mode and exhaustive annotation runs, so a
  single dataset builder works unmodified for either.

## Decision

- Row unit is one non-terminal **canonical** state. Each record canonicalizes through the
  bundle's featurizer; the first record for a canonical state creates the row, later records
  for the same state only increment `occurrences` (symmetric duplicates collapse into one row).
  A mismatch between a record's stored `canonical_state` and the bundle canonicalizer is a
  precondition error.

- `legal` and `optimal` per row are canonical-frame **positions**: actions are mapped through
  `GamePrimitives::action_position` and the canonicalizing permutation. A game whose actions
  have no positions is rejected with a clear precondition error.

- Feature encoding: the featurizer's full vocabulary evaluated on the canonical state, one
  `f64` per column in vocabulary order — Bool -> 0/1, Int -> as `f64`, Float -> itself, Set ->
  its size. Column kinds (`bool`/`int`/`float`/`set`) come from the first row; a kind change in
  a later row is a precondition error.

- Action classes are every `set`-kind column, in vocabulary order. A class `c` qualifies for a
  row iff `C_legal = legal ∩ value(c)` is non-empty **and** `C_legal ⊆ optimal` — playing any
  legal member of `c` is optimal here. `label` is the qualifying class preferred by: highest
  tier first (supplied/invented over primitive), then smallest `|C_legal|` (most specific),
  then lowest vocabulary index. Rows where nothing qualifies are labeled the literal `"none"`
  and kept — they are the fallback-risk population the miner must see.

- Confound controls: every feature is mover-relative (tier 1 by construction, tier 2 by the
  game contract); `side_to_move` is both a row field and a feature column; the tier-1 `free`
  count stands in for move number in placement games (move number itself is not a column — it
  is not derivable from a state in general); the manifest reports label, value, and
  side-to-move counts so per-side imbalance is visible.

- Files: `dataset.jsonl` (one compact-JSON `DatasetRow` per line: canonical state,
  side_to_move, value, occurrences, legal, optimal, qualifying, label, values) and
  `dataset.json` (the `DatasetManifest`: columns with tier/kind/description, classes,
  row/label/value/side-to-move counts, `collapsed` = non-terminal records minus rows,
  `unlabeled`). Both are written by the `dataset` analyzer into the run directory; the `mine`
  analyzer builds the same dataset in memory through the one shared path
  (`dataset_from_context`) and never re-reads `dataset.jsonl`.

- Rendered report section: a `## Dataset` metrics table plus a `### Labels` table.

## Consequences

- Mining sees exactly one row per strategic situation — symmetry cannot double-count evidence.

- Labels are game-agnostic: class names come from the game's vocabulary, but the labeling rule
  (qualify + tier/specificity/index preference) is framework-owned, so no game-specific mining
  logic is needed.

- `"none"` rows quantify fallback exposure directly in the manifest instead of hiding it by
  dropping unlabeled positions.

- The dataset is reproducible byte-for-byte from a run directory, since both files derive
  deterministically from the same annotation records and featurizer.

- Trade-off: set-valued features flatten to sizes in the numeric matrix, losing per-class
  membership detail for the miner's numeric features — but the per-class target sets (`legal`,
  `optimal`, `qualifying`) stay available per row, so the DSL can still consult exact class
  membership at play time.
