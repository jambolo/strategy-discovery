# phase5-corpus — Brief

## Goal

Execute Phase 5 of `docs/plan.md` ("Corpus generation, annotation, and persistence"): land the sweep-driven corpus generation runner, the versioned position-level record schema, the JSONL/JSON persistence layer, the separate annotation pass (exhaustive solver cross-checked against `game-player`), the replay parser and summary aggregator with corpus-diversity metrics, a minimal `generate`/`annotate`/`analyze` CLI so batch runs emit files, and the Gate B evidence (seeded byte-identical reproducibility + diversity thresholds at real corpus sizes). Authoritative spec: `docs/plan.md` lines 129-148 (Phase 5) and lines 271-275 (Gate B) at commit `e4e0a7ea7975301531851c5ded86257ff72d5373` (`docs/plan.md` unchanged since `c2d1386`); line numbers shift if that file is edited — re-verify against live source.

## Context

### Repo state at planning time (commit `e4e0a7ea7975301531851c5ded86257ff72d5373`, branch `feature/phase-5-corpus`, branched from `develop`)

- `develop` = `7e1026e` = `c2d1386` (Phase 4 squash commit "Implemented phase 4: game-player integration, strategies, and match engine.") + `7e1026e` ("Added rustfmt.toml"). The working branch = `develop` + `e4e0a7e` ("Added phase5-corpus plan documents": this brief, the roadmap and the ledger). Source code at `e4e0a7e` is identical to `c2d1386`. Plain feature branch off `develop`; PR back into `develop` when Phase 5 completes (gitflow per `CLAUDE.md`). Earlier feature branches were deleted after squash; only `develop`/`master` exist locally besides the working branch.
- Single Rust crate `strategy-discovery` 0.1.0 (lib + thin bin), edition 2024, stable `rustc 1.94.1 (e408947bf 2026-03-25)`, `cargo 1.94.1`. No `[workspace]` table. No `[dev-dependencies]`.
- Root `[dependencies]` (exact; do not add or change): `anyhow 1.0.104`, `clap 4.6.6 (derive)`, `game-player` (git `https://github.com/jambolo/game-player.git`, branch `master`, locked `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`, crate 0.4.0), `rand 0.10.2`, `rand_chacha 0.10.0`, `rayon 1.12.0`, `serde 1.0.229 (derive)`, `serde_json 1.0.151`, `thiserror 2.0.20`, `toml 1.1.4`, `tracing 0.1.44`, `tracing-subscriber 0.3.23`. Everything Phase 5 needs is already there (clap for the CLI, toml for config, serde_json for JSONL, rayon, rand).
- Baseline verified green at planning time on this machine (code identical at `c2d1386` and `e4e0a7e`): `cargo test` → 191 lib tests + `tests/match_bench.rs` (2, 17.5 s debug) + `tests/match_engine.rs` (9, 3.4 s) + `tests/minimax_tactics.rs` (7, 1.4 s) + `tests/smoke.rs` (2) + `tests/ttt_bench.rs` (2) all pass; whole debug `cargo test` ≈ 25 s wall. `cargo fmt --all --check` exits 0 at `e4e0a7e`.
- Formatting: `rustfmt.toml` at the repo root (committed in `7e1026e`) sets `max_width = 132`; effective config (`cargo fmt -- --print-config current src/lib.rs`) is `max_width = 132`, `single_line_if_else_max_width = 65`, `single_line_let_else_max_width = 65`, so CI's `cargo fmt --all --check` matches local runs. Run `cargo fmt --all` before every commit; do not modify `rustfmt.toml`.
- `git config core.autocrlf` is `true` on the dev machine: checked-out text files (including any TOML fixtures) may carry `\r\n`. `toml` parses CRLF fine. Never byte-compare a checked-out fixture; only compare files the program itself wrote (the program writes `\n` only — Rust's `writeln!`/`b"\n"` never translate).
- Source tree (all `src/` paths):
  - `lib.rs`: `pub mod cli, core, discovery, games, io, strategy`; consts `NAME = "strategy-discovery"`, `VERSION`.
  - `main.rs`: calls `strategy_discovery::cli::run()`.
  - `core/{mod,traits,symmetry,features,derived,dsl,interpreter}.rs` — complete game-agnostic framework core (unchanged by this effort except additively, see Constraints). `core/mod.rs::kinds`: `MINIMAX`, `RANDOM`, `HEURISTIC_RULES`, `SCRIPTED`, `EVOLUTIONARY`, `LLM`.
  - `games/mod.rs` (`pub mod tictactoe;`), `games/tictactoe/{mod,board,rules,primitives,features,canonical,engine,eval,roster}.rs` — complete tic-tac-toe domain.
  - `strategy/{mod,engine,minimax,random,scripted,registry,roster}.rs` — complete (Phase 4).
  - `discovery/mod.rs` (`pub mod match_engine; pub use match_engine::{RayonMatchEngine, game_seed, play_game, player_seed, splitmix64};`), `discovery/match_engine.rs` — the rayon match engine (Phase 4).
  - `io/mod.rs` — 2-line doc-comment placeholder (`//! Versioned record schema, JSONL writers, replay parser (Phase 5).`).
  - `cli/mod.rs` — placeholder: `pub fn run() -> anyhow::Result<()>` prints `"{NAME} {VERSION}"`. `tests/smoke.rs::cli_bootstrap_runs` calls `cli::run()` and asserts `Ok` — this test MUST be rewritten when `run()` starts parsing argv (see Design, CLI), because the test harness's own argv would otherwise reach clap.
  - `tests/smoke.rs`, `tests/ttt_bench.rs`, `tests/match_bench.rs` (non-failing micro-benchmark pattern: `#[test]` + `std::time::Instant` + `std::hint::black_box`, `println!("[bench] ...")`, run with `cargo test --release --test <name> -- --nocapture`), `tests/match_engine.rs`, `tests/minimax_tactics.rs`.
- `spikes/{annotate-spike,throughput-spike,record-format-spike,rule-induction-spike}/` — throwaway crates with their own `Cargo.toml`; NOT built by CI; `src/` must never depend on them; never modified. Reference material only:
  - `spikes/annotate-spike/src/main.rs` lines ~86-135: `solve(rules, state, memo) -> Solved { value: i8, optimal: Vec<Move> }` — exhaustive memoized negamax over `GameRules` (side-to-move perspective: `+1` win / `0` draw / `-1` loss; terminal `Win(_)` is `-1` because the side to move at a won terminal is the loser; `optimal` = every legal action achieving the value, in `legal_actions` order). Lines ~137-147 `value_label(to_move, value)` → `"X wins" | "draw" | "O wins"`. Lines ~245-275 `cross_check`: game-player's `search` move ∈ solver optimal set.
  - `spikes/record-format-spike/src/{record,jsonl,aggregate,selfplay}.rs`: flat provisional `PositionRecord`, `BufWriter` + `serde_json::to_writer` + `b"\n"` JSONL writer, `BufReader::lines` reader, `fnv1a_64`, orbit index of the first move via `symmetry_group().orbits()`, outcome-by-(first_move_orbit | move_number) aggregation sorted for determinism.
- Docs/ADRs that bind this effort:
  - `docs/adr/0001-game-player-minimax-adapter.md` — Decision: "Phase 5 annotation approach: framework-side exhaustive solver (memoized negamax over GameRules), cross-checked against game-player search; flagged as a small-game accelerant." Consequences: solver lands in `src/discovery/`, produces game-theoretic value (win/draw/loss for side to move) plus the full optimal-action set; annotation MUST cross-check `search` against the solver's optimal set (expect 0 disagreements); larger games cannot use the solver (deferred).
  - `docs/adr/0002-self-play-experiment-runner.md` — per-game seed = f(master, game_index); results in `game_index` order; parallel ≡ serial byte-identical; thread count is a performance knob only; "any future change to task granularity or seed derivation must re-run the serial-vs-parallel determinism check".
  - `docs/adr/0003-json-toml-metadata-persistence.md` — TOML for human-authored run configuration input; JSON/JSONL for machine-produced metadata/summaries/reports; every persisted record/document carries `schema_version`; schema changes bump it with a compatibility note.
  - `docs/adr/0004-summary-aggregator.md` — first analyzer = serde deserialization of JSONL + hand-rolled aggregation keyed with `BTreeMap`; outputs: outcome distribution by first-move orbit, by move number, by strategy pairing.
  - `docs/adr/0005-dataframe-layer.md` — serde + custom aggregation; no `polars` in root for Phases 3-7.
  - `docs/adr/0007-corpus-format.md` — JSONL only; `src/io` implements a streaming `serde_json` writer/reader over the versioned position-level record, one JSON object per line; every record and corpus file carries `schema_version`; no Parquet writer in the MVP.
  - `docs/adr/0008-minimax-tie-breaking.md` — `root_values` is `pub` because Phase 5 annotation and Phase 7 metrics reuse it; `TieBreak::Engine` kept for annotation cross-checks; seed formulas (`splitmix64`, `game_seed`, `player_seed`).
  - `docs/component-evaluation.md` § "Consequence for Phase 5": corpus format JSONL only; no `polars`/`parquet`/`arrow`; `linfa`/`linfa-trees`/`ndarray` enter in Phase 8, not before; annotation uses the framework-side exhaustive solver. § "Gate A" table is the format to copy for a new § "Gate B" table. ADR numbering continues at `0009`.
- Evidence numbers to compare against (Phase 3 spikes, release, AMD Ryzen 7 7800X3D 16 logical CPUs): `artifacts/benchmarks/record-format.md` — 10000 games / 76409 position records: JSONL 22,658,488 bytes, write 51.6 ms, read 58.5 ms, serde aggregation 71.2 ms. `artifacts/benchmarks/annotation-per-position.md` — depth-9 `search` mean 6.5 µs/position over 5478 positions (35.7 ms total), exhaustive solver 5478 positions in 3.5 ms; value distribution over all 5478: X wins 2936 / draw 1068 / O wins 1474; over the 4520 non-terminal: 2310 / 1052 / 1158; cross-check disagreements 0 at depth 9 and 10; 958 terminal positions; 765 canonical forms of which 138 are terminal. `artifacts/benchmarks/selfplay-throughput.md` — depth-9 self-play 977 games/s serial, 5409 at 16 threads (spike); Phase 4 `tests/match_bench.rs` on this box: seeded-uniform depth-9 ≈150 games/s serial, ≈1040 parallel (PV-replay tie-break costs ≈6× vs engine tie-break ≈410). Artifact header style (copy it): table with `date`, `git_commit`, `rustc`, `os`, `cpu`, `ram`, `build_profile`, `command`.
- CI (`.github/workflows/ci.yml`, NOT modified): `cargo build --workspace --all-targets`, `cargo test --workspace` on Ubuntu + Windows; PRs also `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo doc --no-deps --workspace` on `master`. Keep debug-build test time in check (see Constraints).
- Commit style: plain descriptive messages, no conventional-commit prefixes. Prior pipeline artifacts: `implementation-artifacts/phase{1,2,3,4}-*` (same directory used here; ledgers of earlier plans show the step/report conventions); this plan's brief/roadmap/ledger are committed at `e4e0a7e`, so step and report files are added alongside them.

### Framework core API (`src/core/traits.rs`, read live source for exact signatures; all at `e4e0a7e`)

- `trait GameDomain: Send+Sync+'static { type State: Clone+Eq+Hash+Debug+Send+Sync+'static; type Action: Clone+Eq+Hash+Debug+Send+Sync+'static; type Player: Copy+Eq+Hash+Debug+Send+Sync+'static; type Outcome: Clone+Eq+Debug+Send+Sync+'static; }` — NOTE: no `Serialize`/`Deserialize` bounds on any associated type; persistence code must add them at its own boundary (see `CorpusGame` in Design).
- `trait GameRules<G> { initial_state; player_to_move(&State)->Player; legal_actions(&State)->Vec<Action> /* empty iff terminal */; apply(&State,&Action)->Result<State,RulesError>; outcome(&State)->Option<Outcome> /* Some iff terminal */; is_terminal (default) }`. `RulesError::{GameOver, IllegalAction(String)}`.
- `trait GamePrimitives<G> { position_count; adjacent; lines()->Vec<Vec<usize>>; symmetry_group()->SymmetryGroup; occupant(&State, usize)->Option<Player>; action_position(&Action)->Option<usize>; transform(&State,&Permutation)->State }`.
- `trait Canonicalize<G>: Send+Sync { canonicalize_with_transform(&State)->(State, Permutation) /* idempotent; primitives.transform(state,&p)==canonical */; canonicalize (default) }`.
- `trait StateEvaluator<G>: Send+Sync { evaluate(&State, perspective: Player)->f32 }`.
- `trait Strategy<G>: Send { choose(&mut self,&State,&[Action])->Result<Action,StrategyError> }`; `trait StrategyProvider<G>: Send+Sync { kind()->&str; create(seed:u64)->Box<dyn Strategy<G>> }`; `StrategyError::{NoLegalActions, Unimplemented{kind,detail}, Other(String)}`.
- `struct MatchConfig { games: usize, seed: u64, max_plies: Option<usize> }` (serde, `Eq`). `struct MatchRecord<A,O> { game_index: usize, seed: u64, actions: Vec<A>, outcome: Option<O> }` (serde; `outcome: None` iff aborted by `max_plies`). `enum MatchError { MissingProvider(String), Strategy(#[from] StrategyError), Rules(#[from] RulesError) }` (`Clone+PartialEq+Eq`). `trait MatchEngine<G> { run(&self,&MatchConfig,&[(G::Player,&dyn StrategyProvider<G>)])->Result<Vec<MatchRecord<..>>,MatchError> }`.
- `core/symmetry.rs`: `Permutation { new(Vec<usize>)->Result; identity(degree); degree(); is_identity(); apply(usize)->usize; as_slice()->&[usize]; compose; inverse; permute }`; `SymmetryGroup { generate; elements(); order(); degree(); orbits()->Vec<Vec<usize>> /* each ascending, ordered by smallest element */; orbit_index()->Vec<usize> /* orbit_index()[p] = index into orbits() */; set_orbits }`. Tic-tac-toe orbits: `[[0,2,6,8],[1,3,5,7],[4]]` → orbit 0 = corners, 1 = edges, 2 = centre.
- `core/features.rs`/`derived.rs`/`dsl.rs`/`interpreter.rs`: not needed by Phase 5 (features are computed by the Phase 8 feature-dataset stage, not stored in corpus records).

### Strategy / discovery API (exists; read live source)

- `strategy::engine`: `WIN_VALUE: f32 = 100.0`; `trait EngineGame: GameDomain<State: game_player::State<Action = Self::Action>> { fn player_id(Player)->PlayerId; fn player_from_id(PlayerId)->Player; fn winner(&Outcome)->Option<Player> }`; `RulesResponseGenerator<'a,G>::new(&dyn GameRules<G>)`; `AliceEvaluator<'a,G>::new(&dyn GameRules<G>, &dyn StateEvaluator<G>)` (terminal-aware, Alice perspective, clamps heuristics to `[-99, 99]`).
- `strategy::minimax`: `TieBreak::{Engine, SeededUniform(default)}` (`kebab-case` serde), `MinimaxConfig { depth: u32, #[serde(default)] epsilon: f64, #[serde(default)] tie_break: TieBreak }` + `validate()`; `MinimaxProvider<G>`, `MinimaxStrategy<G>`; `pub fn root_values<G: EngineGame>(rules: &dyn GameRules<G>, evaluator: &dyn StateEvaluator<G>, state, legal, depth) -> Vec<(G::Action, f32)>` (Alice-perspective depth-`depth` value per legal action, PV replay). `game_player::minimax::search(&sef, &rg, state, depth) -> Option<Action>` returns `None` iff terminal.
- `strategy::registry`: `#[serde(tag = "kind", rename_all = "kebab-case")] enum StrategySpec { Minimax(MinimaxConfig), Random, HeuristicRules { heuristic }, Evolutionary, Llm }` (`Clone+Debug+PartialEq`, NOT `Eq` — `MinimaxConfig` holds an `f64`); JSON shape `{"kind":"minimax","depth":9,"epsilon":0.0,"tie_break":"seeded-uniform"}`; TOML round-trip verified (`kind = "minimax"` inside `[[entries]]` tables). `StrategySpec::kind()->&'static str`. `EngineBundle<G> { rules: Arc<dyn GameRules<G>>, evaluator: Arc<dyn StateEvaluator<G>> }` (`Clone`). `StrategyRegistry<G: EngineGame>::new(EngineBundle<G>)`, `build(&StrategySpec)->Result<Box<dyn StrategyProvider<G>>,StrategyError>` (validates minimax config; reserved kinds → `Unimplemented`), `register`, `kinds()`, `bundle()`.
- `strategy::roster`: `RosterEntry { name: String, spec: StrategySpec }`, `Roster { name, version, entries }` + `new/validate/id/get/graded`. Tic-tac-toe `games::tictactoe::benchmark_roster()` → `Roster::graded("ttt-benchmark", 1, &[1,2,3,4,6], 9)`: entries `random, depth-1, depth-2, depth-3, depth-4, depth-6, perfect` (7; all minimax entries `epsilon 0.0`, `SeededUniform`); id `ttt-benchmark-v1`.
- `discovery::match_engine` (ADR 0002/0008 formulas — do not change): `splitmix64(x)` (`splitmix64(0) == 0xE220A8397B1DCDAF`, `splitmix64(1) == 0x910A2DEC89025CC1`); `game_seed(master, i) = splitmix64(master ^ (i as u64).wrapping_mul(0x9E3779B97F4A7C15))` (`game_seed(42,0) == 0xBDD732262FEB6E95`); `player_seed(game_seed, slot) = splitmix64(game_seed ^ (slot as u64 + 1).wrapping_mul(0xD1B54A32D192ED03))`; `play_game(rules, players, game_index, game_seed, max_plies)` (creates one strategy per `players` slot seeded by `player_seed(game_seed, slot)`, duplicate player → first slot wins, illegal returned action → `MatchError::Strategy(Other(..))`); `RayonMatchEngine<G> { new(rules) /* global pool */, with_threads(rules, n) /* private pool built per run */, serial(rules), threads(), is_parallel() }` implementing `MatchEngine<G>` (serial loop or `into_par_iter().map().collect()` in `game_index` order). Struct fields are private; constructors are the only way to build it.
- `rand 0.10` names: `rand::SeedableRng::seed_from_u64`, `rand::RngExt::{random, random_range}` (on `RngExt`, NOT `Rng` — `use rand::RngExt;`), `rand::seq::IndexedRandom::choose`; RNG type `rand_chacha::ChaCha8Rng`.

### Tic-tac-toe domain API (`src/games/tictactoe`, exists; read live source)

- Re-exports from `games/tictactoe/mod.rs`: `Board, LINES, Move, Outcome, Player, TicTacToe, TicTacToeEvaluator, TicTacToeRules, benchmark_roster`; `engine_bundle() -> EngineBundle<TicTacToe>` (`TicTacToeRules` + `TicTacToeEvaluator`).
- `TicTacToe: GameDomain` with `State = Board`, `Action = Move(pub usize)` (cell 0..9 row-major), `Player::{X, O}` (`X` moves first; `.other()`), `Outcome::{Win(Player), Draw}`. All four are `Copy + Serialize + Deserialize + Hash + Eq + Debug` (derived). Serde shapes: `Board` → `{"cells":[null,"X",...9 entries...],"to_move":"O"}`; `Move(4)` → `4` (newtype struct = inner value); `Player::X` → `"X"`; `Outcome::Win(Player::X)` → `{"Win":"X"}`, `Outcome::Draw` → `"Draw"`. Debug: `Win(X)`, `Draw`, `X`, `O`.
- `Board`: `empty()`, `from_cells([Option<Player>;9], to_move)`, `cells()`, `cell(i)`, `to_move()`, `winner()`, `is_full()`, `place(i, player)`, `parse(&str)` (9 chars over `{X,O,.}`; side to move derived from counts: `X` when `#X == #O`, `O` when `#X == #O + 1`, else `Err` — fixtures MUST obey this), `Display` (3 rows).
- `TicTacToeRules` (`Copy`, `Default`): `legal_actions` = empty cells ascending; `reachable_positions() -> Vec<Board>` (5478 incl. 958 terminals, BFS first-discovery order — this inherent method is tic-tac-toe-specific; the generic equivalent is added in Design `reachable_states`).
- `TicTacToeCanonicalizer::new()` (`Canonicalize<TicTacToe>`, minimum base-3 encoding over the 8 D4 images; 765 canonical forms over the 5478 reachable; `canonical.rs::encode(&Board)->u32`). `TicTacToePrimitives` (`GamePrimitives<TicTacToe>`; `action_position(Move(c)) == Some(c)`; `symmetry_group()` = D4 of order 8; `transform` keeps `to_move`).
- `engine.rs`: `impl game_player::State for Board`, `impl EngineGame for TicTacToe` (`X ↔ Alice` maximizing, `O ↔ Bob`, `winner(Win(p)) = Some(p)`, `winner(Draw) = None`).
- `eval.rs::TicTacToeEvaluator` (`Copy`, `Default`): open-lines heuristic in `[-8, 8]`, terminal-aware.

### Design (decisions fixed by the planner; the decomposer projects these into steps)

Module placement (new files unless marked exists):

| path | contents |
| --- | --- |
| `src/io/mod.rs` (exists, placeholder) | `pub mod hash, jsonl, replay, schema;` + re-exports; `IoError` |
| `src/io/hash.rs` | `fnv1a64`, `hex16`, `config_hash` |
| `src/io/jsonl.rs` | `write_jsonl`, `read_jsonl`, `JsonlReader<T>`, `write_json_pretty`, `read_json` |
| `src/io/schema.rs` | `SCHEMA_VERSION`, file-name consts, `CorpusGame`, `CellKey`, `GameRecord`, `PositionRecord`, `AnnotationRecord` |
| `src/io/replay.rs` | `replay`, `final_state` — rebuild (state, action) pairs from an action list |
| `src/strategy/engine.rs` (exists) | add `ConstantEvaluator` |
| `src/discovery/match_engine.rs` (exists) | add `Opening<A>`, `OPENING_SEED_SALT`, `opening_seed`, `play_game_with_opening`, `opening_plies_played`, `RayonMatchEngine::with_opening` |
| `src/discovery/solver.rs` | `Solved<A>`, `SolverError`, `ExhaustiveSolver<G>`, `reachable_states` |
| `src/discovery/bundle.rs` | `GameBundle<G>` — everything the corpus/annotation/summary stages need from a game |
| `src/discovery/config.rs` | `GenerateConfig<A>`, `Pairing`, `NamedOpening<A>`, `ResolvedConfig<A>`, `Cell<A>`, `cell_seed`, `CorpusError` |
| `src/discovery/corpus.rs` | `GenerateOptions`, `RunMetadata`, `generate`, `expand_game` — the corpus generation runner |
| `src/discovery/annotate.rs` | `AnnotateMode`, `AnnotateOptions`, `AnnotateMetadata`, `annotate_states`, `annotate_corpus`, `annotate_exhaustive` |
| `src/discovery/summary.rs` | `OutcomeCounts`, `DiversityThresholds`, `CorpusSummary`, `summarize`, `analyze_corpus`, `verify_corpus` |
| `src/discovery/mod.rs` (exists) | `pub mod annotate, bundle, config, corpus, match_engine, solver, summary;` + re-exports (keep the existing `match_engine` re-exports) |
| `src/games/tictactoe/corpus.rs` | `game_bundle() -> GameBundle<TicTacToe>` |
| `src/games/tictactoe/mod.rs` (exists) | add `pub mod corpus;` + `pub use corpus::game_bundle;` |
| `src/cli/mod.rs` (exists, placeholder) | clap `Cli`/`Command::{Generate, Annotate, Analyze}`, `run`, `run_from` |
| `tests/smoke.rs` (exists) | `cli_bootstrap_runs` → call `cli::run_from(["strategy-discovery"])` |
| `tests/fixtures/generate-small.toml`, `tests/fixtures/generate-draws.toml` | fixture configs for tests |
| `configs/tictactoe-default.toml` | the checked-in default corpus config (CLI docs + evidence runs) |
| `tests/corpus.rs` | generation determinism, replay equality, openings, diversity, schema errors |
| `tests/annotate.rs` | corpus-mode and exhaustive annotation |
| `tests/cli_pipeline.rs` | spawns the binary: `generate` → `annotate` → `analyze` |
| `tests/corpus_bench.rs` | non-failing games/s + positions/s + write/read micro-benchmark |
| `docs/adr/0009-corpus-record-schema.md` | ADR for schema v1 + run directory layout |
| `docs/component-evaluation.md` (exists) | append `## Gate B` section |
| `artifacts/benchmarks/corpus-generation.{md,json}`, `artifacts/reproducibility/corpus-determinism.md`, `artifacts/reproducibility/corpus-diversity.md` | Gate B evidence |
| `README.md` (exists) | Usage section: real `generate`/`annotate`/`analyze` invocations |

Layering: `src/core` unchanged; `src/io` and `src/discovery` are game-agnostic (generic over `G`, bounded by `EngineGame` / `CorpusGame`); `src/cli` is the composition root and is the ONLY non-test place that names a concrete game (`"tictactoe"` → `games::tictactoe::game_bundle()`). Everything tic-tac-toe-specific lives under `src/games/tictactoe/`. Tests in `io`/`discovery` modules MAY use tic-tac-toe.

**Byte-identity rule (applies to every file the pipeline writes).** Same resolved config + same seed ⇒ byte-identical `run.json`, `games.jsonl`, `positions.jsonl`, `annotations.jsonl`, `annotate.json`, `summary.json` — across repeated runs AND across serial / 4-thread / global-pool execution. Therefore no output file may contain timestamps, durations, hostnames, thread counts, absolute paths, git SHAs, or anything environment-dependent; durations/progress go to stderr only. Every map serialized into an output is a `BTreeMap`; every list is in a defined order (cell index, game index, ply; first-appearance order for annotations). JSON is written with `serde_json::to_writer` (compact) for JSONL lines and `serde_json::to_string_pretty` + trailing `"\n"` for `.json` documents; line terminator is always `\n`.

**`src/io/hash.rs`.** `pub fn fnv1a64(bytes: &[u8]) -> u64` (offset `0xcbf29ce484222325`, prime `0x100000001b3`; reference: `fnv1a64(b"") == 0xcbf29ce484222325`, `fnv1a64(b"a") == 0xaf63dc4c8601ec8c`); `pub fn hex16(x: u64) -> String` = `format!("{x:016x}")`; `pub fn config_hash<T: Serialize>(value: &T) -> Result<String, IoError>` = `hex16(fnv1a64(&serde_json::to_vec(value)?))`.

**`src/io/jsonl.rs`.** `pub fn write_jsonl<T: Serialize>(path: &Path, items: impl IntoIterator<Item = T>) -> Result<usize, IoError>` (`File::create` → `BufWriter`; per item `serde_json::to_writer` then `b"\n"`; flush; returns count). `pub fn read_jsonl<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, IoError>`. `pub struct JsonlReader<T>` (`BufReader` over the file, `Iterator<Item = Result<T, IoError>>`, 1-based line numbers in errors, empty lines skipped). `pub fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), IoError>`; `pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, IoError>`. `IoError` (thiserror): `Io { path: PathBuf, #[source] source: std::io::Error }`, `Json { path: PathBuf, line: usize, #[source] source: serde_json::Error }`, `Toml { path: PathBuf, message: String }`, `SchemaVersion { path: PathBuf, expected: u32, found: u32 }`, `Missing { path: PathBuf }`, `Invalid(String)`. Schema check: `pub fn check_schema_version(path: &Path, found: u32) -> Result<(), IoError>` used by every reader of a record with a `schema_version` field (every record/document below has one; readers verify the first record and every subsequent record's field equals `SCHEMA_VERSION`).

**`src/io/schema.rs`.**

```rust
pub const SCHEMA_VERSION: u32 = 1;
pub const RUN_FILE: &str = "run.json";
pub const GAMES_FILE: &str = "games.jsonl";
pub const POSITIONS_FILE: &str = "positions.jsonl";
pub const ANNOTATIONS_FILE: &str = "annotations.jsonl";
pub const ANNOTATE_FILE: &str = "annotate.json";
pub const SUMMARY_FILE: &str = "summary.json";

/// Games whose state/action/player/outcome types can be persisted. Blanket-implemented.
pub trait CorpusGame: GameDomain<State: Serialize + DeserializeOwned, Action: Serialize + DeserializeOwned, Player: Serialize + DeserializeOwned, Outcome: Serialize + DeserializeOwned> {}
impl<G: GameDomain> CorpusGame for G where G::State: Serialize + DeserializeOwned, G::Action: ..., G::Player: ..., G::Outcome: ... {}
```

(Exact bound syntax is the implementer's call — associated-type bounds in supertrait position are the Phase 4 precedent for `EngineGame`; a where-clause per function is the fallback — as long as code generic over `G: CorpusGame` can serialize/deserialize all four associated types and `TicTacToe: CorpusGame` holds without any impl in the tic-tac-toe module.)

```rust
/// One sweep cell: a strategy pairing × evaluator variant × opening × random-opening-plies.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CellKey { pub index: usize, pub strategies: Vec<String> /* aligned to GameBundle.players */, pub evaluator: String, pub opening: String, pub random_opening_plies: u32 }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GameRecord<S, A, P, O> {
    pub schema_version: u32,
    pub run_id: String,
    pub game_id: String,            // format!("{}:{}", cell.index, game_index)
    pub cell: CellKey,
    pub game_index: usize,          // within the cell
    pub seed: u64,                  // MatchRecord.seed == game_seed(cell.seed, game_index)
    pub players: Vec<P>,            // GameBundle.players, turn order
    pub specs: Vec<StrategySpec>,   // aligned to players (self-contained: full strategy params)
    pub opening_plies: usize,       // plies played by the opening (fixed + random) before strategies took over
    pub actions: Vec<A>,
    pub outcome: Option<O>,         // None iff aborted by max_plies
    pub length: usize,              // actions.len()
    pub final_state: S,
    pub final_canonical_state: S,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PositionRecord<S, A, P, O> {
    pub schema_version: u32,
    pub game_id: String,
    pub ply: usize,                 // 0-based index into GameRecord.actions; state is BEFORE chosen_action
    pub side_to_move: P,
    pub state: S,
    pub canonical_state: S,         // == state when the game has no canonicalizer
    pub canonical_transform: Vec<usize>,  // Permutation::as_slice() mapping state -> canonical_state; empty = identity / none
    pub legal_actions: Vec<A>,      // rules.legal_actions(state), rules order
    pub chosen_action: A,
    pub chosen_by: String,          // "opening" | "random-opening" | strategy entry name of the side to move
    pub outcome: Option<O>,         // the game's outcome, denormalized for flat mining
    pub game_length: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnnotationRecord<S, A, P> {
    pub schema_version: u32,
    pub state: S,                   // the key: raw (not canonical) state
    pub canonical_state: S,
    pub side_to_move: P,
    pub terminal: bool,
    pub value: i8,                  // side-to-move perspective: 1 win, 0 draw, -1 loss (exhaustive solver)
    pub optimal_actions: Vec<A>,    // every legal action achieving `value`, legal order; empty iff terminal
    pub engine_action: Option<A>,   // game-player `search` at engine_depth; None iff terminal
    pub engine_agrees: Option<bool>,// Some(optimal_actions.contains(engine_action)) iff non-terminal
}
```

`StrategySpec` is imported from `crate::strategy::registry` (io → strategy dependency is accepted; io is a pipeline layer, not core). Generic derives add the obvious bounds on `S/A/P/O`; tic-tac-toe's types satisfy them. Records NEVER carry feature vectors (Phase 8 computes them from `state`).

**`src/io/replay.rs`.** `pub fn replay<G: GameDomain>(rules: &dyn GameRules<G>, actions: &[G::Action]) -> Result<Vec<(G::State, G::Action)>, RulesError>` — from `rules.initial_state()`, yields `(state_before, action)` per action, applying each via `rules.apply` (errors propagate). `pub fn final_state<G>(rules, actions) -> Result<G::State, RulesError>`. These are the single source of truth for turning an action list into positions; `corpus::expand_game` uses them, and a test proves `positions.jsonl` equals the replay of `games.jsonl`.

**Engine additions (`src/strategy/engine.rs`, `src/discovery/match_engine.rs`) — all additive, existing signatures unchanged.**

- `#[derive(Debug, Clone, Copy, PartialEq)] pub struct ConstantEvaluator(pub f32);` with `impl<G: GameDomain> StateEvaluator<G> for ConstantEvaluator { evaluate(..) = self.0 }` — a game-agnostic "no heuristic" evaluator variant: depth-limited search then sees every non-terminal leaf as equal, so seeded-uniform tie-breaking spreads play widely (a real corpus-diversity lever).
- `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)] pub struct Opening<A> { #[serde(default)] pub actions: Vec<A>, #[serde(default)] pub random_plies: u32 }` with a MANUAL `impl<A> Default for Opening<A>` (a derived `Default` would wrongly require `A: Default`; `Move` has none).
- `pub const OPENING_SEED_SALT: u64 = 0xA0761D6478BD642F; pub fn opening_seed(game_seed: u64) -> u64 { splitmix64(game_seed ^ OPENING_SEED_SALT) }`.
- `pub fn play_game_with_opening<G: GameDomain>(rules, players, game_index, game_seed, max_plies, opening: &Opening<G::Action>) -> Result<MatchRecord<..>, MatchError>`: (1) create strategies exactly as `play_game` does (same `player_seed(game_seed, slot)` — existing seeds and Phase 4 tests unchanged); (2) apply `opening.actions` in order via `rules.apply` (an illegal/terminal-state action surfaces as `MatchError::Rules`); stop if the state becomes terminal or `max_plies` is reached (same `outcome` rules as the normal loop); (3) play `opening.random_plies` plies choosing uniformly among `rules.legal_actions` with `ChaCha8Rng::seed_from_u64(opening_seed(game_seed))` (`legal.choose(&mut rng)`, one draw per ply), stopping early on terminal / `max_plies`; (4) continue with the existing strategy loop. `max_plies` counts opening plies. `play_game(..)` becomes `play_game_with_opening(.., &Opening::default())` (behaviour identical). `pub fn opening_plies_played<A, O>(record: &MatchRecord<A, O>, opening: &Opening<A>) -> usize = record.actions.len().min(opening.actions.len() + opening.random_plies as usize)`.
- `impl RayonMatchEngine<G> { pub fn with_opening(mut self, opening: Opening<G::Action>) -> Self; pub fn opening(&self) -> &Opening<G::Action> }` — `run` passes the engine's opening to `play_game_with_opening` in both serial and parallel paths. Default opening = empty (existing behaviour, existing tests stay green). Serial ≡ parallel byte-identity must hold with openings (the opening RNG is derived from `game_seed` only).

**`src/discovery/solver.rs` — the small-game accelerant (ADR 0001).**

```rust
#[derive(Clone, Debug, PartialEq, Eq)] pub struct Solved<A> { pub value: i8, pub optimal: Vec<A> }
#[derive(Debug, Error, Clone, PartialEq, Eq)] pub enum SolverError {
    #[error("state space exceeds the exhaustive-solver limit of {limit} states; exhaustive modes are a small-game accelerant — larger games must sample")]
    TooLarge { limit: usize },
}
pub struct ExhaustiveSolver<G: EngineGame> { rules: Arc<dyn GameRules<G>>, memo: HashMap<G::State, Solved<G::Action>>, limit: usize }
impl<G: EngineGame> ExhaustiveSolver<G> {
    pub const DEFAULT_LIMIT: usize = 1_000_000;
    pub fn new(rules: Arc<dyn GameRules<G>>, limit: usize) -> Self;
    pub fn solve(&mut self, state: &G::State) -> Result<Solved<G::Action>, SolverError>;  // memoized negamax
    pub fn solved_states(&self) -> usize;
}
pub fn reachable_states<G: GameDomain>(rules: &dyn GameRules<G>, limit: usize) -> Result<Vec<G::State>, SolverError>;
```

Semantics (identical to the spike's `solve`): terminal state → `value` from `G::winner(&outcome)`: `Some(p)` with `p == rules.player_to_move(state)` → `1`, `Some(_)` → `-1`, `None` → `0`; `optimal = []`. Non-terminal → `value = max over legal a of -solve(apply(state, a)).value`, `optimal` = every legal action achieving it, in `legal_actions` order. `memo` is a `HashMap` used for lookup only (no iteration feeds any output). `TooLarge` when `memo.len()` would exceed `limit`. `reachable_states` = BFS from `initial_state` over `legal_actions`, first-discovery order, deduplicated, includes terminals (same algorithm as `TicTacToeRules::reachable_positions`; for tic-tac-toe returns the same 5478 boards in the same order — assert in a test), `TooLarge` past `limit`. Expected tic-tac-toe facts (assert in tests): `solve(empty)` → `value 0`, `optimal` = all 9; memo 5478 after solving from empty; label distribution over all 5478 (label = `"X wins"` if `(to_move, value) ∈ {(X,1),(O,-1)}`, `"O wins"` if `{(O,1),(X,-1)}`, else `"draw"`): X wins 2936 / draw 1068 / O wins 1474; over the 4520 non-terminal: 2310 / 1052 / 1158; `XX.OO....` (X to move) → `value 1`, `optimal [Move(2)]`; `XX.O.....` (O to move) → `value -1`, `optimal` = all six empties (every O move loses — see Phase 4 ledger note); `X...O...X` (O to move) → `value 0`, `optimal [1,3,5,7]`.

**`src/discovery/bundle.rs`.**

```rust
pub struct GameBundle<G: EngineGame> {
    pub name: String,                                        // "tictactoe"
    pub rules: Arc<dyn GameRules<G>>,
    pub players: Vec<G::Player>,                             // turn order; players[0] moves first
    pub evaluators: BTreeMap<String, Arc<dyn StateEvaluator<G>>>,
    pub default_evaluator: String,                           // key into evaluators
    pub canonicalizer: Option<Arc<dyn Canonicalize<G>>>,
    pub primitives: Option<Arc<dyn GamePrimitives<G>>>,
    pub default_strategies: Vec<RosterEntry>,
    pub full_search_depth: u32,                              // engine depth exhaustive for this game
    pub known_canonical_positions: Option<usize>,            // coverage denominator
}
impl<G: EngineGame> GameBundle<G> {
    pub fn engine_bundle(&self, evaluator: &str) -> Result<EngineBundle<G>, CorpusError>;  // unknown name → CorpusError::Config
    pub fn canonicalize(&self, state: &G::State) -> (G::State, Vec<usize>);                   // (state.clone(), vec![]) when no canonicalizer
    pub fn player_slot(&self, player: G::Player) -> Option<usize>;
}
```

Tic-tac-toe (`src/games/tictactoe/corpus.rs::game_bundle()`): `name "tictactoe"`, `TicTacToeRules`, `players [X, O]`, evaluators `{"default": TicTacToeEvaluator, "zero": ConstantEvaluator(0.0)}`, `default_evaluator "default"`, `Some(TicTacToeCanonicalizer::new())`, `Some(TicTacToePrimitives)`, `default_strategies = benchmark_roster().entries`, `full_search_depth 9`, `known_canonical_positions Some(765)`.

**`src/discovery/config.rs` — the sweep model.** The plan's sweep dimensions map as follows: search depth per side and epsilon rates → the `StrategySpec`s in `strategies` (any depth/epsilon combination is just another named entry); tie-break seeds → `seed`; evaluator variants → `evaluators`; starting-position variants / varied openings → `openings` + `random_opening_plies`; opponent pairings from the roster (mixed strengths) → `pairings` (default: ALL ordered pairs of `strategies`, including self-play).

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)] #[serde(transparent)]
pub struct Pairing(pub Vec<String>);                        // strategy names aligned to GameBundle.players; TOML: pairings = [["random","perfect"], ["perfect","random"]]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NamedOpening<A> { pub name: String, #[serde(default)] pub actions: Vec<A> }   // TOML: [[openings]] name = "center" / actions = [4]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(deny_unknown_fields)]
pub struct GenerateConfig<A> {
    pub schema_version: u32,                                  // must equal SCHEMA_VERSION
    pub game: String,                                         // must equal bundle.name
    pub seed: u64,
    pub games_per_cell: usize,                                // > 0
    #[serde(default)] pub max_plies: Option<usize>,
    #[serde(default)] pub strategies: Vec<RosterEntry>,       // empty ⇒ bundle.default_strategies
    #[serde(default)] pub pairings: Vec<Pairing>,             // empty ⇒ all ordered pairs (n^players, listed order, outer loop = slot 0)
    #[serde(default)] pub evaluators: Vec<String>,            // empty ⇒ [bundle.default_evaluator]
    #[serde(default)] pub openings: Vec<NamedOpening<A>>,     // empty ⇒ [NamedOpening { name: "none", actions: [] }]
    #[serde(default)] pub random_opening_plies: Vec<u32>,     // empty ⇒ [0]
}
impl<A: DeserializeOwned> GenerateConfig<A> { pub fn from_toml_str(text: &str) -> Result<Self, CorpusError>; pub fn from_toml_file(path: &Path) -> Result<Self, CorpusError>; }
pub struct Cell<A> { pub key: CellKey, pub entries: Vec<RosterEntry> /* aligned to players */, pub opening: Opening<A>, pub seed: u64 }
pub struct ResolvedConfig<A> { pub config: GenerateConfig<A> /* defaults filled in */, pub cells: Vec<Cell<A>> }
pub fn resolve<G: EngineGame + CorpusGame>(config: GenerateConfig<G::Action>, bundle: &GameBundle<G>) -> Result<ResolvedConfig<G::Action>, CorpusError>;
pub const CELL_SEED_SALT: u64 = 0x5851F42D4C957F2D; pub const CELL_SEED_MUL: u64 = 0xD1B54A32D192ED03;
pub fn cell_seed(master: u64, cell_index: usize) -> u64 { splitmix64(master ^ CELL_SEED_SALT ^ (cell_index as u64).wrapping_mul(CELL_SEED_MUL)) }
#[derive(Debug, Error)] pub enum CorpusError { #[error("invalid configuration: {0}")] Config(String), #[error(transparent)] Match(#[from] MatchError), #[error(transparent)] Strategy(#[from] StrategyError), #[error(transparent)] Io(#[from] IoError), #[error(transparent)] Rules(#[from] RulesError), #[error(transparent)] Solver(#[from] SolverError) }
```

Cell enumeration (fixed; `index` counts from 0): `for pairing in pairings { for evaluator in evaluators { for opening in openings { for k in random_opening_plies { cell } } } }`; `CellKey { index, strategies: pairing.0.clone(), evaluator, opening: opening.name, random_opening_plies: k }`; `Cell.opening = Opening { actions: opening.actions.clone(), random_plies: k }`; `Cell.seed = cell_seed(config.seed, index)`. Validation (all → `CorpusError::Config` with a message naming the offending item): `schema_version != SCHEMA_VERSION`; `game != bundle.name`; `games_per_cell == 0`; empty or duplicate strategy names; a pairing whose length ≠ `bundle.players.len()` or naming an unknown strategy; unknown evaluator; empty or duplicate opening names; duplicate `random_opening_plies` values; an opening whose actions are not legal when replayed from `initial_state` (use `io::replay::replay`); any strategy spec that `StrategyRegistry::new(bundle.engine_bundle(default_evaluator)).build(spec)` rejects (depth 0, reserved kinds). The resolved config is what gets hashed and persisted, so a config with defaults omitted and one with them spelled out produce the same `run_id` and byte-identical output (test this).

**`src/discovery/corpus.rs` — generation runner.**

```rust
#[derive(Debug, Clone, Default)] pub struct GenerateOptions { pub threads: Option<usize>, pub serial: bool }   // default: parallel on rayon's global pool
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RunMetadata<A, P> { pub schema_version: u32, pub run_id: String, pub config_hash: String, pub crate_name: String, pub crate_version: String, pub game: String, pub players: Vec<P>, pub config: GenerateConfig<A> /* resolved */, pub cells: Vec<CellKey>, pub games: usize, pub positions: usize }
pub fn generate<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, config: GenerateConfig<G::Action>, options: &GenerateOptions, out_dir: &Path) -> Result<RunMetadata<G::Action, G::Player>, CorpusError>;
pub fn expand_game<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, run_id: &str, cell: &Cell<G::Action>, record: &MatchRecord<G::Action, G::Outcome>) -> Result<(GameRecord<..>, Vec<PositionRecord<..>>), CorpusError>;
```

`generate`: `resolve` → `run_id = config_hash(&resolved.config)` (`config_hash == run_id`) → `create_dir_all(out_dir)` → open `games.jsonl` / `positions.jsonl` writers → for each cell in order: registry for the cell's evaluator (`StrategyRegistry::new(bundle.engine_bundle(&key.evaluator)?)`, cached per evaluator name in a `BTreeMap`), providers `registry.build(&entry.spec)?` per slot, `players: Vec<(G::Player, &dyn StrategyProvider<G>)>` = `bundle.players` zipped with providers, engine = `serial` / `with_threads(n)` / `new` per options, `.with_opening(cell.opening.clone())`, `run(&MatchConfig { games: games_per_cell, seed: cell.seed, max_plies })?` → for each record `expand_game` → write the game line then its position lines (streaming; never hold the whole corpus in memory) → finally `run.json` with totals. Parallelism is within a cell (cells run sequentially); that keeps ADR 0002's engine as the only parallel loop. `expand_game`: `replay(rules, &record.actions)` → for ply `i` with `(state, action)`: `side_to_move = rules.player_to_move(&state)`; `(canonical_state, canonical_transform) = bundle.canonicalize(&state)`; `legal_actions = rules.legal_actions(&state)`; `opening_plies = opening_plies_played(record, &cell.opening)`; `chosen_by` = `"opening"` if `i < cell.opening.actions.len()`, else `"random-opening"` if `i < opening_plies`, else `cell.entries[bundle.player_slot(side_to_move)].name`; `final_state = final_state(rules, &record.actions)`; `game_id = format!("{}:{}", cell.key.index, record.game_index)`. Positions file order = (cell index, game index, ply).

**`src/discovery/annotate.rs` — separate pass, reads a corpus, never regenerates it.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "kebab-case")] pub enum AnnotateMode { Corpus, Exhaustive }
#[derive(Debug, Clone)] pub struct AnnotateOptions { pub engine_depth: Option<u32> /* None ⇒ bundle.full_search_depth */, pub solver_limit: usize /* default ExhaustiveSolver::DEFAULT_LIMIT */ }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AnnotateMetadata { pub schema_version: u32, pub mode: AnnotateMode, pub game: String, pub run_id: Option<String> /* corpus mode */, pub engine_depth: u32, pub evaluator: String /* bundle.default_evaluator, used for the engine cross-check */, pub annotated: usize, pub terminal: usize, pub disagreements: usize, pub solver_states: usize }
pub fn annotate_states<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, states: &[G::State], engine_depth: u32, solver: &mut ExhaustiveSolver<G>) -> Result<Vec<AnnotationRecord<G::State, G::Action, G::Player>>, CorpusError>;
pub fn annotate_corpus<G>(bundle, corpus_dir: &Path, options: &AnnotateOptions) -> Result<AnnotateMetadata, CorpusError>;   // reads run.json (game must match bundle.name) + positions.jsonl via JsonlReader, dedupes `state` in FIRST-APPEARANCE order (HashSet for membership, Vec for order), annotates, writes <corpus_dir>/annotations.jsonl + annotate.json
pub fn annotate_exhaustive<G>(bundle, out_dir: &Path, options: &AnnotateOptions) -> Result<AnnotateMetadata, CorpusError>;  // reachable_states(rules, solver_limit) in BFS order (all states, terminals included), annotates, writes <out_dir>/annotations.jsonl + annotate.json (run_id None)
```

Per state: `solved = solver.solve(state)?`; `terminal = rules.is_terminal(state)`; `engine_action = search(&AliceEvaluator::new(rules, default evaluator), &RulesResponseGenerator::new(rules), state, engine_depth)` (`None` iff terminal — the raw engine choice, equivalent to `TieBreak::Engine`; ADR 0008 keeps that mode for exactly this cross-check); `engine_agrees = engine_action.map(|a| solved.optimal.contains(&a))`. `disagreements` = count of `Some(false)`. Expected for tic-tac-toe: exhaustive mode → 5478 records, 958 terminal, 0 disagreements at depth 9; corpus mode → one record per distinct `state` in `positions.jsonl`, 0 disagreements. Annotation is keyed by the RAW state (not canonical) so that `optimal_actions` never need a transform; `canonical_state` is present for joins.

**`src/discovery/summary.rs` — first analyzer (ADR 0004) + diversity metrics (plan step 5).**

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct OutcomeCounts { pub wins: BTreeMap<String, usize> /* key = format!("{winner:?}") */, pub draws: usize, pub unfinished: usize, pub total: usize }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiversityThresholds { pub min_canonical_coverage: f64, pub min_decisive_fraction: f64, pub min_distinct_game_fraction: f64 }   // Default: 0.5 / 0.2 / 0.5
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CorpusSummary {
    pub schema_version: u32, pub run_id: Option<String>, pub game: String,
    pub games: usize, pub positions: usize,
    pub distinct_games: usize /* distinct action sequences */, pub distinct_game_fraction: f64,
    pub distinct_positions: usize /* distinct raw states over positions ∪ final states */, pub distinct_canonical_positions: usize,
    pub known_canonical_positions: Option<usize>, pub canonical_coverage: Option<f64> /* distinct_canonical / known */,
    pub decisive_games: usize /* outcome is a win */, pub decisive_fraction: f64,
    pub outcomes: OutcomeCounts,
    pub by_pairing: BTreeMap<String, OutcomeCounts> /* key = cell.strategies joined with " vs " */,
    pub by_length: BTreeMap<usize, usize>,
    pub by_ply: BTreeMap<usize, OutcomeCounts> /* positions at ply p, grouped by their game's outcome */,
    pub by_first_action_orbit: BTreeMap<usize, OutcomeCounts> /* empty when the game has no primitives */,
    pub thresholds: DiversityThresholds, pub diversity_pass: bool, pub diversity_failures: Vec<String>,
}
pub fn summarize<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, games: &[GameRecord<..>], positions: &[PositionRecord<..>], thresholds: &DiversityThresholds) -> CorpusSummary;
pub fn analyze_corpus<G>(bundle, corpus_dir: &Path, thresholds: &DiversityThresholds) -> Result<CorpusSummary, CorpusError>;   // reads run.json/games.jsonl/positions.jsonl, writes <corpus_dir>/summary.json
pub fn verify_corpus<G>(bundle, corpus_dir: &Path) -> Result<usize, CorpusError>;   // replays every game and checks games.jsonl ⇔ positions.jsonl consistency (state/legal/chosen/side/canonical per ply, final states); returns games verified
```

Outcome classification via `G::winner(&outcome)`: `Some(p)` → win for `format!("{p:?}")` and decisive; `None` → draw; record `outcome: None` → unfinished. Coverage denominators: `distinct_canonical_positions` counts the canonical states of every position record PLUS every game's `final_canonical_state` (so terminal states count toward the known 765). `by_first_action_orbit`: `primitives.action_position(&actions[0])` → `symmetry_group().orbit_index()[p]` (compute the index vector once); games with no actions or no position are skipped. `diversity_failures` lists human-readable strings like `"canonical_coverage 0.31 < 0.50"`; `diversity_pass = failures.is_empty()`. All fractions are `f64` computed as `numerator as f64 / denominator as f64` (0.0 when the denominator is 0).

**`src/cli/mod.rs` — minimal stage commands (Phase 6 restructures freely; this is the floor the Phase 5 acceptance check needs).**

```rust
#[derive(Parser)] #[command(name = "strategy-discovery", version, about)] pub struct Cli { #[command(subcommand)] pub command: Option<Command> }
#[derive(Subcommand)] pub enum Command {
    /// Generate a corpus from a TOML config into an output directory.
    Generate { #[arg(long)] config: PathBuf, #[arg(long)] out: PathBuf, #[arg(long)] threads: Option<usize>, #[arg(long)] serial: bool },
    /// Annotate a corpus (--corpus DIR) or every reachable position of a game (--exhaustive --game NAME --out DIR).
    Annotate { #[arg(long)] corpus: Option<PathBuf>, #[arg(long)] exhaustive: bool, #[arg(long)] game: Option<String>, #[arg(long)] out: Option<PathBuf>, #[arg(long)] engine_depth: Option<u32> },
    /// Summarize a corpus into summary.json; --strict exits 2 when diversity thresholds fail.
    Analyze { #[arg(long)] corpus: PathBuf, #[arg(long)] min_coverage: Option<f64>, #[arg(long)] min_decisive: Option<f64>, #[arg(long)] min_distinct: Option<f64>, #[arg(long)] strict: bool },
}
pub fn run() -> anyhow::Result<()> { run_from(std::env::args_os()) }
pub fn run_from<I, T>(args: I) -> anyhow::Result<()> where I: IntoIterator<Item = T>, T: Into<OsString> + Clone;
```

No subcommand → print `"{NAME} {VERSION}"` and return `Ok` (preserves the Phase 1 behaviour). Game dispatch: `generate` reads `config.game`; `annotate --corpus` reads `run.json`'s `game`; `annotate --exhaustive` needs `--game` and `--out`; `"tictactoe"` → `games::tictactoe::game_bundle()`, anything else → `anyhow::bail!("unknown game `{name}`; known games: tictactoe")`. stdout, one line each: `generate` → `run_id=<hex> cells=<n> games=<n> positions=<n> out=<dir>`; `annotate` → `mode=<corpus|exhaustive> annotated=<n> terminal=<n> disagreements=<n>`; `analyze` → `games=<n> distinct_games=<n> canonical_coverage=<f> decisive_fraction=<f> diversity_pass=<bool>`. `--strict` with `diversity_pass == false` → `std::process::exit(2)` after printing. `main.rs` unchanged. `tests/smoke.rs::cli_bootstrap_runs` must become `assert!(strategy_discovery::cli::run_from(["strategy-discovery"]).is_ok())` (the test harness's argv must never reach clap). `tracing` remains optional/non-behavioural (Phase 6 adds logging).

**Fixture configs.** `tests/fixtures/generate-small.toml`: `game = "tictactoe"`, `seed = 20260821`, `games_per_cell = 4`, strategies `random` (`kind = "random"`), `depth-2` (`minimax`, depth 2), `perfect` (`minimax`, depth 9); pairings omitted (⇒ 9 ordered pairs); evaluators `["default"]`; openings `none` (omitted actions) + `center` (`actions = [4]`); `random_opening_plies = [0, 2]` ⇒ 36 cells × 4 = 144 games. `tests/fixtures/generate-draws.toml`: one strategy `perfect-engine` (`minimax`, depth 9, `tie_break = "engine"`), pairings `[["perfect-engine","perfect-engine"]]`, `games_per_cell = 16` — every game identical (the plan's "corpus of identical perfect-play draws" failure mode; diversity MUST fail on it). `configs/tictactoe-default.toml`: `seed = 20260821`, `games_per_cell = 20`, everything else defaulted (7 roster strategies ⇒ 49 ordered pairs ⇒ 980 games; the Gate B evidence runs scale `games_per_cell` to 200 ⇒ 9800 games). The decomposer may adjust fixture sizes to meet the test-time budget but must keep the structural properties above. TOML shape reminder (`[[strategies]]` with a nested `[strategies.spec]` table carrying `kind = "..."`, `pairings = [["a","b"]]`, `[[openings]]` with `actions = [4]`) — the decomposer must verify the exact TOML spelling round-trips through `GenerateConfig<Move>` before writing step files (the Phase 4 precedent is `roster_round_trips_json_and_toml`).

**Tests (names are binding; the decomposer may add more).**

`tests/corpus.rs` (all via `game_bundle()`, fixture configs read from `tests/fixtures/`, outputs under `env!("CARGO_TARGET_TMPDIR")` in per-test subdirectories, removed-then-created at test start):

- `same_seed_is_byte_identical`: generate `generate-small.toml` twice into two directories → `run.json`, `games.jsonl`, `positions.jsonl` byte-equal (`std::fs::read`).
- `serial_equals_parallel`: `GenerateOptions { serial: true }` vs `threads: Some(4)` vs default → all three output sets byte-equal.
- `defaults_spelled_out_produce_same_run_id`: the small config with `pairings`/`evaluators`/`random_opening_plies` explicitly listed equals the defaulted config byte-for-byte.
- `positions_equal_replay_of_games`: `verify_corpus` passes; additionally an independent check that every `PositionRecord.state` equals `replay(rules, &game.actions)[ply].0`, `legal_actions` equals `rules.legal_actions`, `canonical_state` equals `canonicalizer.canonicalize(state)`, and `primitives.transform(state, Permutation::new(canonical_transform)) == canonical_state`.
- `openings_are_honoured`: cells with opening `center` have `actions[0] == Move(4)` and ply-0 `chosen_by == "opening"`; cells with `random_opening_plies == 2` have plies 0-1 `chosen_by == "random-opening"` (after any fixed prefix) and, across the cell's games, at least 2 distinct first actions for the `none` opening; `opening_plies` equals fixed + random (or the game length if shorter).
- `seeds_follow_the_documented_formulas`: `GameRecord.seed == game_seed(cell_seed(config.seed, cell.index), game_index)`; `cell_seed` values distinct across cells.
- `diversity_thresholds_pass_across_seeds`: for seeds `{1, 2, 3}` over `generate-small.toml` with the seed overridden, `summarize` with thresholds `{ min_canonical_coverage: 0.25, min_decisive_fraction: 0.2, min_distinct_game_fraction: 0.5 }` passes (the decomposer re-tunes these fixture thresholds only downward if measurement shows the small corpus cannot reach them; the default thresholds are for the full-size evidence runs).
- `diversity_catches_identical_perfect_play`: `generate-draws.toml` → `distinct_games == 1`, `decisive_games == 0`, `diversity_pass == false`, `diversity_failures.len() >= 2`.
- `schema_version_mismatch_is_an_error`: a hand-written `positions.jsonl` line with `"schema_version": 99` → `read` fails with `IoError::SchemaVersion { expected: 1, found: 99, .. }`.
- `invalid_configs_are_rejected`: unknown strategy in a pairing, unknown evaluator, `games_per_cell = 0`, illegal opening (`actions = [4, 4]`), unknown field (deny_unknown_fields), wrong `schema_version` — each a `CorpusError::Config`/`Io` with a message naming the item.

`tests/annotate.rs`:

- `exhaustive_annotation_covers_all_positions`: `annotate_exhaustive` → 5478 records, 958 `terminal`, `disagreements == 0`, label distribution X wins 2936 / draw 1068 / O wins 1474 (labels computed from `side_to_move` + `value` as defined under solver), `annotate.json` has `mode == "exhaustive"`, `engine_depth == 9`; running it twice is byte-identical.
- `corpus_annotation_labels_every_distinct_state`: generate `generate-small.toml`, `annotate_corpus` → `annotated` equals the number of distinct `state`s in `positions.jsonl`, records are in first-appearance order, every `optimal_actions ⊆ legal_actions` of a matching position, `disagreements == 0`, `run_id` matches `run.json`.
- `solver_facts`: the `Solved` expectations listed under solver (values/optimal sets for the three fixture boards and the empty board; `reachable_states` equals `TicTacToeRules::reachable_positions()` element-for-element; `TooLarge` with `limit = 10`).

`tests/cli_pipeline.rs` (spawns `env!("CARGO_BIN_EXE_strategy-discovery")` with `std::process::Command`): `generate --config tests/fixtures/generate-small.toml --out <tmp>` exit 0 and the three files exist and stdout starts with `run_id=`; `annotate --corpus <tmp>` exit 0, `annotations.jsonl` + `annotate.json` exist; `analyze --corpus <tmp>` exit 0 and `summary.json` exists; `analyze --corpus <draws-tmp> --strict` exits 2; `annotate --exhaustive --game tictactoe --out <tmp2>` exit 0; no subcommand → exit 0 and stdout `strategy-discovery 0.1.0`; `generate --config missing.toml --out x` → non-zero exit with an error message on stderr.

`tests/corpus_bench.rs` (non-failing, `#[test]`, prints `[bench]` lines): `configs/tictactoe-default.toml` generated serially and in parallel → `[bench] corpus games/s serial=… parallel=… positions=… write_ms=…`; `analyze_corpus` → `[bench] summary read+aggregate ms=…`. Assert only completion. In debug this must stay well under the budget (use `games_per_cell = 20` → 980 games; depth-9 self-play is ≈150 games/s release, far slower in debug — if a debug run of this bench exceeds ~15 s, the decomposer scales the bench config down and documents it in the file's doc comment, as Phase 4 learned with `match_bench`).

**Gate B evidence (produced by CLI runs on the dev machine in release; artifacts carry the Phase 3 header table).**

- `artifacts/benchmarks/corpus-generation.md` + `.json`: `configs/tictactoe-default.toml` with `games_per_cell` raised to 200 (9800 games) — wall seconds and games/s for `--serial` and default parallel, positions written, file sizes (`games.jsonl`, `positions.jsonl`), `annotate --corpus` wall, `analyze` wall; compare against the Phase 3 numbers quoted in Context (JSONL write 51.6 ms / read 58.5 ms / aggregation 71.2 ms at 76409 records — same order of magnitude expected).
- `artifacts/reproducibility/corpus-determinism.md`: SHA-256 (`Get-FileHash -Algorithm SHA256`) of every output file for (a) two runs with the same seed, (b) `--serial` vs `--threads 4` vs default, (c) two `annotate` runs, (d) two `annotate --exhaustive` runs — all identical: YES.
- `artifacts/reproducibility/corpus-diversity.md`: default config (`games_per_cell = 20`) at seeds 1, 2, 3 → table of `distinct_games`, `distinct_game_fraction`, `distinct_canonical_positions`, `canonical_coverage`, `decisive_fraction`, `diversity_pass` against the default thresholds (0.5 / 0.2 / 0.5) — all PASS; plus the `generate-draws.toml` row showing FAIL (the negative control).
- `docs/component-evaluation.md` § "Gate B" (same table format as § "Gate A"): rows for each `docs/plan.md` Phase 5 acceptance check and the Gate B definition (seeded reproducibility + diversity thresholds), each PASS with the evidence path; plus a row confirming selected components (JSONL, serde aggregation, exhaustive solver + game-player cross-check) meet thresholds at 9800 games; "Gate B verdict: PASS".
- `docs/adr/0009-corpus-record-schema.md`: Status / Context / Decision / Consequences; records schema v1 (the three record types, `CorpusGame` bounds, raw-state annotation key, denormalized outcome), the run directory layout and file names, the byte-identity rule, cell/opening seed formulas (`cell_seed`, `opening_seed` with constants; cross-reference ADR 0002/0008), the exhaustive-solver limit as the small-game accelerant flag, and the compatibility rule (bump `SCHEMA_VERSION`, readers reject mismatches).
- `README.md` Usage: replace the placeholder commands with the real `generate`/`annotate`/`analyze` invocations and the run-directory file list; link ADR 0009.

## Constraints

- Toolchain: stable Rust 1.94.1, edition 2024; dev machine Windows 11 (PowerShell-compatible commands in step files: plain `cargo ...`, `Get-FileHash`), CI also Ubuntu — all code platform-neutral (use `Path`/`PathBuf`, never string paths with separators; line endings `\n`).
- Root crate must pass at every merged commit: `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Root `Cargo.toml` and `rustfmt.toml` byte-identical to `e4e0a7e` — no new dependencies or dev-dependencies (`tempfile`, `assert_cmd`, `sha2`, `polars`, `parquet`, `arrow`, `criterion`, `proptest`: none). `game-player` pin unchanged; no features enabled. Temp dirs: `env!("CARGO_TARGET_TMPDIR")` in integration tests, `std::env::temp_dir()` + process id + test name in unit tests.
- `src/core` may change only additively (nothing planned; if an implementer believes a core change is unavoidable it is a deviation to report, not a silent edit). Existing public signatures in `strategy`/`discovery` unchanged; `play_game`, `RayonMatchEngine` constructors, seed formulas, `MatchRecord` unchanged. Phase 4 tests (`tests/match_engine.rs`, `tests/minimax_tactics.rs`, `tests/match_bench.rs`) are not edited.
- `src/core`, `src/io`, `src/discovery`, `src/strategy` contain no tic-tac-toe references outside `#[cfg(test)]` code. `src/cli` is the only production code that maps a game name to a bundle. All tic-tac-toe production code under `src/games/tictactoe/`.
- `src/` never depends on `spikes/`. Spikes, `docs/plan.md`, `rustfmt.toml`, CI workflows are not modified.
- Every RNG is `ChaCha8Rng` seeded from a `u64` derived by the documented formulas; no `rand::rng()`, no `HashMap` iteration feeding any output (lookup-only `HashMap`/`HashSet` is fine), no time- or thread-dependent behaviour on the success path, nothing environment-dependent in any output file (see Byte-identity rule).
- Debug-build `cargo test` wall time: the whole new suite adds ≤ ~30 s on the dev machine (current total ≈ 25 s, dominated by `match_bench` 17.5 s). Fixture corpora stay small; exhaustive 5478-position work is allowed (solver ≈ ms, depth-9 searches ≈ 1.4 s debug). Tests may be `#[ignore]`d only with an explicit justification, and the gate step must run them with `-- --ignored`.
- `unsafe` forbidden. Public items documented (`///`); module docs (`//!`) on every new file. `cargo doc --no-deps` must build without warnings (CI builds docs on `master`).
- JSONL/JSON only (ADR 0007); no Parquet, no binary formats. TOML only as config input (ADR 0003).
- Exhaustive modes (`annotate --exhaustive`, `ExhaustiveSolver`) are explicitly flagged as small-game accelerants in docs and error messages (plan § Further Considerations item 4); nothing in `generate`/`analyze` depends on exhaustive enumeration.

## Assumptions

- The working branch `feature/phase-5-corpus` (= `develop` at `7e1026e` + the plan-documents commit `e4e0a7e`) is where all commits accumulate; merging it back to `develop` is the user's manual step, outside this pipeline.
- `toml 1.1.4` deserializes `GenerateConfig<Move>` from the fixture shape described (internally tagged `StrategySpec` inside `[[strategies]]`/`[strategies.spec]` tables, `Vec<Vec<String>>` pairings, `Vec<Move>` as an integer array). If the nested-table spelling fails, the fallback is a flat `[[strategies]]` table with `name` + `kind` + spec fields flattened — the decomposer verifies before writing steps.
- The exhaustive solver and `search` at depth 9 agree on every tic-tac-toe position (0 disagreements — Phase 3 and Phase 4 evidence). If a disagreement appears, the supervisor records the positions verbatim in the ledger and escalates; nobody weakens the assertion.
- A 144-game fixture corpus reaches ≥ 25 % canonical coverage, ≥ 20 % decisive games and ≥ 50 % distinct games across seeds 1-3 (random and depth-2 players explore widely). If measurement disproves this, the fixture thresholds (not the default thresholds) are lowered and the measured values recorded in the ledger.
- The default config (7 roster strategies, 49 ordered pairs, 20 games/cell) passes the default thresholds 0.5 / 0.2 / 0.5 at seeds 1-3 — the Gate B diversity claim. If it fails, the remedy is a richer default config (add `zero` evaluator / random opening plies to `configs/tictactoe-default.toml`), never a weaker threshold without recording why.
- Phase 6 will replace/extend `src/cli` (config validation layer, logging, error taxonomy, `play`, `report`); Phase 5's CLI is deliberately minimal and its flag names may change then.

## Out of scope

- `play` and `report` subcommands, analyzer registry, typed config validation layer beyond `GenerateConfig` checks, structured logging/verbosity, error taxonomy — Phase 6.
- Tournament harness, agreement metrics against annotations, strategy archive, property-based tests, second-game onboarding docs — Phase 7.
- Feature-vector datasets, rule mining, `linfa`, concept induction — Phases 8-9.
- Parquet export, `polars`, any new dependency.
- Per-position feature values in records (Phase 8 derives them from `state`).
- Interactive play; modifying `game-player` upstream or bumping its pin; MCTS.
- Compact/custom serde representations for `Board` (records use the existing derived serde shape).
- Changing seed formulas or match-engine semantics established in Phase 4.

## Definition of Done (project)

All checked on branch `feature/phase-5-corpus` at the final commit:

1. `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps` all exit 0; `git diff e4e0a7e -- Cargo.toml rustfmt.toml` is empty; `git diff e4e0a7e -- src/core tests/match_engine.rs tests/minimax_tactics.rs tests/match_bench.rs docs/plan.md .github` is empty.
2. `src/io/{hash,jsonl,replay,schema}.rs`, `src/discovery/{solver,bundle,config,corpus,annotate,summary}.rs`, `src/games/tictactoe/corpus.rs`, `configs/tictactoe-default.toml`, `tests/fixtures/generate-{small,draws}.toml` exist and are wired (`pub mod`) and documented; `src/strategy/engine.rs` has `ConstantEvaluator`; `src/discovery/match_engine.rs` has `Opening`, `opening_seed`, `play_game_with_opening`, `RayonMatchEngine::with_opening`; `src/cli/mod.rs` has `Generate`/`Annotate`/`Analyze`.
3. `cargo test --test corpus` passes and includes `same_seed_is_byte_identical`, `serial_equals_parallel`, `defaults_spelled_out_produce_same_run_id`, `positions_equal_replay_of_games`, `openings_are_honoured`, `seeds_follow_the_documented_formulas`, `diversity_thresholds_pass_across_seeds`, `diversity_catches_identical_perfect_play`, `schema_version_mismatch_is_an_error`, `invalid_configs_are_rejected`.
4. `cargo test --test annotate` passes and includes `exhaustive_annotation_covers_all_positions` (5478 / 958 terminal / 0 disagreements / 2936-1068-1474), `corpus_annotation_labels_every_distinct_state`, `solver_facts`.
5. `cargo test --test cli_pipeline` passes: `generate` → `annotate` → `analyze` via the built binary emit `run.json`, `games.jsonl`, `positions.jsonl`, `annotations.jsonl`, `annotate.json`, `summary.json`; `--strict` exits 2 on the draws fixture; no-subcommand prints name + version. `tests/smoke.rs` uses `run_from`.
6. `cargo test --release --test corpus_bench -- --nocapture` prints `[bench]` games/s (serial + parallel) and summary timing lines and completes.
7. Gate B evidence exists: `artifacts/benchmarks/corpus-generation.{md,json}`, `artifacts/reproducibility/corpus-determinism.md` (all hashes identical: YES), `artifacts/reproducibility/corpus-diversity.md` (seeds 1-3 PASS against 0.5 / 0.2 / 0.5; draws fixture FAIL); `docs/component-evaluation.md` has a `## Gate B` section ending in `Gate B verdict: PASS`; `docs/adr/0009-corpus-record-schema.md` exists with Status / Context / Decision / Consequences; `README.md` Usage shows the real commands.
8. Gate statement: `implementation-artifacts/phase5-corpus-gate-report.md` lists each `docs/plan.md` Phase 5 acceptance check (lines 143-148 at `e4e0a7e`) and the Gate B definition (line 274) with PASS and the test name / command / artifact that evidences it.
