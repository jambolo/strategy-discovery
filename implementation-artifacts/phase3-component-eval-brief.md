# phase3-component-eval — Brief

## Goal

Execute Phase 3 of `docs/plan.md` ("Component evaluation and selection"): run feasibility spikes against `game-player` and the candidate analysis/mining crates, record benchmark + reproducibility evidence under `artifacts/`, and produce `docs/component-evaluation.md` (weighted scoring of the contested analysis stack) plus ADR notes under `docs/adr/` for settled components — passing Evaluation Gate A. Authoritative spec: `docs/plan.md` lines 83-104 (Phase 3) and lines 224-275 (Component evaluation) at commit `b8aa59a8cd6c29c1e251198a81c01b39e83ec815`; line numbers shift if that file is edited — re-verify against live source.

Prerequisite folded into this effort: the tic-tac-toe domain (`src/games/tictactoe/`) that Phase 2 was supposed to deliver does NOT exist in the repo (see Context). The spikes need it, Phase 4 needs it, so it is built first here.

## Context

### Repo state at planning time (commit `b8aa59a8cd6c29c1e251198a81c01b39e83ec815`, branch `develop`)

- Single Rust crate `strategy-discovery` (lib + thin bin), edition 2024, stable toolchain `rustc 1.94.1`. No `[workspace]` table in root `Cargo.toml`.
- Root dependencies (exact, do not change in this effort): `anyhow 1`, `clap 4 (derive)`, `game-player` (git `https://github.com/jambolo/game-player.git`, branch `master`, locked to `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`, crate version 0.4.0), `rand 0.10`, `rand_chacha 0.10`, `rayon 1`, `serde 1 (derive)`, `serde_json 1`, `thiserror 2`, `toml 1`, `tracing 0.1`, `tracing-subscriber 0.3`.
- Existing modules: `src/lib.rs` (`pub mod cli, core, discovery, games, io, strategy`; consts `NAME`, `VERSION`), `src/main.rs` (calls `strategy_discovery::cli::run()`), `src/core/{mod,traits,symmetry,features,derived,dsl,interpreter}.rs` (complete, 62 unit tests), `src/games/mod.rs` (doc comment ONLY — no tic-tac-toe), `src/strategy/mod.rs`, `src/discovery/mod.rs`, `src/io/mod.rs`, `src/cli/mod.rs` (placeholders), `tests/smoke.rs` (2 tests).
- Baseline verified green at planning time: `cargo test` → 62 lib + 2 integration tests pass; `cargo check` clean.
- `docs/` contains only `plan.md`. `docs/adr/`, `docs/component-evaluation.md`, `artifacts/` do not exist.
- `.gitignore` ignores any directory named `target` (bare pattern `target`) and `/target`, `*.pdb`, `.vscode/`.
- CI (`.github/workflows/ci.yml`): `cargo build --workspace --all-targets` + `cargo test --workspace` on Ubuntu and Windows; on PRs also `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings`. Because root has no `[workspace]`, nested packages under `spikes/` are NOT built by CI.
- Prior pipeline artifacts live in `implementation-artifacts/` (`phase1-baseline-*`, `phase2-core-*`); same directory is used for this effort.
- Commit style: plain descriptive messages (e.g. `Implemented phase 2: tic-tac-toe domain`), no conventional-commit prefixes.

### Framework core API the tic-tac-toe domain and spikes build on (`src/core`, read live source for exact signatures)

`src/core/traits.rs`:

- `trait GameDomain: Send+Sync+'static { type State: Clone+Eq+Hash+Debug+Send+Sync+'static; type Action: Clone+Eq+Hash+Debug+Send+Sync+'static; type Player: Copy+Eq+Hash+Debug+Send+Sync+'static; type Outcome: Clone+Eq+Debug+Send+Sync+'static; }`
- `trait GameRules<G> { fn initial_state(&self)->G::State; fn player_to_move(&self,&G::State)->G::Player; fn legal_actions(&self,&G::State)->Vec<G::Action> /* empty iff terminal */; fn apply(&self,&G::State,&G::Action)->Result<G::State,RulesError> /* Err(GameOver) on terminal, Err(IllegalAction(String)) otherwise */; fn outcome(&self,&G::State)->Option<G::Outcome> /* Some iff terminal */; fn is_terminal(&self,&G::State)->bool (default) }`
- `trait GamePrimitives<G> { fn position_count(&self)->usize; fn adjacent(&self,usize)->Vec<usize> /* ascending, no dups */; fn lines(&self)->Vec<Vec<usize>> /* stable order = line index */; fn symmetry_group(&self)->SymmetryGroup /* degree == position_count() */; fn occupant(&self,&G::State,usize)->Option<G::Player>; fn action_position(&self,&G::Action)->Option<usize>; fn transform(&self,&G::State,&Permutation)->G::State /* occupant of p moves to perm.apply(p); player-to-move unchanged */ }`
- `trait FeatureExtractor<G> { fn definitions(&self)->Vec<FeatureDef>; fn extract(&self,&G::State)->FeatureVector }`
- `trait Canonicalize<G> { fn canonicalize_with_transform(&self,&G::State)->(G::State,Permutation) /* idempotent; canonical state returns identity */; fn canonicalize(&self,&G::State)->G::State (default) }`
- `trait StateEvaluator<G> { fn evaluate(&self,&G::State,perspective:G::Player)->f32 }`
- `trait Strategy<G>: Send { fn choose(&mut self,&G::State,legal:&[G::Action])->Result<G::Action,StrategyError> }`, `trait StrategyProvider<G> { fn kind(&self)->&str; fn create(&self,seed:u64)->Box<dyn Strategy<G>> }`
- `MatchConfig{games,seed,max_plies}`, `MatchRecord<A,O>{game_index,seed,actions,outcome}`, `trait MatchEngine<G>`.
- `RulesError::{GameOver, IllegalAction(String)}`.

`src/core/symmetry.rs`: `Permutation::new(Vec<usize>)->Result`, `::identity(n)`, `.apply(p)`, `.compose(&other)`, `.inverse()`, `.permute(&[T])`; `SymmetryGroup::generate(degree,&[Permutation])->Result` (closure under composition), `::from_elements`, `.elements()`, `.order()`, `.orbits()->Vec<Vec<usize>>`, `.orbit_index()`, `.set_orbits(&[Vec<usize>])`.

`src/core/features.rs`: `Tier::{Primitive, Supplied, Invented}`, `FeatureValue::{Bool,Int,Float,Set}` with `as_bool/as_int/as_float/as_set`, `FeatureExpr` algebra (serde tag `op`), `FeatureDef::native(name,tier,description)` / `::derived(name,tier,description,expr)`, `FeatureVector(BTreeMap<String,FeatureValue>)` with `insert/get/merged`, `FeatureVocabulary`.

`src/core/derived.rs`: `PrimitiveFeatures::new(rules, primitives)` — tier-1 extractor built mechanically from `GamePrimitives` (orbits, lines); `.orbits()`, `.lines()`.

`src/core/mod.rs::kinds`: `MINIMAX="minimax"`, `RANDOM="random"`, `HEURISTIC_RULES="heuristic-rules"`.

### Tic-tac-toe domain facts (to implement in `src/games/tictactoe/`)

- Marker type `TicTacToe: GameDomain` with `State = Board`, `Action = Move` (newtype over cell index `0..9`, row-major), `Player = Player::{X, O}` (X moves first), `Outcome = Outcome::{Win(Player), Draw}`.
- `Board`: 9 cells each `Option<Player>` plus player-to-move. Must be `Copy`-cheap (≤ 16 bytes is easy) — cheap clone is a design rule. Derive `Clone, Copy, PartialEq, Eq, Hash, Debug`; serde `Serialize/Deserialize` so records can store it.
- Win lines, exactly 8, in this order (line index = position in list): rows `{0,1,2},{3,4,5},{6,7,8}`; cols `{0,3,6},{1,4,7},{2,5,8}`; diags `{0,4,8},{2,4,6}`.
- Topology: `adjacent(p)` = orthogonal 4-neighbourhood on the 3x3 grid (ascending).
- Symmetry group: dihedral D4, order 8. Build with `SymmetryGroup::generate(9, &[rot90, reflect])` from two generator permutations — NOT by listing 8 elements by hand. Rotation 90° clockwise as a cell map: `0→2, 1→5, 2→8, 3→1, 4→4, 5→7, 6→0, 7→3, 8→6` (verify by test: applying 4 times = identity). Horizontal reflection: `0↔2, 3↔5, 6↔8, 1,4,7 fixed`.
- Cell orbits under D4 MUST come out of `symmetry_group().orbits()` as `{4}`, `{0,2,6,8}`, `{1,3,5,7}` — tests assert derivation, code never hand-codes these sets.
- Enumeration oracles: reachable legal positions from the initial state (including terminal ones, BFS over `legal_actions`, no moves after terminal) = **5,478**; distinct canonical forms = **765**. If an implementation disagrees, the implementation is wrong.
- Canonicalization: choose the minimum over the 8 symmetric images under a fixed total order on an integer encoding of the board (e.g. base-3 cell encoding; player-to-move is invariant under transform so it need not enter the order). Return the permutation used; identity when the state is already canonical.
- Tier-2 hand-supplied features (minimum set; names are the decomposer's call but must be stable and documented in `FeatureDef` descriptions): per-line occupancy counts for X and O; `threat` counts per player (lines with two own marks and one empty cell); set of immediate winning cells for player-to-move; set of cells that block an opponent's immediate win; set of fork cells for player-to-move (empty cells which, when played, create ≥ 2 simultaneous threats); boolean "win available". All tier `Tier::Supplied`.
- Engine conversion layer (`src/games/tictactoe/engine.rs` or similar): `impl game_player::State for Board` — `type Action = Move`; `fingerprint()` unique per position (2 bits per cell + 1 bit to-move, as in game-player's example); `whose_turn()` maps `Player::X→PlayerId::Alice`, `Player::O→PlayerId::Bob`; `is_terminal()`; `apply()` (infallible; caller guarantees legality). Provide `Player↔PlayerId` conversions. Do NOT implement `minimax::ResponseGenerator` / `StaticEvaluator` in `src/` — those are Phase 4 (spikes implement them locally, see below).
- Tests (plan Phase 2 step 6): all 8 win lines detected for both players; draw board; illegal moves rejected (occupied cell, out-of-range index, any move after terminal → `GameOver`); turn alternation; orbit derivation; enumeration counts 5,478 / 765; canonicalization idempotent and outcome-preserving on fixture states; feature extractor on fixture positions (a known threat, a known fork).
- Micro-benchmarks (plan Phase 2 step 7): non-failing `#[test]` functions (or `#[ignore]`d tests run explicitly) using `std::time::Instant` + `std::hint::black_box` that print state-clone cost and move-generation throughput. No benchmark crates.

### `game-player` 0.4.0 API facts (read at `%USERPROFILE%\.cargo\git\checkouts\game-player-ac0a30f334ffbd31\0b8ce6f\`; run `cargo fetch` if absent)

- `game_player::{PlayerId, State, StaticEvaluator}`; modules `minimax`, `mcts`, `transposition_table`, `state`, `static_evaluator`; optional feature `mcts_random_playout`.
- `enum PlayerId { Alice = 0, Bob = 1 }` with `.other()`. Alice is the maximizing side.
- `trait State: Clone { type Action: Clone; fn fingerprint(&self)->u64; fn whose_turn(&self)->PlayerId; fn is_terminal(&self)->bool; fn apply(&self,&Self::Action)->Self; }`
- `trait StaticEvaluator { type State; fn evaluate(&self,&Self::State)->f32 /* Alice perspective */; fn alice_wins_value(&self)->f32; fn bob_wins_value(&self)->f32; }` — for terminal states `evaluate` MUST return exactly the win value (or a draw value in between).
- `trait minimax::ResponseGenerator { type State: State; fn generate(&self,&Self::State,depth:u32)->Vec<Action>; }` — POLICY: returns empty iff the state is terminal (debug-asserted inside search; a violation panics in debug/test builds).
- `pub fn minimax::search<S,E,R>(sef:&E, rg:&R, s0:&S, max_depth:u32) -> Option<S::Action>` — alpha-beta with a per-call internal transposition table (`TranspositionTable::new(0)`); uses `Rc`/`RefCell` internally (single-threaded per call; safe to call concurrently from separate rayon tasks because nothing is shared). Deterministic: candidate ordering is a stable sort on preliminary values; no RNG.
- **Pre-spike reading of the source**: `search` returns ONLY the chosen action. The internal `Response { action, value, quality }` type and `search_recursive` are private; the position's minimax value, the set of equal-valued best moves, and the transposition table are not exposed. Ties are resolved by stable-sort order (first best in generation order), not enumerable. The annotation spike must confirm this empirically and prototype the fallback (below). The author of `game-player` is the same person as this repo's owner, so "add a value/tie-set-returning search API upstream" is a legitimate documented option, but it is out of scope for this effort's code.
- Worked example: `examples/tic_tac_toe.rs` in the checkout shows a complete `State` + `StaticEvaluator` (+100/-100 win values, line-count heuristic) + `ResponseGenerator` implementation and a `search(&evaluator,&moves,&board,9)` self-play loop. Use it as the template for spike-local adapters.

### Spike layout and conventions

- Each spike is a throwaway, standalone binary crate under `spikes/<spike-name>/` with its own `Cargo.toml` containing an empty `[workspace]` table (keeps it detached from the root package) and `strategy-discovery = { path = "../.." }`. Spike crates MAY depend on candidate crates (`polars`, `linfa`, `linfa-trees`, `smartcore`, `arrow`, `parquet`, `serde_arrow`, `ndarray`); the ROOT `Cargo.toml` MAY NOT. Spike crates are not part of CI; they must still build and run with `cargo run --release --manifest-path spikes/<spike-name>/Cargo.toml` on the dev machine, and `cargo fmt` is applied to them.
- Candidate crate versions on crates.io at planning time (2026-08-20): `polars 0.55.2`, `linfa 0.8.1`, `linfa-trees 0.8.1`, `smartcore 0.6.5`, `arrow 59.2.0`, `parquet 59.2.0`, `serde_arrow 0.15.0`. Use these or newer; record the exact resolved versions in the artifact.
- Timing runs are ALWAYS `--release`. Every evidence artifact carries a header block: date, git commit of this repo, `rustc --version`, OS (`Windows 11 Pro 10.0.26200`), CPU `AMD Ryzen 7 7800X3D 8-Core Processor` (16 logical CPUs), RAM 31 GiB, and the exact command used to produce it. Machine-readable numbers go in a sibling `.json`; the `.md` is the human-readable summary and the file the evaluation matrix cites.
- Evidence directories: `artifacts/benchmarks/` (throughput, per-position cost, format read/write cost, crate compile time) and `artifacts/reproducibility/` (repeat-run determinism: byte-identical outputs, serial == parallel). Commit the artifacts; they are the evidence Gate A is judged on. Spikes write output files via a `--out <dir>` argument defaulting to the correct artifacts directory relative to the repo root.

### Spikes required (plan Phase 3 step 3-4)

1. **Annotation capability spike** (`spikes/annotate-spike/`): spike-local `ResponseGenerator` + `StaticEvaluator` over `Board`. For every one of the 5,478 reachable positions (and separately the 765 canonical ones): call `minimax::search(.., 9)`, record the returned action, measure per-position wall time (mean/median/p99, total), run twice and diff to confirm determinism. Document explicitly whether value and best-move SET are obtainable (expected: NO). Prototype the fallback in the same spike: a framework-side exhaustive solver (retrograde/negamax over `GameRules` with memoization keyed by state or canonical state) that yields, per position, the game-theoretic value `{win for X, draw, win for O}` / side-to-move value and the full set of optimal actions; time it; cross-check that game-player's returned action is in the solver's optimal set for every non-terminal position (report any disagreement verbatim). Output: `artifacts/benchmarks/annotation-per-position.{md,json}`, `artifacts/reproducibility/annotation-determinism.md`.
2. **Throughput spike** (`spikes/throughput-spike/`): minimax self-play (both sides `search`) games/sec at depths {2, 4, 9}; N ≥ 200 games per configuration; serial vs rayon with thread counts {1, 2, 4, 8, 16} (`ThreadPoolBuilder::num_threads`); per-game seed derived from `(master_seed, game_index)` even though the engine is deterministic (so the harness shape matches Phase 4). Record nodes/sec if measurable (count `generate` calls). Confirm parallel run's ordered results are byte-identical to the serial run. Output: `artifacts/benchmarks/selfplay-throughput.{md,json}`, `artifacts/reproducibility/selfplay-serial-vs-parallel.md`.
3. **Record-format spike** (`spikes/record-format-spike/`): define a provisional position-level record matching plan Phase 5 step 2 — per move: state (encoded), canonical state, legal moves, chosen move, move number, side to move; per game: game id, strategy params, seeds, config hash, outcome, length — as serde structs with a `schema_version` field. Generate ≥ 10,000 games' worth of records (random or minimax self-play — content does not matter). Write and re-read as (a) JSONL via `serde_json`, (b) Parquet via `polars` (`ParquetWriter`/`ParquetReader`) and/or `arrow`+`parquet` (`serde_arrow` for struct→arrow). Measure file size, write time, read time. Then perform the SAME aggregation two ways — (i) `polars` lazy query over the JSONL (`LazyJsonLineReader`) and over the Parquet, (ii) plain serde deserialize + hand-rolled `HashMap` aggregation — e.g. outcome distribution grouped by first-move orbit and by move number; confirm identical results. Record crate compile time (clean `cargo build --release` wall time with and without `polars`) and dependency counts (`cargo tree | wc -l`). Output: `artifacts/benchmarks/record-format.{md,json}`.
4. **Rule-induction spike** (`spikes/rule-induction-spike/`): build a per-position feature dataset — tier-1 features from `PrimitiveFeatures` + tier-2 from the tic-tac-toe extractor, canonicalized, with labels from the exhaustive solver of spike 1 (game-theoretic value of the position; optionally "best move is in orbit k"). Fit a decision-tree classifier with `linfa-trees` (`DecisionTree`) and with `smartcore` (`DecisionTreeClassifier`); for each record: fit time, accuracy on a held-out split (seeded split), determinism (same seed → identical tree structure on two runs), and — most important for Phase 8 — whether the tree's internal structure (feature index, threshold, children, leaf class) is PUBLICLY traversable so it can be converted into an ordered decision list; produce an actual converted decision list printed as text for one tree. Survey association/pattern-mining crates (`cargo search` for apriori / fp-growth / frequent itemset; record what exists, versions, last update) — survey only, no implementation. Output: `artifacts/benchmarks/rule-induction.{md,json}` including the printed decision list.

### Evaluation deliverables (plan "Component evaluation" section)

- `docs/component-evaluation.md`: one weighted decision matrix per contested category — (1) dataframe/query layer: `polars` vs serde + custom aggregation; (2) rule induction: `linfa-trees` vs `smartcore` decision trees, plus an association-mining row (candidates from the survey, or "custom" if none viable); (3) corpus file format: JSONL vs Parquet. Criteria and weights, exact: Reusability across games 0.30; Determinism/reproducibility under seeds 0.20; Performance on target workloads 0.20; Integration complexity in Rust codebase 0.15; Observability/debuggability 0.10; Maintenance risk 0.05. Scores 1-5 integers; `final_score = sum(score_i * weight_i)`. Thresholds: `>= 4.0` default path; `3.0 <= s < 4.0` optional/experimental; `< 3.0` reject/defer. Every score cell cites the artifact file (and field) that justifies it; no uncited scores. Ends with a "Selected defaults" table and an "Experimental alternates" table, plus the consequence for Phase 5 (JSONL-only vs JSONL+Parquet, per plan "Further Considerations" item 3).
- `docs/adr/`: one short ADR per component, numbered `NNNN-<slug>.md` with sections Status / Context / Decision / Consequences. Required: `0001-game-player-minimax-adapter.md` (roles: generation, per-position annotation, benchmark opposition; MUST state the per-position capability finding and the chosen annotation fallback), `0002-self-play-experiment-runner.md`, `0003-json-toml-metadata-persistence.md`, `0004-summary-aggregator.md`, and one ADR per selected analysis component (dataframe layer, rule-induction library, corpus format). ADR numbering continues sequentially.
- `docs/plan.md` is NOT modified.

## Constraints

- Toolchain: stable Rust 1.94.1, edition 2024, Windows dev machine; CI also runs Ubuntu — all `src/` code platform-neutral. PowerShell is the shell for commands in artifacts.
- Root crate must pass: `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Root `Cargo.toml` dependencies unchanged — NO `polars`, `linfa`, `smartcore`, `arrow`, `parquet`, `criterion`, `proptest`, `bincode`, `ndarray` in the root. Candidate crates live only in `spikes/*/Cargo.toml`. (Selected crates enter the root when the analysis path lands in Phase 5/8, per plan Phase 1 note.)
- `src/core` stays game-agnostic (no tic-tac-toe references). Tic-tac-toe code only under `src/games/tictactoe/`.
- Spike code is throwaway and never imported by `src/`. `src/` must not depend on anything under `spikes/`.
- Timing numbers only from `--release` builds. Record exact resolved crate versions.
- No network services; everything runs locally. Cargo may fetch crates from crates.io.
- Artifacts and docs are written for later models and humans alike: tables, exact commands, exact numbers; no speculation presented as measurement — anything not measured is labelled "not measured".

## Assumptions

- The tic-tac-toe domain is absent from the repo (verified on `develop` and on `feature/phase-2-core-abstractions`; trees identical) and is built in this effort as Phase 1 of the roadmap. If it lands elsewhere first, Phase 1 is skipped after verifying its DoD.
- `game-player` at lock `0b8ce6f9` is the version evaluated; no upstream changes are made or assumed during this effort.
- Hardware baseline for all numbers: the dev machine described above. Cross-machine comparability is not required; same-machine comparability between candidates is.
- Plan acceptance "per-position annotation capability confirmed (or a fallback documented)" is satisfied by ADR 0001 + the annotation spike artifact, whichever way the finding goes.
- Spike 4's labels come from spike 1's solver; the decomposer may either order the steps (`depends_on`) or let spike 4 embed its own copy of the solver — both acceptable.

## Out of scope

- `MinimaxStrategy`, strategy registry, match runner, benchmark roster, tactical fixtures in `src/` (Phase 4).
- Corpus schema finalization, `src/io` writers, annotation stage in `src/discovery` (Phase 5) — the spike record type is provisional.
- CLI subcommands (Phase 6). `src/cli` stays a placeholder.
- Adding any analysis crate to the root `Cargo.toml`.
- Modifying `game-player` upstream or `docs/plan.md`; changing CI workflows.
- Property-based tests (Phase 7), mining (Phase 8), concept induction (Phase 9).
- MCTS evaluation (`game_player::mcts`) — minimax is the settled engine.

## Definition of Done (project)

All checked on branch `feature/phase-3-component-evaluation`:

1. `src/games/tictactoe/` implements `GameDomain`, `GameRules`, `GamePrimitives`, tier-2 `FeatureExtractor`, `Canonicalize`, and `game_player::State`; `cargo test` passes and includes assertions for all 8 win lines, draw, illegal-move rejection, turn alternation, derived orbits `{4}/{0,2,6,8}/{1,3,5,7}`, enumeration counts 5,478 and 765, canonicalization idempotence; micro-benchmark tests exist and log timings.
2. `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` all exit 0 at the final commit; root `Cargo.toml` `[dependencies]` byte-identical to the starting commit.
3. Four spike crates exist under `spikes/` and each runs via `cargo run --release --manifest-path spikes/<name>/Cargo.toml` (command recorded in its artifact header).
4. `artifacts/benchmarks/` contains at least `annotation-per-position.md`, `selfplay-throughput.md`, `record-format.md`, `rule-induction.md` (each with a `.json` sibling); `artifacts/reproducibility/` contains at least `annotation-determinism.md` and `selfplay-serial-vs-parallel.md`, each stating byte-identical repeat-run results (or the exact diff if not).
5. `docs/component-evaluation.md` exists with completed, artifact-cited weighted matrices for the three contested categories, thresholds applied, defaults and experimental alternates named.
6. `docs/adr/` contains `0001`-`0004` plus one ADR per selected analysis component; ADR 0001 states whether `game-player` exposes per-position value/best-move-set and names the annotation approach Phase 5 will use.
7. Gate A statement: a final section in `docs/component-evaluation.md` titled "Gate A" listing each plan Phase 3 acceptance check with PASS and the evidence path.
