# ADR 0012: Strategy archive and novelty metric

## Status

Accepted — 2026-08-22 (Milestone 1 / plan Phase 7)

## Context

- `docs/plan.md` Phase 7 requires a "strategy archive with provenance and novelty metadata" so
  the strategy space produced by evaluation (and, later, discovery) can be studied. The archive
  is a store of interesting entries, not a leaderboard: it does not prune, rank or compute Elo.

- ADR 0011 fixed the `evaluate` command and the per-strategy `StrategyEvaluation` document
  (tournament result, headline metrics, agreement, `BehaviorSignature`); this ADR builds the
  archive that persists those evaluations, and does not restate ADR 0011.

- ADR 0009 fixed byte-identical corpus outputs for the same inputs; this ADR extends that rule
  to the archive files. ADR 0003 fixed `schema_version` on every persisted document, which the
  archive index and every entry carry.

- Before this phase there was no durable place to keep an evaluated strategy: every `evaluate`
  run was a disposable report, so nothing accumulated across runs and no notion of "have we seen
  something like this before" existed.

- The archive must be game-neutral (it lives in `src/discovery/`, not under `src/games/`) and
  its novelty metric must need no extra play, since re-running a strategy through the engine on
  every archive append would make appends slower as the archive grows for reasons unrelated to
  novelty itself.

## Decision

- Layout: an archive is a directory with two files. `ARCHIVE_FILE` (`archive.json`) is the
  index, pretty-printed JSON plus a trailing newline; `ARCHIVE_ENTRIES_FILE` (`entries.jsonl`)
  holds one compact-JSON `ArchiveEntry` per line, appended in `sequence` order (0-based). The
  default location is `<out>/archive` of an `evaluate` run; `--archive DIR` overrides it. Both
  types are non-generic and live in `src/io/schema.rs`:

  ```text
  <archive>/
    archive.json                 ArchiveIndex
      schema_version
      game
      entries[]                  ArchiveIndexEntry { entry_id, name, kind, sequence }
    entries.jsonl                one ArchiveEntry per line, append order == sequence
      schema_version, entry_id, sequence, name, kind
      spec: StrategySpec
      spec_hash                  config_hash(spec)
      provenance: Provenance     game, source, evaluation_id, roster_id, seed,
                                 games_per_pairing, evaluator, corpus_run_id,
                                 annotations_run_id, annotations_mode
      evaluation: StrategyEvaluation   full per-strategy result (ADR 0011)
      novelty: Novelty           method, nearest_entry_id, nearest_name,
                                 spec_distance, behavior_distance, distance, is_novel
  ```

- Entry identity and idempotence: `entry_id` is `config_hash(EntryKey { game, spec,
  evaluation_id })`, FNV-1a 64 of the entry key's compact JSON rendered as 16 hex characters
  (the same `config_hash` primitive used for `StrategySpec` hashing elsewhere). `Archive::append`
  looks up this id before writing anything; if it is already present it returns
  `Appended::Duplicate(id)` and writes nothing, so repeating the same `evaluate` command leaves
  `archive.json` and `entries.jsonl` byte-identical and `archive_entries` unchanged. A different
  evaluation configuration (roster, games, seed, annotation set, evaluator or strategy set) of
  the *same* spec produces a different `evaluation_id`, hence a distinct `entry_id` — both
  entries are kept, since the archive is a record of what was measured, not just what was
  specified.

- `Archive` API: `Archive::open(dir, game)` creates the directory and an empty index plus an
  empty `entries.jsonl` when `archive.json` is absent; when it is present, an index whose `game`
  differs from the requested game is a `CorpusError::Config` (exit 2), and the index and
  `entries.jsonl` must agree entry-for-entry (same `entry_id` and `sequence` in the same order),
  else it is an `IoError::Invalid`. `append(NewEntry { name, spec, provenance, evaluation })`
  computes `sequence`, computes `novelty` against the entries already held, rewrites
  `archive.json` in full and appends exactly one line to `entries.jsonl`. Read-only accessors:
  `get(entry_id)`, `entries()` (file order), `len()`, `is_empty()`, `index()`. Loading an archive
  with `open` and re-serializing its entries and index reproduces both files byte for byte
  (`open_append_duplicate_and_reopen_round_trip`).

- Provenance fields on every entry: `game`, `source` (`"evaluate"` now, `"discover"` in a later
  milestone), `evaluation_id` (`EvaluationReport::evaluation_id` from ADR 0011), `roster_id`,
  `seed`, `games_per_pairing`, `evaluator`, and three optionals — `corpus_run_id` (`None` in this
  milestone; no evaluation is yet mined from a corpus run), `annotations_run_id` and
  `annotations_mode` (`"corpus"` or `"exhaustive"`, present only when the evaluation used
  annotations for agreement and signature).

- Novelty method `NOVELTY_METHOD = "m1-v1"`: deterministic, serializable, game-neutral, and
  computable from the specs and any behavior signatures already on hand, so appending never
  triggers new play.

  `spec_distance(a, b) -> f64` in `[0, 1]`, symmetric, `0.0` for identical specs:

  | kind | distance |
  | --- | --- |
  | different `kind()` | `1.0` |
  | `random` vs `random` | `0.0` |
  | `minimax` | `max(\|da - db\| / max(da, db), \|ea - eb\|, 0.1 if tie_break differs else 0.0)` |
  | `heuristic-rules` | Jaccard distance `1 - \|A ∩ B\| / \|A ∪ B\|` over canonical rule strings |
  | `evolutionary` / `llm` (not constructible yet) | `0.0` against the same kind |

  For `minimax`, `da`/`ea` are depth and epsilon; e.g. depth 3 vs depth 9 at equal epsilon and
  tie-break gives `2 / 3` ≈ `0.667`. For `heuristic-rules`, the compared sets are
  `serde_json::to_string(rule)` for every rule plus one `"fallback:" + to_string(fallback)`
  entry, so a fallback-only change still moves the distance; the distance is `0.0` when both rule
  sets are empty regardless of name, since a strategy's `name` never enters `spec_distance`.

  `behavior_distance(a, b) -> Option<f64>` compares two `BehaviorSignature`s captured at a fixed
  deterministic sample of annotated positions (ADR 0011). It is `None` unless both signatures
  share the same `sample_id` — the same annotation sample (mode, run id, non-terminal count,
  stride) — since action lists from different samples are not directly comparable index for
  index. When comparable, it is the fraction of sample positions whose chosen action differs;
  one mismatch out of four sampled positions gives `0.25`.

- The `Novelty` record and the nearest-entry rule: for each existing entry `e`, define
  `d_e = behavior_distance(e).unwrap_or(spec_distance(e))` (behavior distance is used whenever
  both signatures are comparable, since it reflects observed play rather than configuration).
  The nearest entry is the one minimizing `d_e`; ties are broken by keeping the entry with the
  lowest `sequence`. `Novelty.spec_distance` is the minimum `spec_distance` over all existing
  entries; `Novelty.behavior_distance` is the minimum `behavior_distance` over entries with a
  comparable signature (`None` if no entry is comparable); `Novelty.distance =
  behavior_distance.unwrap_or(spec_distance)`; `Novelty.is_novel = distance > 0.0`. An empty
  archive is the base case: `spec_distance = 1.0`, `behavior_distance = None`, `distance = 1.0`,
  `is_novel = true` — the first entry of an empty archive is always maximally novel.

- Float formatting: `Novelty`'s floats are written with `serde_json`'s default shortest
  round-trip formatting (ADR 0013's `float_roundtrip` feature makes that read-back exact); text
  rendered for a human or for stdout (e.g. a future `novelty=` field) uses `{:.3}` fixed
  precision, matching the ADR 0010 convention for other rendered rates.

## Consequences

- Studying the strategy space is reading `entries.jsonl`: every entry is self-describing (spec,
  provenance and the full `evaluation`), so no join against another run directory is needed to
  interpret one line.

- A later `discover` stage appends through the same `Archive::append` API with
  `provenance.source = "discover"`; no archive-file format change is anticipated for that.

- Archive files are byte-deterministic for the same inputs, extending the ADR 0009 byte-identity
  rule: re-running the same `evaluate` command is a no-op append (`Appended::Duplicate`) and
  leaves both files unchanged, verified in `tests/cli_evaluate.rs`.

- `archive.json` is rewritten in full on every new entry rather than appended to; this is
  acceptable while archives stay small (Milestone 1 scope), but an append-only or sharded index
  is a possible later optimization if archives grow large.

- Novelty computation is `O(entries)` per append, since every existing entry is compared against
  the new spec (and signature, when comparable); this is fine at Milestone 1 scale but would need
  an index (e.g. nearest-neighbor structure) if archives grow into the thousands of entries.

- Evidence: `src/discovery/archive.rs` unit tests (`spec_distance_table`,
  `behavior_distance_cases`, `novelty_of_empty_archive_is_maximal`,
  `novelty_uses_behavior_distance_when_comparable`, `open_append_duplicate_and_reopen_round_trip`,
  `open_rejects_another_game`), `tests/cli_evaluate.rs` (repeat run leaves the archive
  byte-identical), and (Milestone 1 Phase 4) `artifacts/reproducibility/tournament-determinism.md`.
