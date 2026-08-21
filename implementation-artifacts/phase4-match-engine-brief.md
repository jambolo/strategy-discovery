# phase4-match-engine — Brief

## Goal

Execute Phase 4 of `docs/plan.md` ("game-player integration, strategies, and match engine"): land the `game-player` adapters, `MinimaxStrategy` (depth, seeded tie-breaks, epsilon), `random` and `scripted` strategies, the strategy registry/factory, the named+versioned benchmark opponent roster, and the rayon match runner — with tactical regression fixtures and the four plan acceptance properties proven by `cargo test`. Authoritative spec: `docs/plan.md` lines 106-127 (Phase 4) at commit `a0c285ca9cd4b6ba1b284c6f6b9dfc91923fc228`; line numbers shift if that file is edited — re-verify against live source.

## Context

### Repo state at planning time (commit `a0c285ca9cd4b6ba1b284c6f6b9dfc91923fc228`, branch `feature/phase-4-match-engine`, branched from `develop`)

- `develop` = `a0c285c` (Phase 3 merged). The working branch is a plain feature branch off `develop`; PR back into `develop` when Phase 4 completes (gitflow per `CLAUDE.md`).
- Single Rust crate `strategy-discovery` (lib + thin bin), edition 2024, stable `rustc 1.94.1`. No `[workspace]` table in root `Cargo.toml`.
- Root dependencies (exact; do not add or change): `anyhow 1.0.104`, `clap 4.6.6 (derive)`, `game-player` (git `https://github.com/jambolo/game-player.git`, branch `master`, locked `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`, crate 0.4.0), `rand 0.10.2`, `rand_chacha 0.10.0`, `rayon 1.12.0`, `serde 1.0.229 (derive)`, `serde_json 1.0.151`, `thiserror 2.0.20`, `toml 1.1.4`, `tracing 0.1.44`, `tracing-subscriber 0.3.23`.
- Baseline verified green at planning time: `cargo test` → 118 lib tests + `tests/smoke.rs` (2) + `tests/ttt_bench.rs` (2) pass; `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean (per Phase 3 gate).
- Source tree (all `src/` paths):
  - `lib.rs`: `pub mod cli, core, discovery, games, io, strategy`; consts `NAME`, `VERSION`.
  - `main.rs`: calls `strategy_discovery::cli::run()`.
  - `core/{mod,traits,symmetry,features,derived,dsl,interpreter}.rs` — complete framework core (game-agnostic; may be extended only additively, see Constraints).
  - `games/mod.rs` (`pub mod tictactoe;`), `games/tictactoe/{mod,board,rules,primitives,features,canonical,engine}.rs` — complete tic-tac-toe domain (5478 reachable positions / 765 canonical verified by tests).
  - `strategy/mod.rs`, `discovery/mod.rs`, `io/mod.rs` — doc-comment-only placeholders (2 lines each). `cli/mod.rs` — placeholder `run()` printing name+version (keep as-is; Phase 6).
  - `tests/smoke.rs`, `tests/ttt_bench.rs` (non-failing micro-benchmarks pattern: `#[test]` + `std::time::Instant` + `std::hint::black_box`, `println!("[bench] ...")`, run with `cargo test --release --test ttt_bench -- --nocapture`).
- `spikes/{annotate-spike,throughput-spike,record-format-spike,rule-induction-spike}/` — throwaway crates with their own `Cargo.toml` (`[workspace]` empty table); NOT built by CI; `src/` must never depend on them. They are reference material only:
  - `spikes/throughput-spike/src/main.rs` lines ~33-120: a working spike-local `StaticEvaluator` (+100/-100 win values, open-line-count heuristic), `ResponseGenerator` wrapping `TicTacToeRules::legal_actions`, `splitmix64` per-game seed derivation, `play_game` self-play loop, `run_serial`/`run_parallel` via `rayon::ThreadPoolBuilder` + `into_par_iter().map().collect()`, fnv1a64 byte-identity check.
  - `spikes/annotate-spike/src/main.rs` lines ~86-135: `solve(rules, state, memo) -> Solved` exhaustive memoized negamax over `GameRules` producing game-theoretic value and full optimal-action set (≈40 lines; reusable as a test-local oracle).
- `docs/adr/0001-game-player-minimax-adapter.md` (minimax is the settled engine for generation / annotation / benchmark opposition; `search` exposes no value or best-move set; Phase 4 lands `ResponseGenerator`/`StaticEvaluator` adapters in `src/strategy/`), `docs/adr/0002-self-play-experiment-runner.md` (runner is internal rayon per-game tasks in `src/discovery/`; per-game seed = f(master_seed, game_index); results in `game_index` order; parallel ≡ serial byte-identical; thread count is a performance knob only), `0003`-`0007` (persistence, aggregator, dataframe, rule induction, corpus format — not touched here). ADR numbering continues at `0008`.
- `artifacts/benchmarks/selfplay-throughput.md`: depth-9 minimax self-play 977 games/s serial, 5409 games/s at 16 threads (release). `artifacts/benchmarks/annotation-per-position.md`: `search(.., 9)` mean 6.5 µs/position; game-player's chosen move was in the exhaustive solver's optimal set for all 5478 positions at depth 9 and 10 (0 disagreements).
- CI (`.github/workflows/ci.yml`): `cargo build --workspace --all-targets`, `cargo test --workspace` on Ubuntu + Windows; PRs also `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`. Debug-build test time matters: keep any exhaustive test (5478 positions × searches) under ~30 s in debug.
- Commit style: plain descriptive messages, no conventional-commit prefixes. Prior pipeline artifacts: `implementation-artifacts/phase{1,2,3}-*` (same directory used here).

### Framework core API (`src/core`, read live source for exact signatures; all items below exist at `a0c285c`)

`core/traits.rs`:

- `trait GameDomain: Send+Sync+'static { type State: Clone+Eq+Hash+Debug+Send+Sync+'static; type Action: Clone+Eq+Hash+Debug+Send+Sync+'static; type Player: Copy+Eq+Hash+Debug+Send+Sync+'static; type Outcome: Clone+Eq+Debug+Send+Sync+'static; }`
- `trait GameRules<G>: Send+Sync { fn initial_state(&self)->G::State; fn player_to_move(&self,&G::State)->G::Player; fn legal_actions(&self,&G::State)->Vec<G::Action> /* empty iff terminal */; fn apply(&self,&G::State,&G::Action)->Result<G::State,RulesError>; fn outcome(&self,&G::State)->Option<G::Outcome> /* Some iff terminal */; fn is_terminal(&self,&G::State)->bool (default) }`
- `trait StateEvaluator<G>: Send+Sync { fn evaluate(&self, state:&G::State, perspective:G::Player)->f32 /* higher = better for perspective */ }` — NO implementation exists yet for tic-tac-toe.
- `enum StrategyError { NoLegalActions, Unimplemented{kind:String,detail:String}, Other(String) }`.
- `trait Strategy<G>: Send { fn choose(&mut self, state:&G::State, legal:&[G::Action])->Result<G::Action,StrategyError> /* legal == rules.legal_actions(state); result must be an element of legal */ }` — one instance per game; `&mut self` holds seeded RNG state.
- `trait StrategyProvider<G>: Send+Sync { fn kind(&self)->&str; fn create(&self, seed:u64)->Box<dyn Strategy<G>> }` — object-safe (asserted by tests).
- `struct MatchConfig { games:usize, seed:u64, max_plies:Option<usize> }` (serde). `struct MatchRecord<A,O> { game_index:usize, seed:u64, actions:Vec<A>, outcome:Option<O> }` (serde). `enum MatchError { MissingProvider(String), Strategy(#[from] StrategyError), Rules(#[from] RulesError) }`.
- `trait MatchEngine<G>: Send+Sync { fn run(&self, config:&MatchConfig, players:&[(G::Player, &dyn StrategyProvider<G>)]) -> Result<Vec<MatchRecord<G::Action,G::Outcome>>, MatchError> }` — doc contract: same config + providers ⇒ identical records, serial or parallel. NO implementation exists yet.
- `RulesError::{GameOver, IllegalAction(String)}`.

`core/mod.rs::kinds`: `MINIMAX="minimax"`, `RANDOM="random"`, `HEURISTIC_RULES="heuristic-rules"`. (This effort adds `SCRIPTED="scripted"`, `EVOLUTIONARY="evolutionary"`, `LLM="llm"` — additive.)

`core/interpreter.rs`: `RuleInterpreterProvider<G>::new(HeuristicStrategy)->Result<Self,DslError>` implements `StrategyProvider<G>` with `kind() == "heuristic-rules"`; its `Strategy::choose` deliberately returns `StrategyError::Unimplemented` until Phase 8. `core/dsl.rs`: `HeuristicStrategy` is `Serialize+Deserialize+Clone`, has `validate()`.

### Tic-tac-toe domain API (`src/games/tictactoe`, exists; read live source)

- Re-exports from `games/tictactoe/mod.rs`: `Board, LINES, Move, Outcome, Player, TicTacToe, TicTacToeRules`.
- `TicTacToe: GameDomain` with `State=Board`, `Action=Move(pub usize)` (cell 0..9 row-major), `Player::{X,O}` (X moves first; `.other()`), `Outcome::{Win(Player), Draw}`. All `Copy + Serialize + Deserialize + Hash + Eq`.
- `Board`: `empty()`, `from_cells([Option<Player>;9], to_move)`, `cells()`, `cell(i)`, `to_move()`, `winner()->Option<Player>`, `is_full()`, `place(i, player)->Board` (no legality check, flips side), `parse(&str)->Result<Board,String>`, `Display` (3 rows). `LINES: [[usize;3];8]` rows, cols, diags in that order.
- **`Board::parse` rule (fixtures MUST obey it):** exactly 9 chars over `{X,O,.}`, char `i` = cell `i`; side to move derived from counts — `X` to move when `#X == #O`, `O` to move when `#X == #O + 1`, any other count is `Err`. A fixture string therefore fixes who moves; a step that says "X to move" on a board where `#X == #O + 1` is WRONG (this exact mistake happened in Phase 3; see ledger Revisions there).
- `TicTacToeRules: GameRules<TicTacToe>` (unit struct, `Copy`, `Default`); `legal_actions` = empty cells in ascending index order, empty on terminal; `apply` errors `GameOver` / `IllegalAction`; `reachable_positions()->Vec<Board>` (5478, BFS first-discovery order, includes terminals).
- `games/tictactoe/engine.rs` (exists): `impl game_player::State for Board` (`type Action = Move`; `fingerprint()` unique over all 5478; `whose_turn()`; `is_terminal()` agrees with rules; `apply()`), `impl From<Player> for PlayerId` (`X→Alice`, `O→Bob`) and inverse, helper fns `player_id`, `player_from_id`. This file is where the tic-tac-toe side of the engine glue (see Design) is added.
- Tier-2 extractor `games/tictactoe/features.rs::TicTacToeFeatures`, canonicalizer `canonical.rs::TicTacToeCanonicalizer`, primitives `primitives.rs::TicTacToePrimitives` — not needed by Phase 4 code but available for tests.

### `game-player` 0.4.0 API facts (checkout `%USERPROFILE%\.cargo\git\checkouts\game-player-ac0a30f334ffbd31\0b8ce6f\`; `cargo fetch` if absent)

- `game_player::{PlayerId, State, StaticEvaluator}`; `game_player::minimax::{ResponseGenerator, search}`.
- `enum PlayerId { Alice = 0, Bob = 1 }`, `.other()`. Alice is the MAXIMIZING side.
- `trait State: Clone { type Action: Clone; fn fingerprint(&self)->u64; fn whose_turn(&self)->PlayerId; fn is_terminal(&self)->bool; fn apply(&self,&Self::Action)->Self; }`
- `trait StaticEvaluator { type State; fn evaluate(&self,&Self::State)->f32 /* Alice perspective */; fn alice_wins_value(&self)->f32; fn bob_wins_value(&self)->f32; }`. Terminal states: `evaluate` must return exactly `alice_wins_value()` / `bob_wins_value()` for a win, a value strictly between them for a draw (the crate's own example uses `0.0`). Search stops recursing when a candidate's value reaches the win value for the side to move.
- `trait ResponseGenerator { type State: State; fn generate(&self, state:&Self::State, depth:u32)->Vec<Action>; }` — POLICY: returns empty iff `state` is terminal (`debug_assert_eq!(actions.is_empty(), state.is_terminal())` inside search: a violation panics in debug/test builds). `depth` is 1 at the root and increases by 1 per ply.
- `pub fn search<S,E,R>(sef:&E, rg:&R, s0:&S, max_depth:u32) -> Option<S::Action>` — alpha-beta (fail-soft) with a fresh per-call transposition table; `Rc`/`RefCell` inside (single-threaded per call; safe to call concurrently from separate rayon tasks because nothing is shared; the evaluator and generator are only required to be `&E`/`&R`, not `Sync`). Returns `None` iff the generator returns no actions (terminal). `max_depth = d` looks `d` plies ahead: `search(s, 1)` picks the child with the best static value; a full tic-tac-toe search is `max_depth = 9` (board has at most 9 plies).
- **No value / best-move set / tie enumeration is exposed.** Internally candidates are stably sorted by static (preliminary) value, iterated, and the FIRST candidate reaching the best searched value wins (`is_better` is strict). So the engine's own tie-break is "highest static value, then generation order" — deterministic, no RNG, and NOT seedable from outside. This is the reason for the tie-break design below.
- Value/quality/TT imprecision: values stored in the TT can be alpha-beta bounds rather than exact values (stored whenever a node was not cut off). For tic-tac-toe this never changed move optimality (0 disagreements over 5478 positions at depths 9 and 10, `artifacts/benchmarks/annotation-per-position.md § Cross-check`).
- `rand 0.10` API names used in this repo: `rand::SeedableRng::seed_from_u64`, `rand::Rng::{random, random_range}`, `rand::seq::IndexedRandom::choose`; RNG type `rand_chacha::ChaCha8Rng`. (`gen`/`gen_range` are the OLD names and do not exist in 0.10.)

### Design (decisions fixed by the planner; the decomposer projects these into steps)

Module placement (new files unless marked exists):

| path | contents |
| --- | --- |
| `src/strategy/engine.rs` | Game-agnostic `game-player` glue: trait `EngineGame`, `RulesResponseGenerator`, `AliceEvaluator`, `WIN_VALUE` |
| `src/strategy/minimax.rs` | `MinimaxConfig`, `TieBreak`, `MinimaxProvider<G>`, `MinimaxStrategy<G>`, `root_values` |
| `src/strategy/random.rs` | `RandomProvider<G>`, `RandomStrategy<G>` |
| `src/strategy/scripted.rs` | `ScriptedProvider<G>`, `ScriptedStrategy<G>` |
| `src/strategy/registry.rs` | `StrategySpec`, `EngineBundle<G>`, `StrategyRegistry<G>` |
| `src/strategy/roster.rs` | `Roster`, `RosterEntry`, `Roster::graded(..)` |
| `src/strategy/mod.rs` (exists, placeholder) | `pub mod engine, minimax, random, scripted, registry, roster;` + re-exports |
| `src/discovery/match_engine.rs` | `RayonMatchEngine<G>: MatchEngine<G>`, seed derivation fns, `play_game` |
| `src/discovery/mod.rs` (exists, placeholder) | `pub mod match_engine;` |
| `src/games/tictactoe/engine.rs` (exists) | add `impl EngineGame for TicTacToe` |
| `src/games/tictactoe/eval.rs` | `TicTacToeEvaluator: StateEvaluator<TicTacToe>` |
| `src/games/tictactoe/roster.rs` | `benchmark_roster() -> Roster` (the versioned tic-tac-toe population) |
| `src/games/tictactoe/mod.rs` (exists) | add `pub mod eval; pub mod roster;` + re-exports; add a convenience `engine_bundle() -> EngineBundle<TicTacToe>` |
| `src/core/mod.rs` (exists) | add `kinds::{SCRIPTED, EVOLUTIONARY, LLM}` |
| `tests/minimax_tactics.rs` | tactical fixtures + exhaustive cross-checks |
| `tests/match_engine.rs` | acceptance properties (draws across seeds, decisive games, serial==parallel) |
| `tests/match_bench.rs` | non-failing games/s micro-benchmark |
| `docs/adr/0008-minimax-tie-breaking.md` | ADR for the tie-break mechanism |

`src/core` stays game-agnostic; `src/strategy` and `src/discovery` are game-agnostic (generic over `G: GameDomain`, bounded by `EngineGame` where the engine is involved); everything tic-tac-toe-specific is under `src/games/tictactoe/`.

**Engine glue (`strategy/engine.rs`).**

- `pub const WIN_VALUE: f32 = 100.0;` — `alice_wins_value() = +WIN_VALUE`, `bob_wins_value() = -WIN_VALUE`, draw = `0.0`.
- `pub trait EngineGame: GameDomain where Self::State: game_player::State<Action = Self::Action> { fn player_id(p: Self::Player) -> PlayerId; fn player_from_id(id: PlayerId) -> Self::Player; fn winner(outcome: &Self::Outcome) -> Option<Self::Player>; }` — the ONLY game-side glue a game supplies (tic-tac-toe: `X↔Alice`, `O↔Bob`, `Win(p)→Some(p)`, `Draw→None`). Exact generic-bound syntax is the implementer's call (a `where` clause on the trait, or a supertrait bound on an associated type), as long as `MinimaxStrategy<G>` needs only `G: EngineGame`.
- `pub struct RulesResponseGenerator<'a, G> { rules: &'a dyn GameRules<G> }` implementing `ResponseGenerator<State = G::State>` with `generate(state, _depth) = rules.legal_actions(state)` — satisfies the empty-iff-terminal policy because `GameRules::legal_actions` is specified that way.
- `pub struct AliceEvaluator<'a, G> { rules: &'a dyn GameRules<G>, evaluator: &'a dyn StateEvaluator<G> }` implementing `StaticEvaluator<State = G::State>`: if `rules.outcome(state)` is `Some(o)` → `G::winner(&o)` mapped to `+WIN_VALUE` (Alice), `-WIN_VALUE` (Bob), `0.0` (none); else `evaluator.evaluate(state, G::player_from_id(PlayerId::Alice))` clamped to `[-(WIN_VALUE-1.0), WIN_VALUE-1.0]` so a heuristic can never masquerade as a terminal win. The framework owns terminal values; a game's `StateEvaluator` supplies only the heuristic.

**Tic-tac-toe evaluator (`games/tictactoe/eval.rs`).** `TicTacToeEvaluator` (unit struct, `Copy`, `Default`): for non-terminal boards, `evaluate(board, perspective)` = (number of `LINES` containing no opponent mark) − (number of `LINES` containing no `perspective` mark), range `[-8, 8]`. Terminal boards: return `±WIN_VALUE`/`0.0` consistently with the adapter (the adapter short-circuits terminals anyway). Same heuristic as the Phase 3 spikes, so Phase 4 throughput is comparable to `artifacts/benchmarks/selfplay-throughput.md`.

**`MinimaxStrategy` (`strategy/minimax.rs`).**

- `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct MinimaxConfig { pub depth: u32 /* ≥ 1 */, #[serde(default)] pub epsilon: f64 /* 0.0..=1.0, default 0.0 */, #[serde(default)] pub tie_break: TieBreak /* default SeededUniform */ }`; `#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)] #[serde(rename_all = "kebab-case")] pub enum TieBreak { Engine, #[default] SeededUniform }`. Reject `depth == 0` and `epsilon ∉ [0,1]` at provider construction with `StrategyError::Other`.
- `MinimaxProvider<G: EngineGame> { rules: Arc<dyn GameRules<G>>, evaluator: Arc<dyn StateEvaluator<G>>, config: MinimaxConfig }` implements `StrategyProvider<G>`: `kind() == kinds::MINIMAX`; `create(seed)` → `MinimaxStrategy` holding clones of the `Arc`s, the config, and `ChaCha8Rng::seed_from_u64(seed)`.
- `MinimaxStrategy::choose(state, legal)`:
  1. `legal.is_empty()` → `Err(NoLegalActions)`.
  2. If `legal.len() == 1` → return it (do NOT consume RNG — keeps streams stable across equivalent positions; document).
  3. Epsilon: if `config.epsilon > 0.0` draw `r = rng.random::<f64>()`; if `r < epsilon` → uniform choice from `legal` (`legal.choose(&mut rng)`). If `epsilon == 0.0` draw nothing. The epsilon draw happens BEFORE the search so RNG consumption per ply is fixed (1 draw for the roll when epsilon > 0, + 1 draw for the choice) regardless of search results.
  4. `TieBreak::Engine` → `search(&AliceEvaluator, &RulesResponseGenerator, state, depth).expect(non-terminal)`; return it. (No RNG consumption.)
  5. `TieBreak::SeededUniform` → `let values = root_values(state, legal)`; best = max over values if side to move maps to `PlayerId::Alice`, else min (compare with `f32::total_cmp`); `ties` = every action whose value `== best` (`total_cmp` equality), in `legal` order; return `ties.choose(&mut rng)` (uniform; one RNG draw). Ties are therefore "equal searched value at this depth", NOT equal static value.
- `pub fn root_values(&self, state, legal) -> Vec<(G::Action, f32)>` — Alice-perspective value of each root action at `depth`, computed by **principal-variation replay** because `search` exposes only an action: for each `a` in `legal`: `child = rules.apply(state, a)`; `value = pv_value(child, depth - 1)` where `pv_value(s, k)`: if `rules.is_terminal(s) || k == 0` → `AliceEvaluator.evaluate(s)`; else `a' = search(.., s, k).expect(..)`; `pv_value(rules.apply(s, a'), k - 1)`. Rationale: each `search(s, k)` call returns a move whose depth-`(k-1)` subtree value equals `s`'s depth-`k` value, so replaying the PV to its leaf and statically evaluating the leaf recovers the depth-`k` minimax value of `s`. Cost: for `b` root moves at depth `d`, `b·(d-1)` searches of decreasing depth — trivially cheap for tic-tac-toe (≈10 ms worst case from the empty board in release). Expose `root_values` publicly (Phase 5 annotation and Phase 7 agreement metrics use it).
- Correctness evidence required (tests, below): (i) for every one of the 5478 positions at depth 9, the engine's own `search` move is in the `SeededUniform` tie set (consistency between the two tie-break modes); (ii) at depth 9 the tie set ⊆ the exhaustive solver's optimal-action set for every non-terminal position (a test-local oracle, ≈40 lines, modelled on `spikes/annotate-spike/src/main.rs::solve`); (iii) perfect-vs-perfect with `SeededUniform` never produces a decisive game across ≥ 32 seeds. If (i) or (ii) fails because of game-player's TT-bound imprecision, the supervisor records the failing positions verbatim in the ledger and escalates; nobody silently weakens the assertion.
- Upstream alternative (documented in ADR 0008, NOT pursued in this effort): the repo owner also authors `game-player`; adding a `search`-variant that returns root candidate values would replace PV replay. The `TieBreak` enum and `root_values` signature are the seam where that would plug in.

**`RandomStrategy` (`strategy/random.rs`).** `RandomProvider<G>` (unit-like, `Default`), `kind() == kinds::RANDOM`; `create(seed)` → `RandomStrategy { rng: ChaCha8Rng::seed_from_u64(seed) }`; `choose` = `legal.choose(&mut rng)` (one draw per ply; `NoLegalActions` on empty).

**`ScriptedStrategy` (`strategy/scripted.rs`).** Debug/test aid (plan step 5 "scripted play"). `ScriptedProvider<G> { actions: Vec<G::Action> }`, `kind() == kinds::SCRIPTED`, `create(_seed)` → `ScriptedStrategy` with a cursor; `choose` returns the next scripted action if it is in `legal`, else `Err(StrategyError::Other("scripted action {a:?} is not legal at ply {n}"))`; when the script is exhausted → `Err(StrategyError::Other("script exhausted after {n} actions"))`. Not part of `StrategySpec` (constructed in code only). Interactive human input is OUT of scope (plan: optional, deprioritized).

**Registry (`strategy/registry.rs`).**

- `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(tag = "kind", rename_all = "kebab-case")] pub enum StrategySpec { Minimax(MinimaxConfig) /* kind "minimax" */, Random /* "random" */, HeuristicRules { heuristic: HeuristicStrategy } /* "heuristic-rules" */, Evolutionary /* "evolutionary", reserved */, Llm /* "llm", reserved */ }` — the tag strings MUST equal the `core::kinds` constants (assert in a test). `MinimaxConfig` is flattened into the variant (`#[serde(flatten)]` or a tuple variant — either; JSON must read `{"kind":"minimax","depth":9,"epsilon":0.0,"tie_break":"seeded-uniform"}`; TOML equivalent must also round-trip since Phase 6 config is TOML).
- `pub struct EngineBundle<G> { pub rules: Arc<dyn GameRules<G>>, pub evaluator: Arc<dyn StateEvaluator<G>> }` (`Clone`).
- `pub struct StrategyRegistry<G>`: `new(bundle: EngineBundle<G>) -> Self` registers the built-in kinds; `register(&mut self, kind: &str, factory: Box<dyn Fn(&StrategySpec, &EngineBundle<G>) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> + Send + Sync>)` for extension; `build(&self, spec: &StrategySpec) -> Result<Box<dyn StrategyProvider<G>>, StrategyError>`; `kinds(&self) -> Vec<&str>` (sorted). Built-ins: `Minimax` → `MinimaxProvider` (validates config), `Random` → `RandomProvider`, `HeuristicRules` → `core::interpreter::RuleInterpreterProvider::new(heuristic)` (DSL validation error → `StrategyError::Other(err.to_string())`), `Evolutionary`/`Llm` → `Err(StrategyError::Unimplemented { kind, detail: "reserved extension kind; no generator implemented (plan.md Phase 8)" })`. An unknown kind string in serialized input is a serde error at parse time (no `other` catch-all).
- Tic-tac-toe convenience: `games::tictactoe::engine_bundle() -> EngineBundle<TicTacToe>` (`TicTacToeRules`, `TicTacToeEvaluator`).

**Roster (`strategy/roster.rs` + `games/tictactoe/roster.rs`).**

- `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct Roster { pub name: String, pub version: u32, pub entries: Vec<RosterEntry> }`, `pub struct RosterEntry { pub name: String, pub spec: StrategySpec }`; `Roster::id(&self) -> String` = `format!("{name}-v{version}")`; `Roster::graded(name, version, depths: &[u32], perfect_depth: u32) -> Roster` builds entries in this order: `random` (`StrategySpec::Random`), one `depth-{d}` per `d` in `depths` (`Minimax { depth: d, epsilon: 0.0, tie_break: SeededUniform }`), `perfect` (`Minimax { depth: perfect_depth, .. }`); entry names must be unique (validate, `Result`).
- Tic-tac-toe population (`games/tictactoe/roster.rs::benchmark_roster()`): `Roster::graded("ttt-benchmark", 1, &[1, 2, 3, 4, 6], 9)` → entries `random, depth-1, depth-2, depth-3, depth-4, depth-6, perfect` (7 entries, id `ttt-benchmark-v1`). Evaluating a strategy means playing EVERY entry (both colours) — the harness that does so is Phase 7; Phase 4 ships the roster and proves every entry builds and plays via the registry.

**Match engine (`discovery/match_engine.rs`).**

- `pub struct RayonMatchEngine<G> { rules: Arc<dyn GameRules<G>>, threads: Option<usize> /* None = rayon global pool */, parallel: bool /* false = plain serial loop */ }` with constructors `new(rules)` (parallel, global pool), `with_threads(rules, n)`, `serial(rules)`. Implements `MatchEngine<G>`.
- Seed derivation (fixed; ADR 0002 requires it documented): `pub fn splitmix64(x: u64) -> u64` (constants `0x9E3779B97F4A7C15`, `0xBF58476D1CE4E5B9`, `0x94D049BB133111EB`, shifts 30/27/31 — as in `spikes/throughput-spike/src/main.rs`); `pub fn game_seed(master: u64, game_index: usize) -> u64 = splitmix64(master ^ (game_index as u64).wrapping_mul(0x9E3779B97F4A7C15))`; `pub fn player_seed(game_seed: u64, slot: usize) -> u64 = splitmix64(game_seed ^ (slot as u64 + 1).wrapping_mul(0xD1B54A32D192ED03))` where `slot` is the index of the player's entry in the `players` slice. `MatchRecord.seed` = `game_seed`.
- `pub fn play_game<G>(rules: &dyn GameRules<G>, players: &[(G::Player, &dyn StrategyProvider<G>)], game_index: usize, game_seed: u64, max_plies: Option<usize>) -> Result<MatchRecord<G::Action, G::Outcome>, MatchError>`: create one strategy per slot via `provider.create(player_seed(game_seed, slot))`; loop: `legal = rules.legal_actions(&state)`; if empty → `outcome = rules.outcome(&state)` (must be `Some`), stop; if `max_plies` reached → `outcome = None`, stop; find the slot whose player `== rules.player_to_move(&state)` (none → `MatchError::MissingProvider(format!("{player:?}"))`); `action = strategy.choose(&state, &legal)?`; if `!legal.contains(&action)` → `Err(MatchError::Strategy(StrategyError::Other(format!("strategy `{kind}` returned illegal action {action:?}"))))`; `state = rules.apply(&state, &action)?`; push action. Duplicate players in `players` → the first slot wins (document) or reject with `MissingProvider`-style error — implementer's choice, but documented and tested.
- `run`: `parallel == false` → `(0..games).map(|i| play_game(.., i, game_seed(seed, i), ..)).collect::<Result<Vec<_>,_>>()`; `parallel == true` → same via `into_par_iter()` (inside `ThreadPoolBuilder::new().num_threads(n).build()?.install(..)` when `threads` is `Some`); rayon's ordered `collect` yields `game_index` order. Which error is reported when several games fail is unspecified (error path only); success output must be identical serial vs parallel (tested).
- No shared mutable state across games: all engine state (strategies, RNGs, game-player's per-call TT) is created inside the per-game task. `RayonMatchEngine` holds only `Arc<dyn GameRules<G>>` (`GameRules: Send + Sync`).

**Tactical fixtures (`tests/minimax_tactics.rs`).** Each fixture below was derived by hand at planning time; the step that uses them MUST re-verify every expected set against the exhaustive oracle before asserting, and fix the fixture (not the code) if they disagree. Board strings follow `Board::parse` (cell `i` = char `i`).

| id | board | to move (by parse rule) | expected optimal set at depth 9 | property |
| --- | --- | --- | --- | --- |
| win-x | `XX.OO....` | X (2 X, 2 O) | `{2}` | take the immediate win |
| win-o | `XX.OO..X.` | O (3 X, 2 O) | `{5}` | take the immediate win |
| block-o | `XX.O.....` | O (2 X, 1 O) | `{2}` | block the immediate threat |
| block-x | `X...OO.X.` | X (2 X, 2 O) | `{3}` | block the immediate threat |
| win-over-block | `XX.OO.X..` | O (3 X, 2 O) | `{5}` | prefer own win over blocking |
| fork-defence | `X...O...X` | O (2 X, 1 O) | `{1,3,5,7}` | avoid corners (X forks); any edge draws |
| empty | `.........` | X | `{0,1,2,3,4,5,6,7,8}` | every opening draws |

Depth-1 (`search(.., 1)`) must also solve `win-x`/`win-o`; depth-2 must solve the block fixtures.

**Acceptance-property tests (`tests/match_engine.rs`).** All with `TicTacToeRules` + `TicTacToeEvaluator`, via `RayonMatchEngine` and the registry:

- `perfect_vs_perfect_always_draws`: depth 9 both sides, `SeededUniform`, `epsilon 0`; `MatchConfig { games: 32, seed: <fixed>, max_plies: None }` → every `outcome == Some(Outcome::Draw)`; additionally at least 2 distinct action sequences appear among the 32 games (proves the seeded tie-break varies play).
- `non_perfect_settings_are_decisive`: each of (a) `random` vs `perfect` (100 games, both colour assignments), (b) `depth-1` vs `depth-9` (100 games), (c) `perfect` self-play with `epsilon 0.25` (200 games) → `≥ 1` game with `Some(Outcome::Win(_))`; and in (a) the perfect side never loses.
- `parallel_equals_serial`: 64 games, depth 4 both, `SeededUniform`, `epsilon 0.1`, same `MatchConfig`: `RayonMatchEngine::serial(..).run(..)` == `RayonMatchEngine::with_threads(.., 4).run(..)` == `::new(..).run(..)` (`Vec` equality AND `serde_json::to_vec` byte equality).
- `max_plies_aborts_with_none_outcome`, `missing_provider_is_an_error`, `illegal_scripted_action_is_an_error`, `scripted_game_replays_exactly` (scripted X vs scripted O reproduces a fixed 9-ply game and its outcome).
- `roster_entries_all_build_and_play`: for every `benchmark_roster()` entry, `registry.build(spec)` succeeds and one game vs `random` completes; `heuristic-rules` builds from a minimal valid `HeuristicStrategy` and `choose` returns `Unimplemented`; `evolutionary`/`llm` return `Unimplemented` from `build`.
- `spec_round_trips_json_and_toml`: every `StrategySpec` variant (except `HeuristicRules`, JSON only is fine) round-trips through `serde_json` and `toml`; tag strings equal `core::kinds`.

**Micro-benchmark (`tests/match_bench.rs`).** Non-failing, same style as `tests/ttt_bench.rs`: depth 9 `SeededUniform` self-play, 200 games, serial vs `RayonMatchEngine::new` → print `[bench] games/s serial=… parallel=… speedup=…`; also 200 games `TieBreak::Engine` to show PV-replay overhead. Assert only that both runs completed.

**ADR 0008 (`docs/adr/0008-minimax-tie-breaking.md`).** Sections Status / Context / Decision / Consequences, same style as `0001`-`0007`. Context: `search` returns only an action; engine tie-break is static-value-then-generation-order; plan requires seeded tie-breaking among equal-valued moves. Decision: PV-replay `root_values` + `TieBreak::{Engine, SeededUniform}`; `EngineGame` glue; `WIN_VALUE` convention; seed derivation formulas (cross-reference ADR 0002). Consequences: cost `b·(d-1)` extra searches per move (fine for tic-tac-toe; larger games should prefer an upstream `game-player` API returning root values — documented option since the repo owner authors `game-player`); `TieBreak::Engine` preserved for annotation cross-checks (ADR 0001).

## Constraints

- Toolchain: stable Rust 1.94.1, edition 2024; dev machine Windows 11, CI also Ubuntu — all code platform-neutral. Commands in step files in PowerShell-compatible form (plain `cargo ...` invocations).
- Root crate must pass at every merged commit: `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Root `Cargo.toml` `[dependencies]` byte-identical to `a0c285c` — no `criterion`, `proptest`, `polars`, `linfa`, `smartcore`, `ndarray`, `bincode`, nothing. `game-player` pin unchanged (`0b8ce6f`); no `game-player` features enabled.
- `src/core` may change only additively (new `kinds` constants). Existing trait signatures, `MatchConfig`, `MatchRecord`, error enums: unchanged. If an implementer believes a core change is unavoidable, it is a deviation to report, not a silent edit.
- `src/core`, `src/strategy`, `src/discovery` contain no tic-tac-toe references outside `#[cfg(test)]` code (tests in those modules MAY use tic-tac-toe as the concrete game). All tic-tac-toe production code under `src/games/tictactoe/`.
- `src/` never depends on `spikes/`. Spikes are not modified.
- Every RNG is `ChaCha8Rng` seeded from a `u64` derived by the documented formulas; no `rand::rng()`/thread RNG, no `HashMap` iteration order feeding any decision (use `Vec`/`BTreeMap`), no time- or thread-dependent behaviour on the success path.
- Debug-build `cargo test` wall time for the whole new suite ≤ ~60 s on the dev machine; exhaustive 5478-position checks are allowed but must stay within that (they may be `#[ignore]`d only with an explicit justification, and the gate step must run them with `-- --ignored`).
- `unsafe` forbidden. Public items documented (`///`) — `cargo doc` is built in CI for `master`.
- `docs/plan.md` is NOT modified. CI workflows are NOT modified.

## Assumptions

- The working branch `feature/phase-4-match-engine` (branched from `develop` at `a0c285c`) is where all commits accumulate; merging it back to `develop` is the user's manual step, outside this pipeline.
- PV replay recovers depth-`k` minimax values exactly for tic-tac-toe (alpha-beta root decisions are exact; the Phase 3 cross-check showed game-player's move choice is always optimal). If the exhaustive tests disprove this, the fallback is the upstream `game-player` API addition (a user action) — the plan does not silently degrade to static-value ties.
- `game_player::minimax::search` is safe to call from multiple rayon tasks concurrently because each call owns its TT and takes only `&E`/`&R` (`Rc`/`RefCell` are per-call internals). Verified by the Phase 3 throughput spike (serial ≡ parallel at 16 threads).
- `heuristic-rules` stays non-playing (`Unimplemented`) in this phase, per `core/interpreter.rs`; the registry only has to build it.
- The hand-derived tactical fixture sets are correct; they are nonetheless re-verified against the oracle by the step that uses them.

## Out of scope

- Corpus records beyond `MatchRecord` (position-level schema, canonical forms, JSONL writers) — Phase 5 (`src/io`, `src/discovery` corpus runner, annotation stage, exhaustive solver in `src/`).
- Parameter sweeps / mixed-pairing generation over the roster — Phase 5 (the roster type is the input to that).
- CLI subcommands (`play`, `generate`, …), TOML config loading, logging setup — Phase 6. `src/cli` stays a placeholder; `tracing` usage optional and non-behavioural.
- Tournament harness, agreement metrics, strategy archive, property-based tests, second-game docs — Phase 7.
- Rule-interpreter execution (Phase 8), mining, concept induction.
- Interactive human-vs-AI play (plan: optional, deprioritized).
- Modifying `game-player` upstream or bumping its pin. MCTS.
- Evidence artifacts under `artifacts/` (not required by plan Phase 4 acceptance; the bench test logs numbers).

## Definition of Done (project)

All checked on branch `feature/phase-4-match-engine` at the final commit:

1. `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` all exit 0; `git diff a0c285c -- Cargo.toml` is empty.
2. `src/strategy/{engine,minimax,random,scripted,registry,roster}.rs`, `src/discovery/match_engine.rs`, `src/games/tictactoe/{eval,roster}.rs` exist and are wired (`pub mod`) and documented; `src/games/tictactoe/engine.rs` implements `EngineGame for TicTacToe`; `core::kinds` has `SCRIPTED`, `EVOLUTIONARY`, `LLM`.
3. `cargo test --test minimax_tactics` passes and includes: all seven fixtures in the table (expected sets re-verified against a test-local exhaustive oracle); engine move ∈ `SeededUniform` tie set for all 5478 positions at depth 9; tie set ⊆ oracle optimal set for all non-terminal positions at depth 9; depth-1 and depth-2 fixtures.
4. `cargo test --test match_engine` passes and includes `perfect_vs_perfect_always_draws` (32 seeds, ≥ 2 distinct games), `non_perfect_settings_are_decisive` (random-vs-perfect, depth-1-vs-depth-9, epsilon-0.25 self-play each decisive; perfect never loses), `parallel_equals_serial` (Vec and JSON-byte equality across serial / 4 threads / global pool), the error-path tests, `roster_entries_all_build_and_play`, `spec_round_trips_json_and_toml`.
5. `cargo test --release --test match_bench -- --nocapture` prints `[bench]` games/s lines for serial and parallel depth-9 self-play and completes.
6. `docs/adr/0008-minimax-tie-breaking.md` exists with Status / Context / Decision / Consequences and records the PV-replay mechanism, `WIN_VALUE`, seed-derivation formulas, and the upstream-API alternative.
7. Gate statement: `implementation-artifacts/phase4-match-engine-gate-report.md` lists each `docs/plan.md` Phase 4 acceptance check (lines 122-127 at `a0c285c`) with PASS and the test name / command that evidences it.
