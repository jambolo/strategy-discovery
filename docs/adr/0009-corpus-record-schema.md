# ADR 0009: Corpus record schema and run directory layout

## Status

Accepted — 2026-08-21 (Phase 5)

## Context

- `docs/plan.md` Phase 5 step 2 requires a versioned position-level record (state plus canonical form, legal moves, chosen move; per-game strategy params, seeds, config hash, outcome, length) and states plainly that "this schema is the contract the Phase 8-9 miners consume — metadata plus move list alone is not sufficient." This ADR records that schema, and the run-directory layout Phase 5 steps 3-5 build on it, as implemented in `src/io/schema.rs`, `src/discovery/corpus.rs`, `src/discovery/annotate.rs`, `src/discovery/summary.rs`.

- ADR 0001 established the framework-side exhaustive solver cross-checked against `game-player`'s `search`; this ADR records how that cross-check is persisted (`AnnotationRecord`) and the solver's small-game limit, per `docs/plan.md` § Further Considerations item 4.

- ADR 0002 requires serial and parallel batches to be byte-identical and per-game/per-player seeds derived by a documented formula; ADR 0008 added the tie-break seed formulas on top. This ADR adds the cell-level and opening-ply seed formulas that sit above them in the derivation chain, and extends the byte-identity requirement from RNG consumption to every persisted file.

- ADR 0003 requires `schema_version` on every persisted document and TOML-in/JSON-out; ADR 0004 fixed the summary aggregator's shape; ADR 0007 selected JSONL as the only corpus record format, Parquet deferred. This ADR is the concrete schema those three decisions were made in service of.

- Records deliberately carry no feature vectors: `docs/plan.md`'s open, tiered feature vocabulary (tier 1 mechanical primitives, tier 2 hand-supplied, tier 3 runtime-invented) is layered on top of `state` at mining time (Phase 8), not baked into the corpus format.

## Decision

- Schema constants (`src/io/schema.rs`): `pub const SCHEMA_VERSION: u32 = 1`; file names `RUN_FILE = "run.json"`, `GAMES_FILE = "games.jsonl"`, `POSITIONS_FILE = "positions.jsonl"`, `ANNOTATIONS_FILE = "annotations.jsonl"`, `ANNOTATE_FILE = "annotate.json"`, `SUMMARY_FILE = "summary.json"`.

- `CorpusGame` (`src/io/schema.rs`): a blanket trait over `GameDomain` requiring `State`, `Action`, `Player`, `Outcome` to be `Serialize + DeserializeOwned`; `io` and `discovery` code is generic over `G: EngineGame + CorpusGame`, so a new game becomes persistable with no per-game code (`TicTacToe: CorpusGame` holds via the blanket impl; no impl in the game module). `StrategySpec` (`src/strategy/registry.rs`) is embedded in game records so each record is self-contained.

- Record types (`src/io/schema.rs`; all carry `schema_version`):

  - `CellKey { index, strategies: Vec<String> (aligned to GameBundle.players), evaluator, opening, random_opening_plies }` — one sweep cell = pairing x evaluator x opening x random-opening-plies; enumeration order is pairings -> evaluators -> openings -> random_opening_plies (outer to inner), `index` assigned from 0 in that order.
  - `GameRecord<S, A, P, O> { schema_version, run_id, game_id = "{cell.index}:{game_index}", cell: CellKey, game_index, seed, players, specs: Vec<StrategySpec>, opening_plies, actions, outcome: Option<O>, length, final_state, final_canonical_state }`; `outcome` is `None` iff the game was aborted by `max_plies`; `seed` is the game seed (formula below).
  - `PositionRecord<S, A, P, O> { schema_version, game_id, ply, side_to_move, state, canonical_state, canonical_transform: Vec<usize>, legal_actions, chosen_action, chosen_by, outcome, game_length }`; `ply` is 0-based and `state` is the position BEFORE `chosen_action`; `canonical_state` equals `state` when the game has no canonicalizer; `canonical_transform` is the permutation mapping `state` to `canonical_state` (empty means identity/none); `legal_actions` is in rules order; `chosen_by` is `"opening"`, `"random-opening"`, or the strategy entry name of the side to move; `outcome` is the game's outcome, denormalized onto every position for flat mining.
  - `AnnotationRecord<S, A, P> { schema_version, state, canonical_state, side_to_move, terminal, value: i8, optimal_actions, engine_action: Option<A>, engine_agrees: Option<bool> }`; `state` (raw, not canonical) is the key, so `optimal_actions` never need a transform; `canonical_state` is carried only for joins; `value` is from the side-to-move's perspective (1 win / 0 draw / -1 loss, exhaustive solver); `optimal_actions` is every legal action achieving `value`, in legal order, empty iff terminal; `engine_action` is `game-player` search at `engine_depth`, `None` iff terminal; `engine_agrees` is `Some(optimal_actions.contains(engine_action))` iff non-terminal.
  - No record type carries a feature vector; Phase 8 derives features from `state`.

- Documents:

  - `run.json` = `RunMetadata<A, P>` (`src/discovery/corpus.rs`): `schema_version, run_id, config_hash (== run_id), crate_name, crate_version, game, players, config (the RESOLVED GenerateConfig, defaults filled in), cells: Vec<CellKey>, games, positions`.
  - `annotate.json` = `AnnotateMetadata` (`src/discovery/annotate.rs`): `schema_version, mode (corpus | exhaustive), game, run_id (null in exhaustive mode), engine_depth, evaluator, annotated, terminal, disagreements, solver_states`.
  - `summary.json` = `CorpusSummary` (`src/discovery/summary.rs`): counts (`games, positions, distinct_games, distinct_game_fraction, distinct_positions, distinct_canonical_positions, known_canonical_positions, canonical_coverage, decisive_games, decisive_fraction`), `outcomes`, `by_pairing`, `by_length`, `by_ply`, `by_first_action_orbit`, `thresholds`, `diversity_pass`, `diversity_failures`.

- Run directory layout: `generate --config <toml> --out <dir>` writes `<dir>/run.json`, `games.jsonl` (one `GameRecord` per game), `positions.jsonl` (one `PositionRecord` per ply); `annotate --corpus <dir>` adds `<dir>/annotations.jsonl` (one `AnnotationRecord` per distinct raw state, first-appearance order) plus `annotate.json`; `annotate --exhaustive --game <name> --out <dir2>` writes the same two files for every reachable state in BFS order (`run_id` null); `analyze --corpus <dir>` adds `summary.json`. Games and positions are ordered `(cell index, game index, ply)`; every serialized map is a `BTreeMap`; JSONL lines are compact `serde_json::to_writer` plus `\n`; `.json` documents are `to_string_pretty` plus a trailing `\n`; the line terminator is always `\n`. TOML is the only config input (ADR 0003); JSONL/JSON are the only outputs (ADR 0007).

- `run_id`: `run_id = config_hash(resolved config) = hex16(fnv1a64(serde_json::to_vec(&resolved)))` (`src/io/hash.rs`: FNV-1a 64-bit, offset `0xcbf29ce484222325`, prime `0x100000001b3`; `hex16` zero-pads to 16 lowercase hex digits). Hashing the resolved config, not the user-supplied one, means a config with defaults omitted and one with them spelled out share a `run_id` and produce byte-identical output (test `defaults_spelled_out_produce_same_run_id`).

- Byte-identity rule: same resolved config plus same seed implies byte-identical `run.json`, `games.jsonl`, `positions.jsonl`, `annotations.jsonl`, `annotate.json`, `summary.json` across repeated runs and across `--serial` / `--threads N` / global-pool execution. Consequently no output contains timestamps, durations, hostnames, thread counts, absolute paths or git SHAs (durations/progress go to stderr only); no `HashMap` iteration feeds any output; every RNG is `ChaCha8Rng` seeded by the formulas below.

- Seed formulas (`src/discovery/config.rs`, `src/discovery/match_engine.rs`; `splitmix64`, `game_seed`, `player_seed` are ADR 0002 / ADR 0008's, unchanged here):

  ```text
  cell_seed(master, cell_index) = splitmix64(master ^ CELL_SEED_SALT ^ (cell_index as u64).wrapping_mul(CELL_SEED_MUL))
      CELL_SEED_SALT = 0x5851F42D4C957F2D
      CELL_SEED_MUL  = 0xD1B54A32D192ED03
  GameRecord.seed = game_seed(cell_seed(config.seed, cell.index), game_index)      (ADR 0008 game_seed)
  player_seed(game_seed, slot)                                                       (ADR 0008, unchanged)
  opening_seed(game_seed) = splitmix64(game_seed ^ OPENING_SEED_SALT)
      OPENING_SEED_SALT = 0xA0761D6478BD642F
  ```

  `opening_seed` seeds the `ChaCha8Rng` that plays a cell's `random_opening_plies` (uniform over legal actions, one draw per ply) after the fixed opening `actions`; strategies then take over with their ADR 0008 player seeds, so openings never perturb Phase 4 seeds. Test `seeds_follow_the_documented_formulas` (`tests/corpus.rs`) pins the `cell_seed` → `game_seed` chain on persisted records; `opening_seed_reference_values` (`src/discovery/match_engine.rs`) pins `opening_seed`.

- Exhaustive solver limit (`src/discovery/solver.rs`): `ExhaustiveSolver::DEFAULT_LIMIT = 1_000_000` states (mirrored by `annotate::DEFAULT_SOLVER_LIMIT`); exceeding it yields `SolverError::TooLarge { limit }`, message: "state space exceeds the exhaustive-solver limit of {limit} states; exhaustive modes are a small-game accelerant — larger games must sample". `ExhaustiveSolver`, `reachable_states` and `annotate --exhaustive` are small-game accelerants per `docs/plan.md` § Further Considerations item 4; `generate` and `analyze` never depend on exhaustive enumeration. Tic-tac-toe: 5478 reachable states, 958 terminal, 765 canonical forms; annotation cross-checks `game-player` `search` at `engine_depth` (default = the bundle's `full_search_depth`, 9 for tic-tac-toe) against the solver's optimal set (ADR 0001), 0 disagreements over all 5478 positions (test `exhaustive_annotation_covers_all_positions`).

- Compatibility rule: any change to a record or document shape bumps `SCHEMA_VERSION`; every reader calls `check_schema_version` (`src/io/schema.rs`) on every record/document and rejects a mismatch with `IoError::SchemaVersion { path, expected, found }`, no silent migration (test `schema_version_mismatch_is_an_error`); a bump is recorded with a compatibility note, per ADR 0003.

## Consequences

- Evidence: `tests/corpus.rs` (`same_seed_is_byte_identical`, `serial_equals_parallel`, `defaults_spelled_out_produce_same_run_id`, `positions_equal_replay_of_games`, `seeds_follow_the_documented_formulas`, `schema_version_mismatch_is_an_error`); `tests/annotate.rs` (`exhaustive_annotation_covers_all_positions`, `solver_facts`); `artifacts/benchmarks/corpus-generation.md` (9800 games / 75004 position records; `positions.jsonl` 28003228 bytes, `games.jsonl` 6161570 bytes); `artifacts/reproducibility/corpus-determinism.md` and `artifacts/reproducibility/corpus-diversity.md`.

- Phase 8 mining reads `state` (and `canonical_state` for canonical-orbit joins) directly off `PositionRecord`/`AnnotationRecord`; because `AnnotationRecord` keys on raw `state` rather than canonical form, joining a corpus's positions against an annotation set is a plain equality join on `state`, never a canonicalization step, so a mining pass cannot silently pick up a stale transform.

- Records are self-contained (`specs: Vec<StrategySpec>` embedded per game rather than looked up externally), so a `games.jsonl` line can be mined or replayed without its originating `run.json`; `run.json` remains the source of truth only for sweep-level facts (`cells`, resolved `config`) that no single game record carries.

- No feature vectors are persisted, so adding a tier-2 or tier-3 feature (`docs/plan.md`'s open, tiered feature vocabulary) never requires a corpus schema bump or regenerating existing corpora — features are recomputed from `state` at mining time.

- The 1,000,000-state exhaustive limit and the "small-game accelerant" framing mean tic-tac-toe-scale exhaustive annotation (`--exhaustive`) does not generalize: larger games fall back to `annotate --corpus <dir>`, which only touches states actually visited during self-play (2882 distinct raw states out of 75004 positions in the Gate B benchmark), keeping the annotation pass's cost proportional to corpus size rather than state-space size.

- JSONL remains the only persisted format (ADR 0007); the size and read-time cost recorded there versus Parquet is carried forward unchanged by this schema, and stays the accepted cost until `polars`/`arrow` is promoted to root `Cargo.toml`.

- Any future record or document field change must bump `SCHEMA_VERSION` and update every reader; `check_schema_version` makes a stale reader fail loudly (`IoError::SchemaVersion`) instead of silently mis-parsing older or newer corpora, so Phase 8-9 miners can pin against a known schema version.
