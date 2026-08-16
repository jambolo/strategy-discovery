# Plan: Reusable Strategy Discovery Application

Build a Rust framework whose purpose is automated strategy discovery: generate diverse corpora of games, annotate them with engine ground truth, mine them for human-readable heuristics, and validate those heuristics by benchmark match play. The `jambolo/game-player` minimax engine is one component with three roles — game generation, position annotation, and benchmark opposition — not the discovery engine itself. Game logic (rules, state, move generation, features, evaluation) lives in replaceable modules so tic-tac-toe is the first concrete game and more complex games can be added later without changing the pipeline or orchestration layer.

## Goals and definitions

These definitions fix what "discovery" means for this project and every phase is checked against them:

1. A **discovered strategy** is a human-readable heuristic: an ordered list of rules over game features (e.g., "if a winning move exists, take it; else if the opponent threatens a line, block it; else prefer the center"), emitted with supporting evidence. Rules are stored in a structured form (a small strategy DSL) so they can also be executed by a generic rule-interpreter strategy — required because validation is by match play.
2. **Discovery operates at two levels.** Level one discovers *rules* over a feature vocabulary ("if a fork is available, take it"). Level two discovers the *concepts* themselves — features like "threat" or "fork" that appear nowhere in the game rules. The feature vocabulary is therefore open, not closed: features are composable expressions over rule-derived primitives, new named concepts can be minted at runtime, and hand-authored features are a bootstrap vocabulary, not a ceiling.
3. The **discovery method** is corpus mining: generate many varied games, then analyze the records for feature patterns that correlate with winning (level one, Phase 8) and for feature expressions worth naming as new concepts (level two, Phase 9).
4. **Validation** is benchmark match play against a graded opponent population (random, depth-limited, perfect) plus agreement checks against engine-annotated best moves.
5. The **MVP is the platform** (Phases 1-7): game framework, engine integration, corpus generation, annotation, persistence, analysis plumbing, and the strategy-evaluation harness. Rule mining lands in Phase 8 and concept induction in Phase 9, the post-MVP iterations — but every platform decision below must answer "can Phases 8-9 consume this?"

## Steps

1. Phase 1 - Project baseline and dependency setup.
2. Phase 2 - Core abstractions and tic-tac-toe domain model.
3. Phase 3 - Component evaluation and selection (analysis/mining stack focus).
4. Phase 4 - game-player integration, strategies, and match engine.
5. Phase 5 - Corpus generation, annotation, and persistence.
6. Phase 6 - CLI workflow and reproducible experiment config.
7. Phase 7 - Strategy-evaluation harness, validation, and hardening.
8. Phase 8 (post-MVP) - Heuristic mining and reporting.
9. Phase 9 (post-MVP) - Concept induction.

### Phase 1 - Project baseline and dependency setup

1. Correct Cargo manifest baseline and add required dependencies in root package: `game-player` (git dependency pinned to commit/tag), `clap`, `serde`, `serde_json`, `toml`, `anyhow`, `thiserror`, `tracing`, `tracing-subscriber`, `rand` plus a seedable RNG (`rand_chacha`), `rayon`.
   - Analysis-stack crates (`polars`, `linfa`/`smartcore`) are not baseline dependencies; they are selected in Phase 3 and added when the analysis path lands.
   - `bincode` is dropped from the baseline: corpus records use analysis-friendly formats (JSONL; optionally Parquet per Phase 3). Add a binary replay format only if a concrete need appears.
2. Restructure source tree into framework-oriented modules under `src/`: `core`, `games`, `strategy`, `discovery`, `io`, `cli`.
3. Define crate-level architecture contracts and module boundaries in top-level docs, including two cross-cutting design rules:
   - The pipeline is composable stages with file handoffs: `generate -> annotate -> analyze -> report`.
   - Match simulation must be embarrassingly parallel (rayon): game state is cheap to clone and shares no mutable state across games.
4. Add compile-only smoke tests ensuring all modules wire correctly.
5. Dependency note: Step 2 and Step 3 depend on Step 1.

Acceptance checks:

- `cargo check` succeeds.
- `cargo test` succeeds with module smoke tests.

### Phase 2 - Core abstractions and tic-tac-toe model

1. Define framework traits/types:
   - `GameDomain` (state/action/outcome typing)
   - `GameRules` (initial state, legal actions, apply action, terminal detection)
   - `GamePrimitives` (structural facts a game declares with no strategic insight: position topology, the win-condition lines, the symmetry group; the framework derives further primitives from these mechanically)
   - `FeatureExtractor` (state -> named feature values over the tiered vocabulary of Step 2; what the mining pipeline and rule DSL operate on — the framework's key reusability lever)
   - `Canonicalize` (optional: state -> canonical form under symmetry; games without cheap canonical forms skip it)
   - `StateEvaluator` (game-specific static evaluation contract for search engines)
   - `Strategy` (choose action given state; engine-backed, random, and later rule-interpreter implementations)
   - `StrategyGenerator` (produces candidate strategies; the corpus miner is the first planned implementation, with evolutionary search and LLM generation as future implementations behind the same contract)
   - `MatchEngine` (parallel play loop over strategy providers)
2. Define the feature system as an open, tiered vocabulary (concept discovery in Phase 9 depends on this shape):
   - Tier 1, rule-derived primitives: obtained from `GamePrimitives` with no human strategic insight, including primitives the framework derives mechanically — e.g., symmetry orbits, from which tic-tac-toe's {center}, {corners}, {edges} cell classes fall out of the symmetry group rather than being hand-coded.
   - Tier 2, hand-supplied features: human strategic insight encoded per game (threats, forks). A bootstrap vocabulary and a benchmark for Phase 9 — not a ceiling.
   - Tier 3, invented concepts: named features minted at runtime as expressions over lower tiers. Phase 9 produces them; the MVP only has to be able to represent, evaluate, and persist them.
   - Features are composable expressions in a small feature algebra: serializable, evaluable against states, with pretty-printable definitions. Nothing in core assumes a fixed feature set.
3. Define the heuristic-strategy representation (strategy DSL) as a first-class core type, even though no miner emits it until Phase 8:
   - An ordered decision list: rules of the form condition-over-features -> action selector, with thresholds and priorities.
   - The framework owns the rule *forms*; each game supplies the *vocabulary* via the tiered feature system. No game-specific concepts are hardcoded in core.
   - Requirements: serializable, pretty-prints to human-readable text, executable by a generic rule-interpreter `Strategy`.
   - Self-containment: an emitted heuristic carries the definitions of every non-primitive feature it references, so heuristics over invented concepts remain fully human-readable.
   - Reserve `heuristic-rules` as a named strategy kind in the registry design; persistence must be able to store it.
4. Implement tic-tac-toe domain objects:
   - Board representation (cheap to clone), player representation, move representation
   - Rule checks (wins/draw/legality)
   - `GamePrimitives` declaration (grid topology, the 8 win lines, the 8-element symmetry group)
   - Tier-2 feature extractor (line occupancy counts, immediate threats, forks)
   - Symmetry canonicalization (8 symmetries)
5. Add conversion layer from framework state/action to `game_player::State` and compatible action types.
6. Add deterministic unit tests for terminal states, illegal move rejection, draw handling, whose-turn transitions, symmetry-orbit derivation ({center}, {corners}, {edges} recovered from the symmetry group, not hand-coded), and symmetry (state enumeration matching the known counts: ~5,478 legal positions, 765 up to symmetry).
7. Add micro-benchmarks (non-failing, logged) for state clone cost and move-generation throughput; run them from Phase 2 onward so performance cliffs are caught early rather than in Phase 7.
8. Dependency note: All steps depend on Phase 1.

Acceptance checks:

- `cargo test ttt` (or module-targeted tests) validates rules, features, canonicalization, and state transitions.
- Deterministic tests cover all 8 winning lines plus draw board; enumeration matches known position counts; cell orbits are derived, not hand-coded.

### Phase 3 - Component evaluation and selection

Formal scoring effort is aimed at the genuinely contested choice — the analysis/mining stack — while settled plumbing gets short ADR notes instead of full scoring rounds.

1. Contested category (full weighted scoring): analysis/mining components for Phase 8.
   - Dataframe/query layer: `polars` vs. plain serde plus custom aggregation
   - Rule induction: decision trees via `linfa` vs. `smartcore` (trees convert directly into ordered, readable decision lists — a strong fit for the heuristic output format); association/pattern-mining candidates
   - Corpus file format: JSONL vs. Parquet (must be readable by the selected analysis layer)
2. Settled categories (short ADR note each): `game-player` minimax adapter, self-play/experiment runner, JSON/TOML metadata persistence, summary aggregator.
3. Run feasibility spikes:
   - Verify `game-player` supports per-position queries (best move(s) plus value for an arbitrary position, not only whole games) — required by the annotation stage. Measure per-position cost. If unsupported, document the fallback annotation approach.
   - Throughput spike: games/sec for minimax self-play, serial vs. rayon scaling.
   - Record-format spike: write and re-read a corpus in candidate formats; confirm the analysis layer consumes it.
4. Record benchmark and determinism measurements from spikes.
5. Select default components for the analysis path and mark alternates as experimental.
6. Dependency note: Depends on Phase 1; spikes use Phase 2's tic-tac-toe domain once available. Informs Phases 4, 5, and 8.

Acceptance checks:

- `docs/component-evaluation.md` exists with completed scores for the analysis stack and ADR notes for settled components.
- Per-position annotation capability of `game-player` is confirmed (or a fallback is documented).
- At least one throughput benchmark and one reproducibility report exist under artifacts.

### Phase 4 - game-player integration, strategies, and match engine

1. Implement `game-player` adapters for tic-tac-toe:
   - `State` implementation with stable fingerprint
   - `minimax::ResponseGenerator` implementation with no-legal-moves policy compliance
   - `StaticEvaluator` implementation from Alice perspective (per game-player contract)
2. Build `MinimaxStrategy` wrapper exposing the framework `Strategy` interface with:
   - configurable depth
   - seeded random tie-breaking among equal-valued moves (reproducible variety — required for corpus diversity)
   - optional epsilon-random move probability
3. Build strategy registry/factory so strategies are selected by config: `minimax` (parameterized) and `random` initially, with named extension kinds reserved for `heuristic-rules` (Phase 8 interpreter), evolutionary, and LLM-generated strategies.
4. Define the benchmark opponent population as a named, versioned roster: random, depth-1, depth-2, ..., perfect. Evaluating any strategy means playing the whole population, not a single opponent — a strategy that beats only one opponent has exploited that opponent, not been discovered.
5. Implement the match runner: AI vs. AI, parallel across games via rayon, seeded per game. Scripted play is supported for debugging; interactive human-vs-AI input is optional and deprioritized (not on the discovery path).
6. Add regression tests around known tactical positions to verify perfect-play behavior and immediate win/block choices.
7. Dependency note: Steps 1-3 depend on Phases 2-3; Steps 4-6 depend on 1-3.

Acceptance checks:

- `cargo test` includes minimax integration tests and tactical fixtures.
- Perfect-play vs. perfect-play always draws across N distinct tie-break seeds (a stronger correctness check than one deterministic game).
- Non-perfect settings (reduced depth, epsilon, weak-vs-strong pairings) demonstrably produce decisive games — the property that makes corpora minable.
- A parallel batch of matches produces results identical to a serial run given the same seeds.

### Phase 5 - Corpus generation, annotation, and persistence

1. Implement the corpus generation runner for repeated self-play episodes with parameter sweeps designed for diversity as well as reproducibility:
   - search depth per side, epsilon-random rates, tie-break seeds
   - evaluator variants
   - starting position variants / varied openings
   - opponent pairings drawn from the population roster (mixed strengths, not only symmetric)
2. Record position-level game records under a versioned schema. Per move: state (stored or cheaply reconstructible) plus canonical form, legal moves, chosen move. Per game: metadata (strategy params, seeds, config hash), outcome, length. This schema is the contract the Phase 8-9 miners consume — metadata plus move list alone is not sufficient.
3. Implement the annotation stage as a separate pass: label positions with `game-player`'s best move(s) and value. For tic-tac-toe, support exhaustive annotation of all reachable positions, treated explicitly as a small-game special case (larger games will sample).
4. Persist corpora in the analysis-friendly format selected in Phase 3 (JSONL baseline; Parquet optional), with JSON/TOML run metadata.
5. Add replay parser and summary aggregator, including corpus diversity metrics: distinct games, distinct canonical positions visited (coverage against the known 765), decisive-game fraction.
6. Rerun component-evaluation checkpoints to verify selected components still meet thresholds at real corpus sizes.
7. Dependency note: Depends on Phase 4.

Acceptance checks:

- CLI batch run emits result files; the same seed reproduces byte-identical outputs.
- Across different seeds, corpus diversity metrics exceed configured thresholds (canonical-position coverage and a healthy decisive-game mix — a corpus of identical perfect-play draws is a failure, not a success).
- Annotation pass labels a generated corpus and, for tic-tac-toe, can label every reachable position.
- Evaluation Gate B criteria pass for selected components.

### Phase 6 - CLI and config-driven orchestration

1. Implement the pipeline as one command per stage with file handoffs:
   - `play` (scripted match runs; interactive mode optional)
   - `generate` (batch self-play corpus generation)
   - `annotate` (label an existing corpus with engine ground truth)
   - `analyze` (run analyzers over a corpus)
   - `report` (render analyzer output in human-readable form)
   A future `discover` command (Phase 8) chains generate -> annotate -> analyze -> report -> validate.
2. Design `analyze` as a pluggable analyzer registry; summary statistics is merely the first registered analyzer, so Phase 8 miners slot in without CLI or pipeline restructuring.
3. Implement strongly-typed config loading/validation for strategies, sweeps, seeds, episode counts, output paths.
4. Add structured logging and verbosity controls for debugging search and pipeline behavior.
5. Add error taxonomy with actionable messages for invalid game state/configuration.
6. Dependency note: Steps 3-5 can run parallel once the command contract is fixed in Steps 1-2.

Acceptance checks:

- `cargo run -- play ...` executes a single game path.
- `cargo run -- generate --config <file>` then `annotate` then `analyze` runs the staged pipeline over files.
- `cargo run -- report --input <path>` returns readable summarized metrics.

### Phase 7 - Strategy-evaluation harness, validation, and hardening

1. Build the strategy-evaluation (tournament) harness: run any registered strategy against the benchmark opponent population and report headline metrics — loss rate vs. perfect (must be 0 for a sound tic-tac-toe heuristic), win/draw rates vs. weaker opponents, and move-agreement rate against annotated best moves. Tic-tac-toe caveat: against perfect play every game draws, so win rate alone cannot differentiate strategies; the graded population and agreement metrics exist precisely for this.
2. Build the strategy archive: a persisted collection of strategy definitions (including future mined heuristics) with provenance (source corpus, config, seeds), evaluation results, and novelty metadata (how different an entry is from existing entries). Keep interesting strategies, not merely the single best — studying the strategy space is the point.
3. Add integration tests that run end-to-end command flows on fixture configs.
4. Add property-based tests for framework invariants:
   - terminal states have no legal moves
   - applying legal moves preserves board validity constraints
   - canonicalization is idempotent and outcome-preserving
5. Add performance baseline checks (non-failing benchmark logging): games/sec, nodes/sec, per-move latency, rayon scaling.
6. Add architecture docs for onboarding future games: a second game requires implementing `GameRules`, `GamePrimitives`, engine adapters, optionally `Canonicalize`, and optionally a tier-2 feature set — the corpus, annotation, analysis, and evaluation pipeline must need no changes. (Konane can serve as the worked example in this doc; it is illustrative, not a committed target.)
7. Dependency note: Depends on Phases 4-6; documentation can proceed in parallel with final tests.

Acceptance checks:

- Full `cargo test` and integration suite pass in CI.
- Tournament harness evaluates a strategy against the full population and writes results to the archive.
- Documentation includes a step-by-step "add new game module" path touching only game-side traits and adapters.
- Evaluation Gate C criteria pass with documented evidence.

### Phase 8 (post-MVP) - Heuristic mining and reporting

Explicitly out of MVP scope; specified here so Phases 1-7 stay honest about what the platform must support.

1. Build the feature-dataset stage: corpus plus annotations -> per-position feature vectors with labels (game outcome from the position, best-move agreement), canonicalized so symmetric duplicates do not inflate patterns, and controlled for side-to-move and move-number confounds.
2. Implement the first miner as a `StrategyGenerator`: rule induction with the Phase 3-selected library (decision trees convert directly into ordered decision lists over features), operating over the full current vocabulary (tiers 1-2 at first; tier-3 concepts once Phase 9 exists).
3. Emit heuristics in the strategy DSL: a pretty-printed human-readable report with supporting evidence (rule support, outcome correlation) plus the executable form, including the definitions of all non-primitive features referenced (the self-containment requirement).
4. Validate every emitted heuristic with the Phase 7 tournament harness; store it in the archive with provenance, results, and novelty metadata.
5. Add the `discover` command chaining the full pipeline: generate -> annotate -> analyze (mine) -> report -> validate.
6. Future generator implementations behind the same `StrategyGenerator` contract (not scheduled): evolutionary search (mutate/recombine archived rule strategies, seeded by mined heuristics) and LLM-generated strategies.

Acceptance checks (for the future iteration):

- Running `discover` on tic-tac-toe emits at least one human-readable heuristic that never loses against the perfect benchmark opponent.
- Every emitted heuristic has archive provenance sufficient to reproduce it from seeds and config.

### Phase 9 (post-MVP) - Concept induction

The second post-MVP iteration: discover the concepts themselves, not only rules over them. This is research-grade work (constructive induction / predicate invention); it is specified here because the platform's open vocabulary and feature algebra exist to make it possible.

1. Implement a concept-induction stage: search the space of feature-algebra expressions over lower-tier features for predicates that predict position outcomes or agreement with annotated best moves.
2. Promotion criteria: a candidate expression becomes a named tier-3 concept only if it (a) carries predictive power on held-out corpora and (b) makes downstream mined rules measurably simpler or stronger than they are without it.
3. Iterate so concepts build on concepts: a threat as a line pattern; a fork as a move creating two threats.
4. Naming and readability: auto-generate each concept's definition text from its expression; support an optional human labeling pass (e.g., renaming `concept_7` to "fork"). Emitted heuristics carry these definitions per the self-containment requirement.
5. Report the given-vs-discovered split: what fraction of the vocabulary used by final heuristics is tier-1 primitive, tier-2 hand-supplied, tier-3 invented — the honesty metric behind any "discovered" claim.
6. Library note: reuse the Phase 3 analysis stack for scoring and validation; expect the expression search itself to be custom, since Rust has little in the way of inductive-logic-programming libraries.
7. Dependency note: Depends on Phase 8; reuses the tournament harness and archive unchanged.

Acceptance checks (for the future iteration):

- Starting from tier-1 primitives only (the hand-supplied tier-2 vocabulary withheld), the system re-invents threat and fork as named concepts and uses them in a heuristic that never loses to the perfect benchmark opponent.
- Every invented concept referenced by an emitted heuristic has a readable auto-generated definition and archive provenance.

## Component evaluation

Formal evaluation effort is proportional to how contested a choice is. The analysis/mining stack — the genuinely open decision, and the one the "use existing AI libraries" requirement hinges on — gets full weighted scoring; settled plumbing gets a short ADR note each.

### Categories under full scoring

1. Analysis/mining components (drive Phase 8; selected in Phase 3):
   - Dataframe/query layer: `polars` vs. serde-based custom aggregation
   - Rule induction: `linfa` vs. `smartcore` decision trees; association-rule mining options
   - Corpus file format: JSONL vs. Parquet
2. Any category that later acquires a genuine alternative (e.g., a second search-engine adapter).

### Categories settled by ADR note

- Search engine: `jambolo/game-player` minimax adapter (roles: corpus generation, per-position annotation, benchmark opposition)
- Self-play/experiment runner (internal)
- Metadata persistence: JSON/TOML writers
- Summary aggregator (first analyzer)

### Evaluation criteria and weights

Use a weighted score from 1 to 5 for each scored candidate.

1. Reusability across games (weight 0.30)
2. Determinism and reproducibility under seeds (weight 0.20)
3. Performance on target workloads (weight 0.20)
4. Integration complexity in Rust codebase (weight 0.15)
5. Observability and debuggability (weight 0.10)
6. Maintenance risk (weight 0.05)

Weighted score formula:

`final_score = sum(score_i * weight_i)`

Selection threshold:

- Promote component to default path if `final_score >= 4.0`
- Keep as optional/experimental if `3.0 <= final_score < 4.0`
- Reject/defer if `final_score < 3.0`

### Required evaluation artifacts

1. Component decision matrix (analysis stack) in `docs/component-evaluation.md`
2. ADR-style decision notes in `docs/adr/` for each selected component, including the settled categories
3. Benchmark outputs in `artifacts/benchmarks/`
4. Determinism and corpus-diversity check outputs in `artifacts/reproducibility/`

### Evaluation gates

1. Gate A (end of Phase 3): analysis-stack selection completed; per-position annotation capability confirmed.
2. Gate B (end of Phase 5): corpus generation and annotation validated under repeat runs (seeded reproducibility plus diversity thresholds).
3. Gate C (end of Phase 7): tournament harness and archive operational; platform proven reusable via a second-game dry-run checklist.

## Relevant files

- `Cargo.toml` - baseline dependencies including git-pinned game-player, rayon, seedable RNG; analysis crates enter only after Phase 3 selection.
- `README.md` - document architecture, workflow commands, and adaptation guidance.
- `src/main.rs` - reduce to CLI bootstrap and command dispatch entry.
- `src/core/mod.rs` - framework traits (`GameRules`, `GamePrimitives`, `FeatureExtractor`, `Canonicalize`, `StateEvaluator`, `Strategy`, `StrategyGenerator`, `MatchEngine`), the feature-algebra types, and the heuristic-rule DSL types.
- `src/games/tictactoe/mod.rs` - tic-tac-toe domain: rules, features, symmetry canonicalization.
- `src/strategy/minimax.rs` - game-player adapters and `MinimaxStrategy` (depth, seeded tie-breaks, epsilon).
- `src/strategy/registry.rs` - strategy registry/factory with named extension kinds.
- `src/discovery/mod.rs` - corpus generation runner, annotation stage, strategy archive.
- `src/io/mod.rs` - versioned record schema, JSONL/Parquet writers, replay parser.
- `src/cli/mod.rs` - stage-per-command contracts and parsing.
- `tests/` - integration and tactical regression tests.
- `docs/framework-plan.md` - execution copy of this plan (requested deliverable location).
- `docs/component-evaluation.md` - weighted scoring for the analysis stack plus ADR notes for settled components.
- `docs/adr/` - component-level final selection records.
- `artifacts/benchmarks/` - throughput and scaling evidence.
- `artifacts/reproducibility/` - repeat-run determinism and corpus-diversity evidence.

## Verification

1. Build and unit tests: `cargo check && cargo test`.
2. Tactical correctness: run fixture tests for forced win/block positions.
3. Perfect-play soundness: perfect-vs-perfect draws across N tie-break seeds; non-perfect pairings produce decisive games.
4. Corpus reproducibility and diversity: same seed produces byte-identical outputs; across seeds, coverage and decisive-game thresholds are met.
5. Pipeline end-to-end: `generate -> annotate -> analyze -> report` on a fixture config.
6. Strategy evaluation: tournament harness runs a registered strategy against the full opponent population and archives the results.
7. Framework reusability gate: complete a second-game dry-run checklist showing new-game code confined to game-side traits and adapters, with the pipeline untouched.
8. Component selection gate: analysis-stack components meet the weighted-score threshold with recorded evidence.

## Decisions

- In scope (MVP = platform, Phases 1-7):
  - Tic-tac-toe as the only concrete game implementation.
  - Reusable framework abstractions (game traits, tiered feature system with feature algebra, strategy DSL, generator contract, staged pipeline) designed so more complex games and additional discovery methods slot in without core changes.
  - Open feature vocabulary: `GamePrimitives` plus tiered features (rule-derived / hand-supplied / invented), with hand-supplied features explicitly a bootstrap for early mining. The tier split makes it measurable how much of a discovered heuristic's vocabulary was given versus invented.
  - `game-player` for game generation, position annotation, and benchmark opposition.
  - Corpus infrastructure shaped for mining: position-level versioned records, annotation stage, diversity controls, analysis-friendly formats.
  - Strategy-evaluation harness (opponent population, headline metrics) and strategy archive.
  - Evidence-based selection of the analysis/mining stack via weighted scoring.
- Out of scope for MVP:
  - The mining algorithms themselves (Phase 8, the first post-MVP iteration).
  - Concept induction itself (Phase 9, the second post-MVP iteration; the MVP ships only the representation for invented concepts).
  - Evolutionary search and LLM strategy generation (future `StrategyGenerator` implementations; named extension points only).
  - Other game implementations (Konane and similar are illustrative examples, not targets).
  - Neural-network RL stack.
  - Distributed self-play infrastructure.
- Assumptions:
  - `jambolo/game-player` API remains stable for the pinned version and supports (or can be adapted to) per-position best-move/value queries — verified by a Phase 3 spike.
  - Seeded reproducibility is the policy: runs are reproducible given a seed, and seeded stochasticity (tie-breaks, epsilon, mixed pairings) is deliberately used to create corpus diversity. Pure determinism without variety is a non-goal — deterministic perfect-vs-perfect self-play yields one identical draw repeated N times, which is unminable.
  - Benchmarks and reproducibility checks run on a stable hardware/software baseline for fair component comparison.

## Further Considerations

1. Dependency pinning policy recommendation:
   - Option A: pin to git commit SHA for maximum reproducibility (recommended initially).
   - Option B: pin to release tag once a stable published version exists.
2. Crate layout recommendation:
   - Option A: keep single crate with internal modules for rapid MVP (recommended now).
   - Option B: split into workspace crates after MVP stabilizes and a second game begins.
3. Corpus format recommendation:
   - Option A: JSONL only for MVP (recommended if `polars` is not selected).
   - Option B: Parquet alongside JSONL (recommended if `polars` is selected in Phase 3).
4. Exhaustive-analysis special case: tic-tac-toe permits exhaustive annotation and evaluation (~5,478 legal positions). Keep exhaustive modes clearly flagged as small-game accelerants so no discovery method silently depends on enumeration that larger games cannot provide.
