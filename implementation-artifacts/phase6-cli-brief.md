# phase6-cli — Brief

## Goal

Execute Phase 6 of `docs/plan.md` ("CLI and config-driven orchestration"): one CLI command per pipeline stage with file handoffs (`play`, `generate`, `annotate`, `analyze`, `report`, plus a config-driven `pipeline` runner), `analyze` as a pluggable analyzer registry with at least two registered analyzers, strongly-typed experiment configuration with validation, structured logging with verbosity controls, and an error taxonomy with documented exit codes and actionable messages. Authoritative spec: `docs/plan.md` lines 150-169 (Phase 6) at commit `f0b633fe6093a76b7d7d92c3965caed800b1f60b` (`docs/plan.md` unchanged since `e4e0a7e`); line numbers shift if that file is edited — re-verify against live source.

## Context

### Repo state at planning time (commit `f0b633fe6093a76b7d7d92c3965caed800b1f60b`, branch `feature/phase-6-cli`, branched from `develop`)

- `develop` = `f0b633f` ("Implemented phase 5: Corpus generation, annotation, and persistence.", the Phase 5 squash). Working branch `feature/phase-6-cli` created at the same commit; the plan documents (this brief, roadmap, ledger) will be committed on top of it. Plain feature branch off `develop`; PR back into `develop` when Phase 6 completes (gitflow per `CLAUDE.md`). Remote default branch: `origin/develop`.
- Single Rust crate `strategy-discovery` 0.1.0 (lib + thin bin), edition 2024, stable `rustc 1.94.1 (e408947bf 2026-03-25)`, `cargo 1.94.1`. No `[workspace]` table. No `[dev-dependencies]`. Dependencies (`Cargo.toml`): `anyhow 1.0.104`, `clap 4.6.6` (`derive`), `game-player` (git `jambolo/game-player`, branch `master`, lock `0b8ce6f`, v0.4.0), `rand 0.10.2`, `rand_chacha 0.10.0`, `rayon 1.12.0`, `serde 1.0.229` (`derive`), `serde_json 1.0.151`, `thiserror 2.0.20`, `toml 1.1.4`, `tracing 0.1.44`, `tracing-subscriber 0.3.23` (DEFAULT features only — `env-filter` is NOT enabled; `Cargo.lock` shows no `matchers`/`regex` under it). `tracing`/`tracing-subscriber` are declared but UNUSED anywhere in `src/` (`grep -rn tracing src` is empty).
- `rustfmt.toml`: `max_width = 132`. `core.autocrlf = true` locally — never byte-compare checked-out text fixtures; normalize CRLF→LF before string-replacing fixture text (a `\n`-anchored replace silently no-ops otherwise).
- Baseline verification at `f0b633f`, debug profile: `cargo test` = lib 253 + annotate 3 + cli_pipeline 6 + corpus 10 + corpus_bench 1 + match_bench 2 + match_engine 9 + minimax_tactics 7 + smoke 2 + ttt_bench 2 = **295 passed, 0 failed**; debug wall ≈ 32 s (`match_bench` alone 17.7 s). `cargo fmt --all --check` exit 0. `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean. CI (`.github/workflows/ci.yml`): build + test on ubuntu/windows for pushes to `master`/`develop`/`release/**`; fmt + clippy on pull requests; `cargo doc --no-deps --workspace` on master.
- Git Bash on this machine has NO `python` on PATH. Edit files with the Edit tool or `sed`. One cargo command per worker tool call (long chained commands stall workers).

### Source tree (at `f0b633f`; line counts)

```text
src/main.rs                      11   ExitCode::SUCCESS / FAILURE around cli::run(); prints "error: {err:#}" to stderr
src/lib.rs                       17   pub mod cli, core, discovery, games, io, strategy; NAME, VERSION consts
src/cli/mod.rs                  251   clap Cli/Command (Generate, Annotate, Analyze); run(), run_from(); game dispatch by string match (4 copies)
src/core/{traits,symmetry,features,derived,dsl,interpreter}.rs   game-agnostic framework (unchanged by Phase 6)
src/strategy/{engine,minimax,random,scripted,registry,roster}.rs  StrategySpec/StrategyRegistry/EngineBundle, MinimaxConfig, RosterEntry/Roster
src/discovery/mod.rs             20   pub mod + re-exports (see below)
src/discovery/match_engine.rs   762   RayonMatchEngine, Opening, play_game(_with_opening), seeds
src/discovery/solver.rs         275   ExhaustiveSolver, reachable_states
src/discovery/bundle.rs          81   GameBundle<G>
src/discovery/config.rs         711   GenerateConfig<A>, Pairing, NamedOpening<A>, Cell<A>, ResolvedConfig<A>, CorpusError, resolve(), cell_seed()
src/discovery/corpus.rs         367   GenerateOptions, RunMetadata<A,P>, generate(), expand_game()
src/discovery/annotate.rs       300   AnnotateMode, AnnotateOptions, AnnotateMetadata, annotate_states/annotate_corpus/annotate_exhaustive, DEFAULT_SOLVER_LIMIT
src/discovery/summary.rs        614   OutcomeCounts, DiversityThresholds, CorpusSummary, summarize(), analyze_corpus(), verify_corpus(), private load_corpus()
src/io/{mod,hash,jsonl,replay,schema}.rs   IoError, fnv1a64/config_hash/hex16, JsonlWriter/JsonlReader/read_json/read_jsonl/write_json_pretty/write_jsonl, replay/final_state, schema consts + records
src/games/tictactoe/{mod,board,rules,eval,engine,features,primitives,canonical,roster,corpus}.rs   Board (impl Display: 3 rows of `X`/`O`/`.`; Board::parse), Move(pub usize) (serializes as bare integer), Player {X,O}, Outcome {Win(Player), Draw}, game_bundle(), engine_bundle(), benchmark_roster()
configs/tictactoe-default.toml        schema_version=1, game="tictactoe", seed=20260821, games_per_cell=20 (everything else defaulted → 49 cells, 980 games)
tests/fixtures/generate-small.toml    36 cells × 4 games = 144 games (random / depth-2 / perfect[tie_break="engine"], evaluators ["default"], openings none+center(4), random_opening_plies [0,2])
tests/fixtures/generate-small-explicit.toml   same with the 9 pairings spelled out (resolves identical, same run_id)
tests/fixtures/generate-draws.toml    1 cell × 16 games, perfect-engine vs perfect-engine (all identical draws; negative diversity control)
tests/{annotate,cli_pipeline,corpus,corpus_bench,match_bench,match_engine,minimax_tactics,smoke,ttt_bench}.rs
docs/adr/0001..0009                   0009 = corpus record schema + run-directory layout (THE contract; see below)
docs/component-evaluation.md          Gate A/B records (historical evidence; do not edit in Phase 6)
artifacts/benchmarks/*, artifacts/reproducibility/*   Phase 3/5 evidence (historical; do not edit)
README.md                             Usage section documents generate/annotate/analyze; says "`report` arrives with Phase 6"
```

### Current CLI contract (at `f0b633f`, `src/cli/mod.rs`)

- `Cli { #[command(subcommand)] command: Option<Command> }`, `#[command(name = "strategy-discovery", version, about)]`. No subcommand → prints `strategy-discovery 0.1.0` to stdout, exit 0 (`tests/smoke.rs::cli_bootstrap_runs` calls `cli::run_from(["strategy-discovery"])`; `tests/cli_pipeline.rs::no_subcommand_prints_name_and_version` asserts the exact banner).
- `generate --config <toml> --out <dir> [--threads N] [--serial]` → `run_generate`; stdout: `run_id={} cells={} games={} positions={} out={}`.
- `annotate --corpus <dir> [--engine-depth D]` or `annotate --exhaustive --game <name> --out <dir> [--engine-depth D]`; stdout: `mode={corpus|exhaustive} annotated={} terminal={} disagreements={}`.
- `analyze --corpus <dir> [--min-coverage F] [--min-decisive F] [--min-distinct F] [--strict]`; stdout: `games={} distinct_games={} canonical_coverage={:.3} decisive_fraction={:.3} diversity_pass={}`; `--strict` + `diversity_pass=false` → `std::process::exit(2)`.
- Game dispatch: `struct GameName { game: String }` deserialized from the config TOML (`generate`) or `run.json` (`annotate`, `analyze`); `match header.game.as_str() { "tictactoe" => ..., other => Err(unknown_game(other)) }` repeated in FOUR places; `unknown_game` message: `unknown game `{name}`; known games: tictactoe`.
- Errors: every command returns `anyhow::Result<()>`; `main.rs` prints `error: {err:#}` (anyhow alternate = cause chain joined by `: `) and exits 1. clap usage errors exit 2 (clap's own behavior).
- `tests/cli_pipeline.rs` spawns the binary via `env!("CARGO_BIN_EXE_strategy-discovery")`, writes under `env!("CARGO_TARGET_TMPDIR")`, and asserts the stdout prefixes above plus `strict_analyze_exits_two_on_undiverse_corpus` (exit code 2 — CHANGES to 3 in Phase 6, see Decisions).

### Library API the CLI composes (exact signatures at `f0b633f`)

```rust
// src/discovery/bundle.rs
pub struct GameBundle<G: EngineGame> { pub name: String, pub rules: Arc<dyn GameRules<G>>, pub players: Vec<G::Player>,
    pub evaluators: BTreeMap<String, Arc<dyn StateEvaluator<G>>>, pub default_evaluator: String,
    pub canonicalizer: Option<Arc<dyn Canonicalize<G>>>, pub primitives: Option<Arc<dyn GamePrimitives<G>>>,
    pub default_strategies: Vec<RosterEntry>, pub full_search_depth: u32, pub known_canonical_positions: Option<usize> }
impl<G: EngineGame> GameBundle<G> { pub fn engine_bundle(&self, evaluator: &str) -> Result<EngineBundle<G>, CorpusError>;
    pub fn canonicalize(&self, state: &G::State) -> (G::State, Vec<usize>); pub fn player_slot(&self, player: G::Player) -> Option<usize>; }
// tictactoe bundle: name "tictactoe", players [X, O], evaluators {"default", "zero"}, default_strategies = benchmark_roster().entries
//   = ["random", "depth-1", "depth-2", "depth-3", "depth-4", "depth-6", "perfect"] (perfect = minimax depth 9, seeded-uniform tie-break), full_search_depth 9, known_canonical_positions Some(765)

// src/discovery/config.rs
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(deny_unknown_fields)]
pub struct GenerateConfig<A> { pub schema_version: u32, pub game: String, pub seed: u64, pub games_per_cell: usize,
    #[serde(default)] pub max_plies: Option<usize>, #[serde(default = "Vec::new")] pub strategies: Vec<RosterEntry>,
    #[serde(default = "Vec::new")] pub pairings: Vec<Pairing>, #[serde(default = "Vec::new")] pub evaluators: Vec<String>,
    #[serde(default = "Vec::new")] pub openings: Vec<NamedOpening<A>>, #[serde(default = "Vec::new")] pub random_opening_plies: Vec<u32> }
impl<A: DeserializeOwned> GenerateConfig<A> { pub fn from_toml_str(text: &str) -> Result<Self, CorpusError>; pub fn from_toml_file(path: &Path) -> Result<Self, CorpusError>; }
pub struct Pairing(pub Vec<String>);                       // TOML: pairings = [["random","perfect"]]
pub struct NamedOpening<A> { pub name: String, pub actions: Vec<A> }   // TOML: [[openings]] name = "center" actions = [4]
pub struct Cell<A> { pub key: CellKey, pub entries: Vec<RosterEntry>, pub opening: Opening<A>, pub seed: u64 }
pub struct ResolvedConfig<A> { pub config: GenerateConfig<A>, pub cells: Vec<Cell<A>> }
pub fn resolve<G: EngineGame + CorpusGame>(config: GenerateConfig<G::Action>, bundle: &GameBundle<G>) -> Result<ResolvedConfig<G::Action>, CorpusError>;
pub fn cell_seed(master: u64, cell_index: usize) -> u64;
#[derive(Debug, Error)] pub enum CorpusError { #[error("invalid configuration: {0}")] Config(String), Match(#[from] MatchError),
    Strategy(#[from] StrategyError), Io(#[from] IoError), Rules(#[from] RulesError), Solver(#[from] SolverError) }  // non-Config variants are #[error(transparent)]
// resolve() validates: schema_version == 1, game == bundle.name, games_per_cell > 0, strategy names non-empty/unique and buildable,
// pairings length == players.len() and names known, evaluators known, opening names non-empty/unique and legal sequences, plies unique.
// Cell order: pairings → evaluators → openings → random_opening_plies (outer→inner), index from 0.

// src/discovery/corpus.rs
#[derive(Debug, Clone, Default)] pub struct GenerateOptions { pub threads: Option<usize>, pub serial: bool }
pub struct RunMetadata<A, P> { pub schema_version: u32, pub run_id: String, pub config_hash: String, pub crate_name: String, pub crate_version: String,
    pub game: String, pub players: Vec<P>, pub config: GenerateConfig<A>, pub cells: Vec<CellKey>, pub games: usize, pub positions: usize }
pub fn generate<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, config: GenerateConfig<G::Action>, options: &GenerateOptions, out_dir: &Path)
    -> Result<RunMetadata<G::Action, G::Player>, CorpusError>;
// generate(): resolve → run_id = config_hash(&resolved.config) → create_dir_all(out_dir) → JsonlWriter games/positions → per cell (sequential, in order):
//   StrategyRegistry per evaluator (cached in BTreeMap), providers = registry.build(entry.spec), players zip, engine = RayonMatchEngine::{serial|with_threads|new}(rules).with_opening(cell.opening),
//   records = engine.run(&MatchConfig { games: games_per_cell, seed: cell.seed, max_plies }, &players), expand_game per record, write → finish → write_json_pretty(run.json)
pub fn expand_game<G>(bundle, run_id: &str, cell: &Cell<G::Action>, record: &MatchRecord<G::Action, G::Outcome>) -> Result<(GameRecord<..>, Vec<PositionRecord<..>>), CorpusError>;

// src/discovery/annotate.rs
pub enum AnnotateMode { Corpus, Exhaustive }   // serde kebab-case
pub const DEFAULT_SOLVER_LIMIT: usize = 1_000_000;
pub struct AnnotateOptions { pub engine_depth: Option<u32>, pub solver_limit: usize }   // impl Default
pub struct AnnotateMetadata { pub schema_version: u32, pub mode: AnnotateMode, pub game: String, pub run_id: Option<String>, pub engine_depth: u32,
    pub evaluator: String, pub annotated: usize, pub terminal: usize, pub disagreements: usize, pub solver_states: usize }
pub fn annotate_corpus<G>(bundle: &GameBundle<G>, corpus_dir: &Path, options: &AnnotateOptions) -> Result<AnnotateMetadata, CorpusError>;
pub fn annotate_exhaustive<G>(bundle: &GameBundle<G>, out_dir: &Path, options: &AnnotateOptions) -> Result<AnnotateMetadata, CorpusError>;
// annotate.rs and summary.rs each read run.json through a PRIVATE `struct RunHeader { schema_version, game, run_id }` deserializer.

// src/discovery/summary.rs
pub struct DiversityThresholds { pub min_canonical_coverage: f64, pub min_decisive_fraction: f64, pub min_distinct_game_fraction: f64 }  // Default 0.5 / 0.2 / 0.5
pub struct CorpusSummary { schema_version, run_id: Option<String>, game, games, positions, distinct_games, distinct_game_fraction, distinct_positions,
    distinct_canonical_positions, known_canonical_positions: Option<usize>, canonical_coverage: Option<f64>, decisive_games, decisive_fraction,
    outcomes: OutcomeCounts, by_pairing: BTreeMap<String, OutcomeCounts>, by_length: BTreeMap<usize, usize>, by_ply: BTreeMap<usize, OutcomeCounts>,
    by_first_action_orbit: BTreeMap<usize, OutcomeCounts>, thresholds: DiversityThresholds, diversity_pass: bool, diversity_failures: Vec<String> }
pub struct OutcomeCounts { pub wins: BTreeMap<String, usize>, pub draws: usize, pub unfinished: usize, pub total: usize }
pub fn summarize<G>(bundle, games: &[GameRec<G>], positions: &[PosRec<G>], thresholds) -> CorpusSummary;
pub fn analyze_corpus<G>(bundle, corpus_dir: &Path, thresholds: &DiversityThresholds) -> Result<CorpusSummary, CorpusError>;   // writes summary.json
pub fn verify_corpus<G>(bundle, corpus_dir: &Path) -> Result<usize, CorpusError>;
// private: struct Loaded { header: RunHeader, games, positions }; fn load_corpus(bundle, dir) -> Result<Loaded, CorpusError> (checks schema_version, game == bundle.name)

// src/discovery/mod.rs re-exports
pub use match_engine::{RayonMatchEngine, game_seed, play_game, player_seed, splitmix64};
pub use annotate::{AnnotateMetadata, AnnotateMode, AnnotateOptions, annotate_corpus, annotate_exhaustive, annotate_states};
pub use bundle::GameBundle;
pub use config::{Cell, CorpusError, GenerateConfig, NamedOpening, Pairing, ResolvedConfig, cell_seed, resolve};
pub use corpus::{GenerateOptions, RunMetadata, expand_game, generate};
pub use solver::{ExhaustiveSolver, Solved, SolverError, reachable_states};
pub use summary::{CorpusSummary, DiversityThresholds, OutcomeCounts, analyze_corpus, summarize, verify_corpus};

// src/io
pub enum IoError { Io { path, source: std::io::Error }, Json { path, line, source }, Toml { path, message }, SchemaVersion { path, expected, found }, Missing { path }, Invalid(String) }
//   Display: "{path}: {source}" / "{path}:{line}: {source}" / "{path}: {message}" / "{path}: schema_version {found} does not match expected {expected}" / "{path}: missing" / "{0}"
pub const SCHEMA_VERSION: u32 = 1; RUN_FILE "run.json"; GAMES_FILE "games.jsonl"; POSITIONS_FILE "positions.jsonl"; ANNOTATIONS_FILE "annotations.jsonl"; ANNOTATE_FILE "annotate.json"; SUMMARY_FILE "summary.json"
pub trait CorpusGame: GameDomain<State/Action/Player/Outcome: Serialize + DeserializeOwned> (blanket impl)
pub struct CellKey { pub index: usize, pub strategies: Vec<String>, pub evaluator: String, pub opening: String, pub random_opening_plies: u32 }
pub struct GameRecord<S,A,P,O> { schema_version, run_id, game_id /* "{cell.index}:{game_index}" */, cell: CellKey, game_index, seed: u64, players: Vec<P>, specs: Vec<StrategySpec>,
    opening_plies: usize, actions: Vec<A>, outcome: Option<O>, length: usize, final_state: S, final_canonical_state: S }
pub struct PositionRecord<S,A,P,O> { schema_version, game_id, ply, side_to_move: P, state: S, canonical_state: S, canonical_transform: Vec<usize>, legal_actions: Vec<A>,
    chosen_action: A, chosen_by: String /* "opening" | "random-opening" | strategy entry name */, outcome: Option<O>, game_length: usize }
pub struct AnnotationRecord<S,A,P> { schema_version, state: S /* RAW state = join key */, canonical_state: S, side_to_move: P, terminal: bool, value: i8, optimal_actions: Vec<A>,
    engine_action: Option<A>, engine_agrees: Option<bool> }
pub fn read_json<T: DeserializeOwned>(path) -> Result<T, IoError>; read_jsonl<T>(path) -> Result<Vec<T>, IoError>; write_json_pretty<T: Serialize>(path, &T) -> Result<(), IoError> (pretty + trailing "\n"); write_jsonl; JsonlWriter::create(path) / .write(&T) / .finish() -> Result<usize>; JsonlReader
pub fn config_hash<T: Serialize>(value: &T) -> Result<String, IoError>;  // hex16(fnv1a64(serde_json::to_vec(value)))
pub fn check_schema_version(path: &Path, found: u32) -> Result<(), IoError>;

// src/strategy
pub struct RosterEntry { pub name: String, pub spec: StrategySpec }
#[serde(tag = "kind", rename_all = "kebab-case")] pub enum StrategySpec { Minimax(MinimaxConfig), Random, HeuristicRules { heuristic: HeuristicStrategy }, Evolutionary, Llm }
pub struct MinimaxConfig { pub depth: u32, pub epsilon: f64, pub tie_break: TieBreak }   // TieBreak::{SeededUniform (default), Engine}; serde kebab-case
pub struct StrategyRegistry<G: EngineGame>; ::new(EngineBundle<G>); .build(&StrategySpec) -> Result<Box<dyn StrategyProvider<G>>, StrategyError>; .register(kind, Factory<G>); .kinds()
pub enum StrategyError { NoLegalActions, Unimplemented { kind, detail }, Other(String) }
// src/core/traits.rs
pub struct MatchConfig { pub games: usize, pub seed: u64, pub max_plies: Option<usize> }
pub struct MatchRecord<A, O> { pub game_index: usize, pub seed: u64, pub actions: Vec<A>, pub outcome: Option<O> }
pub enum MatchError { MissingProvider(String), Strategy(StrategyError), Rules(RulesError) }
pub enum RulesError { IllegalAction(String), GameOver, ... }   // "game is over; no further actions accepted"
pub trait MatchEngine<G> { fn run(&self, config: &MatchConfig, players: &[(G::Player, &dyn StrategyProvider<G>)]) -> Result<Vec<MatchRecord<G::Action, G::Outcome>>, MatchError>; }
// src/discovery/match_engine.rs
pub struct Opening<A> { pub actions: Vec<A>, pub random_plies: u32 }  // impl Default manually
RayonMatchEngine::new(rules) | ::with_threads(rules, n) | ::serial(rules); .with_opening(Opening) -> Self; .opening(); .threads(); .is_parallel()
pub fn opening_plies_played<A, O>(record: &MatchRecord<A, O>, opening: &Opening<A>) -> usize;
```

### Pinned Phase 5 outputs (debug binary at `f0b633f`; regression pins for Phase 6 — the byte-identity rule means these MUST NOT change)

| config | run_id | cells | games | positions | file SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `tests/fixtures/generate-small.toml` | `5eafa65f76637df3` | 36 | 144 | 1083 | run.json `a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1` (8415 B); games.jsonl `d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d` (86479 B); positions.jsonl `cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7` (403892 B) |
| same, after `annotate --corpus` | — | — | annotated 390, terminal 0, disagreements 0, solver_states 5478 | — | annotations.jsonl `9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3` (113018 B); annotate.json `65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e` (232 B) |
| same, after `analyze --corpus` (default thresholds) | — | — | games 144, distinct_games 115, canonical_coverage 0.312, decisive_fraction 0.549, diversity_pass false | — | summary.json `5794b37e9fe1f3ee786092160298a9435092c47dc4dc9a2b233d0c49e93d24d5` (3788 B) |
| `tests/fixtures/generate-draws.toml` | `933a94e742497e81` | 1 | 16 | 144 | run.json `cb93a327f1c960bc8805e382af4295ae1d7a6bfd3bd1eb4509579a4af0dbf37a`; games.jsonl `7fd0c779a944f0fcd550a2fdddd19a1ac69d227a441abb66b00516df44ba4b79`; positions.jsonl `f24904a57f2da4e4cd040f7848ba38045997b39386ce9aaf140478c4fbc3b69c`; analyze: distinct_games 1, decisive_fraction 0.000, diversity_pass false |
| `configs/tictactoe-default.toml` | `74bba44805e514a7` | 49 | 980 | 7491 | (Gate B at games_per_cell 200 was `dea7db96340e923d`) |

These hashes are of files written by the binary (`\n` terminators; unaffected by autocrlf). Debug generation of `generate-small.toml` ≈ 0.5 s; exhaustive annotation ≈ 0.5 s; `annotate --corpus` on it ≈ 0.5 s.

### Known pitfalls carried from Phases 4-5

- `toml 1.1.4`: top-level scalar/array keys MUST precede any `[[strategies]]`/`[[openings]]`/`[section]` table in a TOML document; a top-level key after a table silently joins that table. `deny_unknown_fields` errors and unknown `kind` surface as `toml` errors whose message names the offending token.
- serde: a bare `#[serde(default)]` on a field whose type mentions a type parameter `A` adds an `A: Default` bound; use `#[serde(default = "Vec::new")]` (or a named fn) on such fields. `Move` has no `Default`.
- `rand 0.10`: `random`/`random_range` live on `rand::RngExt`, not `rand::Rng`.
- Byte-identity rule (ADR 0009): no timestamps, durations, hostnames, thread counts, absolute paths, git SHAs, or `HashMap` iteration order in ANY persisted output; durations/progress go to stderr only.
- Board fixtures must satisfy `Board::parse` (`#X == #O` ⇒ X to move; `#X == #O + 1` ⇒ O to move).
- `tests/cli_pipeline.rs` style for CLI tests: spawn `env!("CARGO_BIN_EXE_strategy-discovery")`, isolate under `env!("CARGO_TARGET_TMPDIR")/<test-name>` (remove + create), return `(code, stdout, stderr)`.
- Debug `cargo test` wall budget: ≤ 45 s total (currently ≈ 32 s; `match_bench` 17.7 s is fixed cost). Size new CLI tests with the small fixtures (`generate-small.toml`, `generate-draws.toml`), never `configs/tictactoe-default.toml` in debug.
- Gate-report steps: the report's own `acceptance:` section must not re-quote `Overall verdict:` verbatim when a grep-count check expects exactly one match.

## Decisions (the Phase 6 design — fixed here so the decomposer projects, not invents)

### D1. Command set and global flags

- Subcommands: `play`, `generate`, `annotate`, `analyze`, `report`, `pipeline`. `discover` is NOT added (reserved for Phase 8). No interactive `play` mode.
- Global flags on `Cli` (before the subcommand): `-v/--verbose` (count: 0 = warn, 1 = info, 2 = debug, ≥3 = trace) and `-q/--quiet` (errors only; conflicts with `-v`). Environment `RUST_LOG`, when set and non-empty, overrides the flag-derived level (EnvFilter directive string).
- No-subcommand behavior unchanged: print `strategy-discovery 0.1.0`, exit 0.
- stdout carries ONLY the stage's result (one summary line per stage, or the `play` transcript / `report` Markdown). All logs go to stderr. Nothing written to stdout by a stage may vary between runs of the same inputs (byte-identity extends to stdout).
- `generate`, `annotate` flags unchanged from Phase 5 (see Current CLI contract) and their stdout lines unchanged. `analyze` keeps `--corpus`, `--min-coverage`, `--min-decisive`, `--min-distinct`, `--strict` and its stdout line; adds `--analyzers <comma list>` (default `summary`) and `--list-analyzers` (prints one `name<TAB>description` line per registered analyzer, exit 0, requires no corpus). `--strict` exit code becomes 3 (D9).

### D2. Game dispatch lives in exactly one place

- `src/cli/` becomes a directory module: `mod.rs` (Cli, Command, run, run_from, dispatch), `games.rs` (game-name resolution), `logging.rs`, `error.rs`, `play.rs`, `generate.rs`, `annotate.rs`, `analyze.rs`, `report.rs`, `pipeline.rs`. `src/main.rs` stays bootstrap + exit-code mapping only.
- `games.rs` is the ONLY non-test file in the crate that names a concrete game: `pub const KNOWN_GAMES: &[&str] = &["tictactoe"]` and a dispatch mechanism (macro `dispatch_game!(name, |bundle| expr)` or an equivalent visitor trait) that resolves a name to `crate::games::tictactoe::game_bundle()` and evaluates a generic body over `G: EngineGame + CorpusGame` (plus `G::State: Display` where `play` needs it). Unknown name → `CorpusError::Config("unknown game `{name}`; known games: {KNOWN_GAMES joined by ", "}")`. Adding a game = one arm + one `KNOWN_GAMES` entry. The four existing `match header.game.as_str()` copies are replaced by this.

### D3. `GenerateConfig` and the record schema are frozen

- `GenerateConfig<A>` fields, serde attributes, field order, `resolve()` semantics, `config_hash`, `RunMetadata`, `GameRecord`, `PositionRecord`, `AnnotationRecord`, `AnnotateMetadata`, `CorpusSummary`, `OutcomeCounts`, `DiversityThresholds` are unchanged in Phase 6. `SCHEMA_VERSION` stays 1; new documents introduced by Phase 6 carry `schema_version: 1`. Evidence: the pinned `run_id`s and SHA-256s above must reproduce at the end of every Phase 6 phase.
- `summary.json` content and bytes for a given corpus are unchanged (the summary analyzer wraps `summarize()` and writes the same `CorpusSummary`).

### D4. Analyzer registry (`src/discovery/analyze.rs`, game-agnostic)

```rust
pub struct AnalyzeOptions { pub analyzers: Vec<String> /* names, in run order */, pub thresholds: DiversityThresholds, pub strict: bool }  // impl Default: ["summary"], defaults, false
pub struct AnalyzeContext<'a, G: EngineGame + CorpusGame> { pub bundle: &'a GameBundle<G>, pub corpus_dir: &'a Path, pub run_id: String,
    pub games: &'a [GameRec<G>], pub positions: &'a [PosRec<G>], annotations: OnceCell-style lazy load of Vec<AnnotationRecord<..>> from annotations.jsonl }
impl AnalyzeContext { pub fn annotations(&self) -> Result<&[AnnotationRecord<..>], CorpusError> }   // Err(CorpusError::Precondition("{dir}/annotations.jsonl not found; run `annotate --corpus {dir}` first")) when absent
pub struct AnalyzerOutput { pub file: String /* file name relative to corpus_dir, e.g. "summary.json" */, pub value: serde_json::Value }
pub trait Analyzer<G: EngineGame + CorpusGame>: Send + Sync {
    fn name(&self) -> &str;                       // registry key; kebab-case
    fn description(&self) -> &str;                // one line, for --list-analyzers
    fn requires_annotations(&self) -> bool;
    fn run(&self, ctx: &AnalyzeContext<'_, G>, options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError>;
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError>;   // Markdown section for `report`; deterministic
}
pub struct AnalyzerRegistry<G> { BTreeMap<String, Arc<dyn Analyzer<G>>> }  // new(), register(analyzer) (replaces same name), get(name), names() sorted, iter
pub fn builtin_registry<G: EngineGame + CorpusGame>() -> AnalyzerRegistry<G>;  // "summary" + "agreement"
pub struct AnalyzeMetadata { pub schema_version: u32, pub game: String, pub run_id: String, pub analyzers: Vec<AnalyzerEntry { name: String, file: String }>, pub thresholds: DiversityThresholds, pub strict: bool, pub checks_pass: bool }
pub const ANALYZE_FILE: &str = "analyze.json";   // add to src/io/schema.rs alongside the other file-name constants
pub fn analyze<G>(bundle: &GameBundle<G>, corpus_dir: &Path, registry: &AnalyzerRegistry<G>, options: &AnalyzeOptions) -> Result<AnalyzeMetadata, CorpusError>;
// analyze(): load corpus once (reuse/extract summary.rs load_corpus) → for each name in options.analyzers (order given, duplicates → Config error, unknown → Config error listing registry.names()):
//   analyzer.run → write_json_pretty(corpus_dir/output.file) → record entry → write analyze.json. checks_pass = every analyzer's check passed (summary: diversity_pass; agreement: always true).
// analyze_corpus() (existing) stays as a thin wrapper: analyze with ["summary"] → returns the CorpusSummary (existing tests keep compiling).
```

- Built-in `summary` analyzer: `run` = `summarize(bundle, games, positions, &options.thresholds)` with `run_id` set, file `summary.json` (`SUMMARY_FILE`), value = the `CorpusSummary`; `render` = Markdown with the counts, a diversity table (metric / value / threshold / pass), outcomes, `by_pairing` table, `by_length`, `by_first_action_orbit`.
- Built-in `agreement` analyzer (`requires_annotations = true`): joins every `PositionRecord` to the `AnnotationRecord` with equal RAW `state` (plain equality; ADR 0009) — a position with no annotation → `CorpusError::Precondition("annotations.jsonl does not cover every position of {dir}; re-run `annotate --corpus {dir}`")`. Output `agreement.json`:
  `AgreementReport { schema_version, run_id, positions: usize, by_strategy: BTreeMap<String, AgreementCounts>, by_ply: BTreeMap<usize, AgreementCounts>, overall: AgreementCounts }`, `AgreementCounts { positions, agreeing /* chosen_action ∈ optimal_actions */, rate: f64 /* agreeing/positions, 0.0 when positions == 0 */ }`; `by_strategy` keyed by `chosen_by` (so `"opening"` and `"random-opening"` appear as their own keys). Facts to assert: on `generate-small.toml` the `perfect` key has `rate == 1.0` and `random` has `rate < 1.0`. `render` = Markdown table by strategy + by ply.
- Phase 8 miners register further analyzers (and their renderers) into this registry; nothing in `analyze()`/`report` names a specific analyzer except the default list.

### D5. Experiment config (`src/discovery/experiment.rs`) and the `pipeline` command

TOML shape (`schema_version` first; all relative paths resolve against the EXPERIMENT FILE'S parent directory; CLI `--out` overrides `out`):

```toml
schema_version = 1
name = "ttt-small"
game = "tictactoe"
out = "runs/ttt-small"          # run directory (created)

[generate]
sweep = "generate-small.toml"   # path to a GenerateConfig TOML, OR an inline table [generate.sweep] with the full GenerateConfig fields
threads = 4                     # optional
serial = false                  # optional, default false

[annotate]
enabled = true                  # default true
engine_depth = 9                # optional

[analyze]
analyzers = ["summary", "agreement"]   # default ["summary"]
strict = false                         # default false
[analyze.thresholds]                   # optional; defaults 0.5 / 0.2 / 0.5
min_canonical_coverage = 0.25
min_decisive_fraction = 0.2
min_distinct_game_fraction = 0.5

[report]
out = "report.md"               # relative to `out`; default "report.md"
```

```rust
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(deny_unknown_fields)]
pub struct ExperimentConfig<A> { schema_version: u32, name: String, game: String, out: PathBuf, #[serde(default)] generate: GenerateSection<A>, #[serde(default)] annotate: AnnotateSection, #[serde(default)] analyze: AnalyzeSection, #[serde(default)] report: ReportSection }
pub enum SweepSource<A> { #[serde(untagged)] Path(PathBuf) | Inline(GenerateConfig<A>) }  // string vs table is unambiguous in TOML; decomposer verifies with toml 1.1.4
pub struct ResolvedExperiment<A> { pub name, pub game, pub out: PathBuf /* absolute or cwd-relative after resolution */, pub sweep: GenerateConfig<A>, pub generate: GenerateOptions, pub annotate: Option<AnnotateOptions>, pub analyze: AnalyzeOptions, pub report_out: PathBuf }
pub fn load_experiment<A: DeserializeOwned>(path: &Path) -> Result<ExperimentConfig<A>, CorpusError>;          // IoError::Io / IoError::Toml with the path
pub fn resolve_experiment<G>(config: ExperimentConfig<G::Action>, base_dir: &Path, bundle: &GameBundle<G>, out_override: Option<&Path>) -> Result<ResolvedExperiment<G::Action>, CorpusError>;
// validation (CorpusError::Config, each message names field + value + fix): schema_version == 1; name non-empty; game == bundle.name and == sweep.game; sweep resolves via resolve(); analyzers non-empty, unique, all registered (registry names listed on error); agreement listed while annotate.enabled == false → error "analyzer `agreement` requires annotations; set [annotate] enabled = true or drop it"; threads Some(0) → error.
```

- `pipeline --experiment <toml> [--out DIR] [--stages generate,annotate,analyze,report]`: runs the stages in fixed order `generate → annotate → analyze → report`, each through the SAME library function the standalone command uses, with file handoffs in `out`. `--stages` selects a subset; stages run in canonical order regardless of list order; a stage whose input files are missing fails with `CorpusError::Precondition` naming the file and the stage to run first. Prints each stage's standard stdout line, then `pipeline={name} out={dir} stages={list}`; exit 3 if `analyze.strict` and checks failed (D9).
- Checked-in examples: `configs/tictactoe-experiment.toml` (sweep = `tictactoe-default.toml`, analyzers summary+agreement) and `tests/fixtures/experiment-small.toml` (sweep = `generate-small.toml`, thresholds 0.25/0.2/0.5, analyzers summary+agreement, out = `runs/experiment-small` — tests always override with `--out`).

### D6. `play`

- `play --game <name> --players <a,b,...> [--games N=1] [--seed S=0] [--opening <json array>] [--random-opening-plies K=0] [--max-plies M] [--evaluator NAME=bundle default] [--strategies FILE] [--out DIR] [--serial | --threads N]`.
- `--players`: comma-separated strategy entry names, exactly `bundle.players.len()` of them, resolved from `--strategies FILE` when given, else `bundle.default_strategies`. `--strategies FILE` is a TOML `StrategyFile { schema_version: u32, strategies: Vec<RosterEntry> }` (`#[serde(deny_unknown_fields)]`; `[[strategies]]` with `name` + `[strategies.spec]` exactly as in sweep configs; new type in `src/discovery/experiment.rs`; reused by the Phase 7 harness). Unknown name → Config error listing the available names.
- `--opening`: parsed with `serde_json::from_str::<Vec<G::Action>>` (tic-tac-toe: `--opening '[4,0]'`); validated by `replay` (illegal → Config error quoting the sequence and the rules error).
- Implementation = the `generate` path on a synthesized one-cell config: `GenerateConfig { schema_version: 1, game, seed, games_per_cell: N, max_plies, strategies: <resolved entries>, pairings: [players], evaluators: [evaluator], openings: [NamedOpening { name: "play", actions }], random_opening_plies: [K] }`. Extract from `corpus.rs` a reusable per-cell play helper (resolve → registry/providers → `RayonMatchEngine` → `expand_game`) and a run writer so `play` and `generate` share one code path. With `--out DIR`: the directory written is byte-identical to `generate` on the equivalent TOML (`run.json`, `games.jsonl`, `positions.jsonl`; a test proves it) and is a valid corpus for `annotate`/`analyze`/`report`. Without `--out`: nothing is written.
- stdout transcript (requires `G::State: std::fmt::Display`; deterministic, no timing): per game a header line `game {game_id} seed={seed:#018x} players={a,b} opening_plies={n}`, then one line per ply `{ply:>2}. {side_to_move:?} {action:json}` (annotate lines for `chosen_by == "opening"`/`"random-opening"` with a trailing ` (opening)`), then the final state via `Display` indented two spaces, then `outcome={outcome:?} length={len}`; after all games one line `games={N} {OutcomeCounts as wins=X:..,O:.. draws=.. unfinished=..}` (exact spelling fixed by the decomposer, then frozen in the README).

### D7. `report`

- `report --input <path> [--out FILE]`. `<path>` = a run directory → full report; or a single `.json` file whose file name equals a registered analyzer's output file (`summary.json`, `agreement.json`) → that analyzer's section only. Markdown to stdout; `--out FILE` additionally writes the identical bytes. Deterministic.
- Run-directory report sections, in order: `# Run {run_id}` — table (game, crate_name/crate_version, seed, games_per_cell, cells, games, positions, strategies list, pairings count, evaluators, openings, random_opening_plies); `## Annotation` (only if `annotate.json` exists: mode, engine_depth, evaluator, annotated, terminal, disagreements, solver_states); then one `## {Title}` section per entry of `analyze.json`, rendered by `registry.get(name).render(value)` (missing `analyze.json` → Precondition error telling the user to run `analyze --corpus <dir>`). Game dispatch via `run.json`'s `game`.
- Markdown style (matches the repo's conventions): blank line after every heading, blank line before and after every list/table, tables use `| --- |` delimiters, fenced blocks always carry a language. Floats printed with fixed precision (`{:.3}` for rates/fractions).

### D8. Logging (`src/cli/logging.rs` + instrumentation)

- `Cargo.toml`: `tracing-subscriber = { version = "0.3.23", features = ["env-filter", "fmt"] }` (add `env-filter`; keep default features otherwise).
- `pub fn init(verbosity: u8, quiet: bool)`: `fmt::Subscriber` → stderr, `with_ansi(false)`, `with_target(false)`, no timestamps (`without_time()`), level from flags (`error` if quiet; else warn/info/debug/trace by count) unless `RUST_LOG` is set (then `EnvFilter::try_from_default_env()`). Uses `try_init()` and ignores the already-initialized error so `run_from` can be called repeatedly in-process (tests).
- Instrumentation (stderr only; never into outputs): `generate` — `info_span!("generate", run_id)`, per cell `info!(cell = index, strategies, evaluator, opening, plies, games, positions, elapsed_ms)`; `annotate` — `info!(mode, annotated, terminal, disagreements, elapsed_ms)`; `analyze` — per analyzer `info!(analyzer, file, elapsed_ms)`; `report`/`pipeline`/`play` — `info!` per stage/game; `MinimaxStrategy::choose` — `trace!` of root values and the chosen action; `debug!` at resolve time listing the cell count and `run_id`.
- Every stage's stdout summary line stays exactly as today; logging adds nothing to stdout.

### D9. Error taxonomy and exit codes (`src/cli/error.rs`, `src/main.rs`)

| exit | class | when |
| --- | --- | --- |
| 0 | success | stage completed; for `analyze --strict` / `pipeline`, all checks passed |
| 1 | failure | runtime error: I/O write failure, match/engine/strategy failure at play time, solver limit, internal invariant (`IoError::Io` not NotFound, `IoError::Json` on a file we wrote, `MatchError`, `SolverError`, `StrategyError::{NoLegalActions, Other}` at play time, anything unclassified) |
| 2 | usage | invalid invocation or input: clap parse errors (clap's own 2), `CorpusError::Config`, `CorpusError::Precondition`, `IoError::{Toml, SchemaVersion, Missing, Invalid}`, `IoError::Io` with `ErrorKind::NotFound` on an input path, `IoError::Json` on a user-supplied file, `StrategyError::Unimplemented` (reserved kinds) |
| 3 | check | a requested check failed: `analyze --strict` with `diversity_pass == false` (WAS 2 in Phase 5); `pipeline` with `[analyze] strict = true` and failing checks |

- Add `CorpusError::Precondition(String)` (`#[error("precondition failed: {0}")]`) for "required input missing; run X first" situations.
- `pub fn exit_code(err: &anyhow::Error) -> u8` classifies by downcasting the chain (`err.chain()` / `downcast_ref`) per the table; `main.rs` = `ExitCode::from(exit_code(&err))` after printing `error: {err:#}` to stderr. Remove the `std::process::exit(2)` in `analyze`; strict failure becomes a typed error (`CliError::CheckFailed(String)` or equivalent) mapped to 3, message listing `summary.diversity_failures`.
- Message audit: every `CorpusError::Config`/`Precondition` message names the offending field or flag, the offending value, and the accepted values or the fix. Unknown game/strategy/evaluator/analyzer messages list the known names. Illegal opening messages quote the action sequence and the rules error. Existing messages already of this form stay.
- Update every place that asserts or documents `--strict exit 2`: `tests/cli_pipeline.rs::strict_analyze_exits_two_on_undiverse_corpus` (rename/assert 3), `README.md`. Do NOT edit `docs/component-evaluation.md` or `artifacts/**` (historical Phase 5 evidence); ADR 0010 records the change.

### D10. Byte-identity rule extended

Same inputs ⇒ byte-identical: `play` stdout and its `--out` files; every analyzer output file; `analyze.json`; `report` stdout and `--out` file; `pipeline` stage files. stderr (logs) is exempt. No `HashMap` iteration reaches any of these; all maps `BTreeMap`.

### D11. Tests and fixtures

- New/extended CLI integration tests (binary-spawning, `cli_pipeline.rs` style; one file per command or one per step — decomposer's call, each `tests/*.rs` owned by exactly one step): `play` transcript + `--out` byte-equality with `generate`; `generate → annotate → analyze --analyzers summary,agreement → report` over `generate-small.toml`; `report --input <dir>/summary.json` single-file mode; `analyze --list-analyzers`; `pipeline --experiment tests/fixtures/experiment-small.toml --out <tmp>` full run + `--stages` subset + Precondition error when skipping ahead; exit codes 1/2/3 exercised (missing config → 2, bad TOML → 2, unknown analyzer → 2, strict failure on `generate-draws.toml` → 3); `-v` produces stderr and leaves stdout unchanged; `RUST_LOG=off` silences; repeated `report` produces identical bytes.
- Library tests: registry (register/replace/names/unknown), agreement analyzer facts (perfect rate 1.0, random < 1.0 on `generate-small`), experiment config parse/validate/relative-path resolution (path and inline sweep), `StrategyFile` parse, `exit_code` classification table, logging init idempotence.
- Keep `tests/smoke.rs`, `tests/corpus.rs`, `tests/annotate.rs`, `tests/corpus_bench.rs` compiling: `analyze_corpus` signature preserved (D4).
- Debug `cargo test` wall ≤ 45 s.

### D12. Documentation

- `README.md` Usage rewritten: every command with flags and its stdout line, the run-directory table extended with `analyze.json`, `agreement.json`, `report.md`, exit-code table, logging (`-v`, `-q`, `RUST_LOG`), experiment config example, `play` example. Remove "`report` arrives with Phase 6".
- `docs/adr/0010-cli-contract-and-analyzer-registry.md` (same layout as ADR 0009: Status / Context / Decision / Consequences): command set, stdout/stderr split, game dispatch location, analyzer registry trait + manifest, experiment config schema + relative-path rule, exit-code table (incl. the 2→3 change for `--strict`), logging policy, byte-identity extension.
- `CLAUDE.md` Commands line: `cargo run -- <subcommand>` list updated to `play, generate, annotate, analyze, report, pipeline`. `docs/plan.md` untouched.
- `artifacts/reproducibility/cli-outputs-determinism.md`: evidence (verbatim PowerShell script + output) that `play --out`, `analyze` (both analyzers), `report`, `pipeline` outputs are byte-identical across two runs and that the pinned Phase 5 hashes still reproduce. Script pattern: `$Root` from `$PSScriptRoot`, outputs under gitignored `target/phase6/`, `Get-FileHash`, no timestamps in the committed file.

## Constraints

- Rust edition 2024, stable toolchain; `cargo fmt --all --check` exit 0 (repo `rustfmt.toml`, `max_width = 132`); `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean; `cargo doc --no-deps --workspace` warning-free; every `pub` item documented (`///`).
- No new dependencies beyond enabling `tracing-subscriber`'s `env-filter` feature. No analysis crates (`polars`, `linfa`, `smartcore`).
- `GenerateConfig`, every record/document shape from ADR 0009, `SCHEMA_VERSION = 1`, the seed formulas, and the pinned `run_id`s/SHA-256s are frozen (D3).
- `src/core/**` unchanged except additive doc/trace statements; `src/games/tictactoe/**` changes limited to what a game-side adaptation legitimately needs (none expected; `Board` already implements `Display`). Nothing outside `src/cli/games.rs` (and tests) names a concrete game.
- Byte-identity rule (ADR 0009 + D10) for every persisted file and every stage's stdout.
- stdout = results only; stderr = logs + `error:` lines. Exit codes per D9.
- Existing stdout lines of `generate`/`annotate`/`analyze` and the no-subcommand banner are unchanged (tests and the Phase 5 evidence scripts depend on them).
- Debug `cargo test` wall ≤ 45 s; CI matrix (ubuntu + windows) must stay green — use `Path::join`, never hard-coded separators; tests write only under `CARGO_TARGET_TMPDIR`.
- Markdown written by the tool or by workers follows: blank line after headings, blank lines around lists/tables, `| --- |` delimiters, fenced blocks with a language.
- Worker commit protocol: one commit per step containing all `files_in_scope` including the report; `cargo` commands one per tool call.

## Assumptions

- `game-player` stays pinned at lock `0b8ce6f`; no engine API change is needed (the CLI only composes existing framework functions).
- `toml 1.1.4` deserializes `#[serde(untagged)]` string-vs-table enums for `SweepSource` (decomposer verifies before fixing step text; fallback: a `sweep_path`/`sweep` pair of optional fields with an exactly-one validation rule).
- Exit code 2 for clap usage errors is clap's default and is kept; the project's own usage errors deliberately share it.
- Phase 7 (harness/archive) will consume `StrategyFile`, the agreement analyzer, and `pipeline`; Phase 8 `discover` = `pipeline` + mining analyzer + validate. Nothing in Phase 6 pre-builds those.

## Out of scope

- Interactive `play` mode; `discover` command; strategy-evaluation harness, tournament, archive (Phase 7); any mining analyzer (Phase 8); Parquet output; schema version bump; changes to diversity thresholds or the default sweep (the `distinct_game_fraction` scale finding stays documented, not re-litigated); editing historical evidence under `artifacts/**` or `docs/component-evaluation.md`; JSON log format; log files; progress bars; a second game.

## Definition of Done (project)

1. `cargo build --workspace --all-targets`, `cargo test --workspace` (all green; new tests per D11 present), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` all exit 0; debug `cargo test` wall ≤ 45 s.
2. `cargo run -- play --game tictactoe --players random,perfect --games 2 --seed 7` exits 0 and prints two game transcripts (D6 format) ending in a `games=2 ...` line; `play ... --out <dir>` followed by `annotate --corpus <dir>` and `analyze --corpus <dir>` exits 0 each; `play --out` files are byte-identical to `generate` on the equivalent TOML.
3. `cargo run -- generate --config tests/fixtures/generate-small.toml --out <dir>` → `annotate --corpus <dir>` → `analyze --corpus <dir> --analyzers summary,agreement` → `report --input <dir>` all exit 0; `<dir>` contains `run.json`, `games.jsonl`, `positions.jsonl`, `annotations.jsonl`, `annotate.json`, `summary.json`, `agreement.json`, `analyze.json`; the report is Markdown with `# Run 5eafa65f76637df3`, `## Annotation`, `## Summary`, `## Agreement` sections; `agreement.json` shows `perfect` rate 1.000 and `random` rate < 1.
4. Pinned Phase 5 outputs reproduce: `run_id` `5eafa65f76637df3` / `933a94e742497e81` / `74bba44805e514a7` and the SHA-256s in the pin table (run.json, games.jsonl, positions.jsonl, annotations.jsonl, annotate.json, summary.json for `generate-small.toml`).
5. `cargo run -- pipeline --experiment tests/fixtures/experiment-small.toml --out <dir>` exits 0 and produces the same file set as item 3 plus `report.md`; `--stages analyze` on an empty dir exits 2 with a message naming `run.json` and `generate`; `analyze --list-analyzers` lists `agreement` and `summary`.
6. Exit codes: missing config file → 2; malformed TOML → 2; unknown game/analyzer → 2; `analyze --corpus <draws-dir> --strict` → 3 with the failures on stderr; all verified by tests.
7. `-v` adds `info` lines on stderr and leaves stdout byte-identical to a run without `-v`; `RUST_LOG=off` suppresses all logs; no log line ever reaches stdout or a persisted file.
8. Repeated runs of `play`, `analyze` (both analyzers), `report`, `pipeline` on the same inputs are byte-identical (files and stdout), evidenced in `artifacts/reproducibility/cli-outputs-determinism.md`.
9. `src/cli/games.rs` is the only non-test file naming `tictactoe` outside `src/games/` (`grep -rln tictactoe src --include=*.rs` returns only `src/games/**`, `src/cli/games.rs`, and files whose only matches are inside `#[cfg(test)]` modules); `src/main.rs` contains no command logic.
10. `README.md`, `CLAUDE.md` commands line, and `docs/adr/0010-cli-contract-and-analyzer-registry.md` updated per D12; `docs/plan.md` and `artifacts/**` (except the new determinism file) untouched.
11. Every step's worker report is committed with its step; ledger complete; project gate report `implementation-artifacts/phase6-cli-gate-report.md` with verdict PASS.
