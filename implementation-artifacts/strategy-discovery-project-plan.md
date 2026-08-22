# strategy-discovery — Project Plan

## Overview

- Rust framework for automated strategy discovery in board games: generate corpora, annotate with engine ground truth, mine human-readable heuristics, validate by benchmark match play.
- Authoritative source plan: `docs/plan.md` (its "Phase N" == this plan's "Milestone"). Phases 1–6 are complete and merged to `develop` (HEAD `8573d79`). This project covers plan.md Phases 7, 8, 9 as Milestones 1, 2, 3.
- Target: stable Rust (edition 2024), single crate `strategy-discovery`, CLI binary. Users: the repo owner running experiments locally on Windows 11; CI on Ubuntu + Windows.
- Only concrete game: tic-tac-toe (`src/games/tictactoe/`). Framework must stay game-neutral.

## Architecture

Existing (do not restructure):

- `src/core/` — traits (`traits.rs`: `GameDomain`, `GameRules`, `GamePrimitives`, `FeatureExtractor`, `Canonicalize`, `StateEvaluator`, `Strategy`, `StrategyProvider`, `StrategyGenerator<G>{Input,Candidate; generate(&mut self,&Input,seed)}`, `MatchEngine`), feature algebra (`features.rs`: `Tier{Primitive,Supplied,Invented}`, `FeatureValue`, `FeatureExpr` serde-tagged `op`, `FeatureVector`, `FeatureDef`, `FeatureVocabulary` ordered/forward-reference-free), tier-1 mechanical extractor (`derived.rs` `PrimitiveFeatures`), strategy DSL (`dsl.rs`: `ActionSelector{AnyLegal,TargetIn,Maximize}`, `Rule{name,priority,condition,action}`, `HeuristicStrategy{name,kind,definitions,rules,fallback}` with `validate()` enforcing self-containment, `Display` pretty-print), interpreter (`interpreter.rs`: `RuleInterpreter<G>` — `choose` currently returns `StrategyError::Unimplemented`), `kinds` constants (`minimax`, `random`, `heuristic-rules`, `evolutionary`, `llm`, `scripted`).
- `src/games/tictactoe/` — board, rules, primitives, tier-2 features (`ttt.` prefix: line counts, threats, win/block/fork cell sets), canonicalization, game-player engine adapters, eval, `benchmark_roster()` = `Roster::graded("ttt-benchmark",1,&[1,2,3,4,6],9)` → `random, depth-1..4, depth-6, perfect`, id `ttt-benchmark-v1`, `corpus.rs`.
- `src/strategy/` — `MinimaxStrategy` (depth, seeded tie-break, epsilon), `random`, `scripted`, `registry.rs` (`StrategySpec` internally tagged `kind`, kebab-case; `heuristic-rules` builds `RuleInterpreterProvider`), `roster.rs` (`Roster{name,version,entries}`, `id()="{name}-v{version}"`, `RosterEntry{name,spec}`).
- `src/discovery/` — `bundle.rs` `GameBundle<G>` (rules, players, evaluators, canonicalizer, primitives, default_strategies, full_search_depth, known_canonical_positions; NO feature-extractor field yet), `match_engine.rs` `RayonMatchEngine` + seed formulas (`splitmix64`, `game_seed`, `player_seed`, `opening_seed`), `corpus.rs`/`config.rs` generation + sweeps, `annotate.rs` (solver ground truth + game-player cross-check), `solver.rs` (`ExhaustiveSolver`, `reachable_states`), `analyze.rs` (`Analyzer<G>` trait → `AnalyzerOutput{file,bytes,check_pass}`, `AnalyzerRegistry<G>` = `BTreeMap<String, Arc<dyn Analyzer<G>>>`, `builtin_registry()` = `summary` + `agreement`, `AnalyzeContext` with lazy annotations), `summary.rs`, `agreement.rs`, `experiment.rs` (`ExperimentConfig`, `StrategyFile{schema_version,strategies:Vec<RosterEntry>}`).
- `src/io/` — `schema.rs` (`SCHEMA_VERSION = 1`; file consts `run.json`, `games.jsonl`, `positions.jsonl`, `annotations.jsonl`, `annotate.json`, `summary.json`, `analyze.json`; `GameRecord`, `PositionRecord{state,canonical_state,canonical_transform,legal_actions,chosen_action,chosen_by,outcome,game_length}`, `AnnotationRecord{value:i8,optimal_actions,engine_action,engine_agrees}`), `jsonl.rs`, `replay.rs`, `hash.rs` (`config_hash`, `fnv1a64`, `hex16`).
- `src/cli/` — subcommands `play, generate, annotate, analyze, report, pipeline` (`pipeline.rs` `STAGE_ORDER` = generate→annotate→analyze→report); `games.rs` is the ONLY non-test file naming a concrete game (`KNOWN_GAMES`, `dispatch_game!`); `discover` reserved. Exit codes 0 ok / 1 failure / 2 usage / 3 check-failed. stdout = results, stderr = logs.
- `tests/` — 15 integration/bench files (full-pipeline e2e exists: `cli_experiment.rs`, `cli_pipeline.rs`; timed non-failing benches `*_bench.rs`). No property tests.
- `spikes/` — disposable crates; `spikes/rule-induction-spike` is the reference implementation for the Milestone 2 miner (linfa-trees tree → 16-rule decision list). `src/` never depends on `spikes/`.
- `docs/` — `plan.md`, `component-evaluation.md`, `adr/0001..0010`. `artifacts/benchmarks/`, `artifacts/reproducibility/` — gate evidence.
- `configs/` — `tictactoe-default.toml` (sweep), `tictactoe-experiment.toml` (experiment).

Data flow (file handoffs): `generate` → run dir (`run.json`, `games.jsonl`, `positions.jsonl`) → `annotate` (`annotations.jsonl`, `annotate.json`) → `analyze` (per-analyzer files + `analyze.json` manifest) → `report` (`report.md`). This project adds: strategy evaluation (roster play + agreement → evaluation results → archive), feature dataset + mining (corpus+annotations → feature vectors → decision list → `HeuristicStrategy`), `discover` chaining all of it, and concept induction feeding the vocabulary the miner uses.

## Techniques

- Engine: `jambolo/game-player` minimax (git, pinned `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795` via `branch = "master"` lock), roles: generation, annotation cross-check, benchmark opposition. `search` returns only the chosen action; framework-side `ExhaustiveSolver` supplies values/tie-sets.
- Parallelism: rayon; per-game seeds; serial == parallel results.
- RNG: `ChaCha8Rng` only, seeded via the documented `splitmix64` seed formulas; no thread RNG, no time.
- Determinism: all persisted outputs and stage stdout byte-identical across runs and platforms given seed (no timestamps, durations, hostnames, thread counts, absolute paths, git SHAs, `HashMap` order; all maps `BTreeMap`; JSONL compact + `\n`; `.json` pretty + trailing `\n`). `serde_json` without `preserve_order`: analyzer outputs carry exact bytes.
- Analysis stack (ADRs 0005–0007): serde + custom aggregation (default), JSONL (default), rule induction = `linfa` + `linfa-trees 0.8` + `ndarray 0.16` (default, enters root in Milestone 2; `smartcore` experimental, association mining deferred). Tree → ordered decision list via `DecisionTree::root_node`/`TreeNode::{is_leaf,children,split,prediction}`.
- Heuristics: `HeuristicStrategy` decision lists (priority-ordered rules, condition `FeatureExpr` → `ActionSelector`), self-contained (carry definitions of all non-primitive features), executed by `RuleInterpreter`.
- Property-based testing: `proptest` (dev-dependency, Milestone 1).
- Concept induction (Milestone 3): custom expression search over `FeatureExpr` algebra (no Rust ILP library exists); scoring reuses the mining stack; held-out corpora for promotion.
- Evidence: weighted scoring in `docs/component-evaluation.md` only for genuinely contested choices; settled choices get an ADR in `docs/adr/NNNN-<slug>.md` (Status/Context/Decision/Consequences).

## Constraints

- Toolchain/quality gates on every merged commit: `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` warning-free. `rustfmt.toml` (`max_width = 132`) unchanged.
- Debug `cargo test --workspace` wall budget: ≤ 90 s total at project end (per-milestone budgets are the planner's; fold tests rather than raise the budget). Heavy full-roster/mining runs: small fixtures in tests, release-only timed benches for numbers.
- Root `Cargo.toml` additions permitted: `proptest`, `tempfile`, `assert_cmd` (dev-dependencies, Milestone 1+); `linfa`, `linfa-trees`, `ndarray` (Milestone 2+). Anything else requires an ADR. Never: `polars`, `smartcore`, `arrow`, `parquet`, `criterion`, `bincode`, `sha2` in root. `game-player` pin, features, and no-MCTS stance unchanged.
- Frozen: `SCHEMA_VERSION = 1` and all existing record/document shapes, file-name consts, `config_hash`, seed formulas, CLI contract (subcommand names, exit codes, stdout/stderr split, global `-v`/`-q`, `RUST_LOG`), report section order, `play` transcript format, `pipeline` stage order, pinned regression run_ids (`5eafa65f76637df3`, `933a94e742497e81`, `74bba44805e514a7`, `dea7db96340e923d`, `988795b19ac38e3c`). New documents carry `schema_version: 1` and new file-name consts in `src/io/schema.rs`. Adding fields to existing records requires a schema bump + ADR (avoid; prefer new files).
- Game-name rule: `src/cli/games.rs` is the only non-test `.rs` that names a concrete game; this is enforced as a substring grep over `src/**/*.rs` (also matches paths like `configs/tictactoe-*.toml`). Adding a game = one `dispatch_game!` arm + one `KNOWN_GAMES` entry; pipeline, corpus, annotation, analysis, evaluation layers untouched.
- Tic-tac-toe exhaustive modes (5,478 positions / 765 canonical) are small-game accelerants; nothing on the discovery path may depend on exhaustive enumeration.
- Never modify: `docs/plan.md`, `.github/` workflows, historical `artifacts/**`, `docs/component-evaluation.md` matrices 1–3, existing ADRs (add new ones).
- Markdown (enforced by acceptance greps): blank line after every heading, blank lines around lists/tables, `| --- |` delimiters, fenced blocks always carry a language, American spelling, floats `{:.3}`.
- Evidence artifacts: `artifacts/benchmarks/*.{md,json}`, `artifacts/reproducibility/*.md`, each with the standard `## Header` table (date, git commit, `rustc --version`, OS, CPU, RAM, exact command); scripts embed paths relative to `$PSScriptRoot`; scratch under gitignored `target/`.
- Environment facts for the pipeline: Windows 11, Git Bash + pwsh; `core.autocrlf = true` (never byte-compare checked-out fixtures; normalize CRLF); no `python`; JSON validation via `pwsh ConvertFrom-Json`; hashes via `Get-FileHash`/`sha256sum`; one cargo command per worker tool call; Bash-tool commands > ~8 KB truncate (use Write/Edit); fresh-worktree debug `cargo test --no-run` ≈ 10 s; `cargo test` stderr carries INFO/TRACE lines from the lib test binary — never grep raw stderr for arbitrary words.
- Commit style: plain descriptive messages. Milestone branches `milestone/<n>-<slug>` off `develop`; squash-merged back.

## Out of scope

- Second game implementation (Konane etc. are illustrative only — docs/checklist only).
- Evolutionary and LLM `StrategyGenerator` implementations (kinds stay reserved, `Unimplemented`).
- Neural-network RL, MCTS, distributed self-play.
- Parquet output, polars, smartcore in root.
- Interactive human-vs-AI play; JSON log format; log files; progress bars.
- Changing diversity thresholds / default sweep (the `distinct_game_fraction` scale finding stays documented, not re-litigated).
- `docs/framework-plan.md` execution copy (never created; not required).
- Upstream changes to `game-player`.

## Definition of Done (project)

All on `develop` after Milestone 3 merge, run from repo root:

1. `cargo test --workspace` passes (0 failed) in debug with wall ≤ 90 s; `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` clean.
2. Tactical fixtures + perfect-vs-perfect-draws-across-seeds tests still present and passing (`cargo test --test minimax_tactics`, `cargo test --test match_engine`).
3. `cargo run -- pipeline --config configs/tictactoe-experiment.toml` (or its successor config) succeeds; repeat run byte-identical.
4. `cargo run -- evaluate ...` evaluates a registered strategy against `ttt-benchmark-v1` and writes an archive entry (Milestone 1 DoD commands pass).
5. `cargo run -- discover --config <fixture>` emits ≥ 1 human-readable heuristic with loss rate 0 vs `perfect`, archived with reproducing provenance (Milestone 2 DoD commands pass).
6. `discover` with tier-2 withheld emits a heuristic using ≥ 1 tier-3 concept with loss rate 0 vs `perfect`, plus the given-vs-discovered split report (Milestone 3 DoD commands pass).
7. `docs/adding-a-game.md` exists with the second-game dry-run checklist; Gate C evidence recorded.
8. `docs/component-evaluation.md` Gate C section (or ADR) records harness/archive operational evidence; new ADRs exist for every new component.

## Milestone 1 — Strategy evaluation harness, archive, and hardening

- slug: evaluation-harness
- objective: Make any registered strategy evaluable against the full graded benchmark population with headline metrics, persist results in a strategy archive with provenance and novelty metadata, harden the framework with property-based tests, and document the add-a-game path (plan.md Phase 7, Gate C).
- scope: `src/discovery/` (new tournament/evaluation + archive modules), `src/cli/` (new `evaluate` subcommand; `games.rs` dispatch), `src/io/schema.rs` (new file-name consts + evaluation/archive record types, `schema_version: 1`), `tests/` (proptest suites, evaluate e2e, tournament bench), `docs/` (`adding-a-game.md`, new ADRs, Gate C record), `artifacts/` (Gate C evidence), `README.md` (evaluate section), `Cargo.toml` (dev-deps `proptest`, `tempfile`, `assert_cmd`).
- deliverables:
  - Tournament harness: plays a strategy-under-test against EVERY roster entry in both colors, N seeded games per pairing, rayon-parallel, seed-reproducible; metrics per opponent and headline: loss rate vs `perfect`, win/draw/loss rates vs each weaker opponent, move-agreement rate vs annotated best moves (consumes `annotations.jsonl`; tic-tac-toe can use the exhaustive annotation).
  - Strategy archive: persisted collection (JSON/JSONL under a directory) of strategy definitions (`StrategySpec`, including `heuristic-rules` bodies) with provenance (source corpus run_id, config hash, seeds, roster id), evaluation results, and novelty metadata (distance from existing entries — definition of distance recorded in an ADR). Append/lookup/list API; keeps all interesting entries, not only the best.
  - `evaluate` CLI subcommand (file handoffs: strategies file / spec in, results + archive entry out; byte-identical repeat runs). `heuristic-rules` entries are accepted by the harness but still return `Unimplemented` when played (interpreter lands in Milestone 2) — the harness must report this as an error, not a loss.
  - Property-based tests (`proptest`): terminal states have no legal moves; applying legal moves preserves board validity; canonicalization idempotent and outcome-preserving; serde round-trips for `FeatureExpr`/`HeuristicStrategy`/`StrategySpec`.
  - Performance baseline logging extended to the harness (games/sec, per-move latency, rayon scaling) as non-failing timed benches.
  - `docs/adding-a-game.md`: step-by-step new-game path (`GameRules`, `GamePrimitives`, engine adapters, optional `Canonicalize`, optional tier-2 features, one `games.rs` arm) with Konane as worked illustration; second-game dry-run checklist proving pipeline untouched.
  - Gate C evidence: `artifacts/reproducibility/tournament-determinism.md`, `artifacts/benchmarks/tournament-throughput.{md,json}`, Gate C record in `docs/component-evaluation.md` (new section only) or a new ADR.
- depends-on: nothing (Phases 1–6 already on `develop`)
- definition-of-done:
  - `cargo test --workspace` 0 failed, debug wall ≤ 60 s; fmt/clippy/doc gates clean.
  - `grep -rl "proptest!" tests/ src/` non-empty; tests named for the four invariants above exist and pass.
  - `cargo run -- evaluate --strategies <strategies.toml> --out <dir>` (exact flags per brief) runs `minimax` depth-9 and `random` against `ttt-benchmark-v1`, both colors, writes results + archive; perfect strategy reports loss rate 0.000 vs every opponent; random reports loss rate > 0 vs `perfect`; running twice → byte-identical output files (hash compare).
  - Agreement rate against exhaustive annotation is reported for the evaluated strategies; depth-9 minimax agreement = 1.000.
  - Archive directory lists ≥ 2 entries with provenance + novelty fields; loading the archive back round-trips.
  - `docs/adding-a-game.md` exists and contains a checklist section; `src/cli/games.rs` remains the only non-test `.rs` naming `tictactoe` (substring grep).
  - `artifacts/reproducibility/tournament-determinism.md` and `artifacts/benchmarks/tournament-throughput.md` exist with the standard header.
  - Root `Cargo.toml` gained only `proptest`, `tempfile`, `assert_cmd` (dev-dependencies).
- planner-context: |
    See Architecture, Techniques, Constraints above in full — all apply. Milestone-specific facts:
    - Existing building blocks: `RayonMatchEngine` (`src/discovery/match_engine.rs`), `benchmark_roster()` (`src/games/tictactoe/roster.rs`), `Roster::graded`, `StrategyFile` (`src/discovery/experiment.rs`) as the harness input shape, `AgreementAnalyzer` (`src/discovery/agreement.rs`) computes corpus-side agreement — the harness needs per-strategy-under-test agreement (reuse its join logic). `ExhaustiveSolver` + `annotate` give full tic-tac-toe ground truth (5,478 positions) in ~seconds.
    - `GameBundle<G>` (`src/discovery/bundle.rs`) is the per-game capability bag; extend it rather than adding game-specific code paths. `dispatch_game!` in `src/cli/games.rs` is how a subcommand reaches the bundle.
    - Harness must be game-neutral: roster comes from the bundle, not from tic-tac-toe code.
    - Tic-tac-toe caveat: against `perfect` every sound strategy draws; headline differentiation comes from weaker opponents and agreement rate — report all, never a single scalar only.
    - Novelty metric: planner chooses (e.g., spec-parameter distance for engine strategies, rule-set Jaccard / move-choice disagreement over a fixed position sample for heuristics); record in an ADR; must be deterministic and serializable.
    - Byte-identity rule applies to every evaluate/archive output and stdout.
    - Exit codes: reuse 0/1/2/3 (3 = a `--strict` threshold check failed, e.g., loss rate vs perfect > 0).
    - Test budget: debug `cargo test --workspace` ≤ 60 s after this milestone (currently ~33 s); full-roster runs in tests use few games per pairing; numbers come from release-only timed benches.
    - Known open warts inherited (fix optional if a step already owns the file): `ANALYZE_FILE` not re-exported from `src/io/mod.rs`; lib unit tests emit INFO/TRACE to stderr (`src/cli/logging.rs`).
    - `docs/adding-a-game.md` is documentation only; no second game is implemented.
    - Plan-name `strategy-discovery-m1`; artifacts in `implementation-artifacts/`; working branch `milestone/1-evaluation-harness` (already checked out).

## Milestone 2 — Heuristic mining and `discover`

- slug: heuristic-mining
- objective: Discover level-one strategies: turn an annotated corpus into a feature dataset, induce an ordered decision list with `linfa-trees`, emit it as a self-contained human-readable `HeuristicStrategy`, execute it via `RuleInterpreter`, validate it with the Milestone 1 harness, archive it with provenance, and chain everything behind `discover` (plan.md Phase 8).
- scope: `src/core/interpreter.rs` (implement `choose`), `src/discovery/` (feature-dataset stage, miner as `StrategyGenerator` impl + analyzer registration, validation/archive wiring), `src/discovery/bundle.rs` (feature-extractor capability), `src/cli/` (`discover` subcommand; `analyze`/`report` registry entries), `src/io/schema.rs` (dataset + heuristic-report file consts/types), `Cargo.toml` (`linfa`, `linfa-trees`, `ndarray`), `configs/` (discover config), `tests/`, `docs/` (ADRs, README), `artifacts/` (mining evidence).
- deliverables:
  - Feature-dataset stage: corpus + annotations → per-position feature vectors (tier-1 mechanical + tier-2 supplied, via the bundle's extractors) with labels (outcome from position, best-move agreement / optimal action set), canonicalized so symmetric duplicates do not inflate patterns, controlled for side-to-move and move number; persisted as JSONL with a column manifest; file handoff consumable by `analyze`.
  - Miner: first `StrategyGenerator<G>` implementation — decision-tree induction (`linfa-trees`, per ADR 0006, ported from `spikes/rule-induction-spike`) → ordered decision list → `HeuristicStrategy` with rule support/outcome-correlation evidence, carrying definitions of every non-primitive feature referenced (self-containment via `FeatureVocabulary`). Registered as an analyzer so `analyze`/`report` render it; miner hyperparameters (depth, min-leaf, seed) in config.
  - `RuleInterpreter::choose` executes decision lists: walk rules by priority, evaluate condition against the position's feature env, apply `ActionSelector` (`TargetIn`, `Maximize` via one-ply feature evaluation, `AnyLegal`), fallback; `heuristic-rules` strategies play in the match engine and harness.
  - Validation + archive: every emitted heuristic is evaluated with the Milestone 1 harness and archived with provenance (corpus run_id, config hash, seeds, miner params, vocabulary tiers used), results, novelty.
  - `discover` subcommand chaining generate → annotate → analyze (dataset + mine) → report → evaluate/archive, config-driven, file handoffs, seed-reproducible.
  - Evidence: `artifacts/benchmarks/mining-*.{md,json}`, `artifacts/reproducibility/discover-determinism.md`, ADR(s) for the dataset schema and miner, README `discover` section.
- depends-on: 1
- definition-of-done:
  - `cargo test --workspace` 0 failed, debug wall ≤ 80 s; fmt/clippy/doc gates clean; root `Cargo.toml` gained exactly `linfa`, `linfa-trees`, `ndarray` beyond Milestone 1.
  - `cargo run -- discover --config <fixture>` exits 0 and emits ≥ 1 heuristic: a pretty-printed Markdown report (rules readable as "if <condition> then <action>" with support/correlation evidence and feature definitions) plus the executable JSON form that passes `HeuristicStrategy::validate()`.
  - The emitted heuristic's archive entry shows loss rate 0.000 vs `perfect` across both colors and the harness's seeds, and contains provenance fields sufficient to rerun (run_id, config hash, seeds, miner params).
  - A test plays a `heuristic-rules` strategy through the match engine (no `Unimplemented`); a tactical test shows the interpreter takes an immediate win and blocks an immediate threat for a hand-written decision list.
  - Feature-dataset file exists in the run dir with a column manifest; a test confirms symmetric duplicates are collapsed (row count ≤ canonical position count for an exhaustive corpus).
  - Running `discover` twice with the same config/seed → byte-identical run directory (hash compare), evidence recorded in `artifacts/reproducibility/discover-determinism.md`.
  - `analyze --analyzers <miner-name>` and `report` work standalone on an existing run dir (stage independence preserved).
- planner-context: |
    See Architecture, Techniques, Constraints above in full — all apply. Milestone-specific facts:
    - Milestone 1 delivered the `evaluate` harness + archive API; reuse, do not reimplement.
    - Reference implementation: `spikes/rule-induction-spike` (linfa-trees on a 5478×69 dataset, tree→16-rule list, `DecisionTree::root_node`, `TreeNode::{is_leaf,children,split,prediction}`, `iter_nodes`, `num_leaves`). Port logic into `src/`; `src/` never depends on `spikes/`.
    - Feature sources: `PrimitiveFeatures` (`src/core/derived.rs`, tier-1, mechanical from `GamePrimitives`) and the game's `FeatureExtractor` (tier-2, `src/games/tictactoe/features.rs`). `GameBundle` currently has no feature-extractor field — add one (game-neutral). `PositionRecord` carries no features: derive them from `state` at dataset time; do NOT add columns to `positions.jsonl` (schema frozen).
    - Dataset labels: `AnnotationRecord{value, optimal_actions}` gives outcome-under-perfect-play and the optimal action set per position; exhaustive annotation covers all 5,478 positions for tic-tac-toe but the stage must also work on sampled/non-exhaustive annotations.
    - Decision-list target: condition over features → `ActionSelector`; tree leaves predict an action class (e.g., "play a cell in `ttt.win_cells`") — the planner decides the action-class encoding so that a tree leaf maps onto a DSL rule with `TargetIn{expr}`. Prefer a cell-set vocabulary (tier-1 orbit sets + tier-2 win/block/fork sets) as the action classes.
    - Interpreter semantics must be deterministic: ties inside a `TargetIn` set are broken by a seeded `ChaCha8Rng` from the provider seed (matching `MinimaxStrategy` tie-break policy, ADR 0008).
    - `discover` = `pipeline` stages + mining analyzer + evaluate/archive; reuse `run_experiment` / `STAGE_ORDER` machinery, extend the experiment config with `[discover]`/`[mine]` sections; `[generate]` remains required.
    - Mined heuristic must be non-trivial: the plan's acceptance is "never loses vs perfect", not "matches minimax"; report agreement rate alongside.
    - Test budget: debug `cargo test --workspace` ≤ 80 s after this milestone; mining tests use small corpora; the exhaustive-corpus mining run is a release-only bench or a single bounded test.
    - `linfa` builds on Ubuntu + Windows CI (no BLAS features); keep default features minimal and deterministic (fixed seeds, no threading nondeterminism).
    - Plan-name `strategy-discovery-m2`; artifacts in `implementation-artifacts/`; working branch `milestone/2-heuristic-mining` (already checked out).

## Milestone 3 — Concept induction

- slug: concept-induction
- objective: Discover level-two concepts: search the feature-algebra expression space over lower-tier features for predicates worth naming as tier-3 concepts, promote them by predictive power on held-out corpora and by downstream rule simplification, stack concepts on concepts, auto-generate readable definitions, and report the given-vs-discovered vocabulary split (plan.md Phase 9, soft DoD).
- scope: `src/discovery/` (concept-induction stage: candidate generation, scoring, promotion, naming; vocabulary management), `src/core/features.rs` only if the algebra needs additions (serialization-compatible), miner integration (tier-3 concepts in the vocabulary the Milestone 2 miner consumes), `src/cli/` (`discover` options: withhold tier-2, label map), `src/io/schema.rs` (concept file consts/types), `configs/`, `tests/`, `docs/` (ADRs, README, readability examples), `artifacts/` (evidence incl. given-vs-discovered report).
- deliverables:
  - Concept-induction stage (file handoff): from the feature dataset (tier-1 only when tier-2 is withheld) enumerate candidate `FeatureExpr` expressions (bounded depth/size, deterministic order), score them on train/held-out corpus splits for predicting position outcome and best-move agreement, and promote those that (a) retain held-out predictive power and (b) make the downstream mined decision list measurably simpler or stronger (fewer rules / higher harness metrics) — thresholds in config.
  - Iteration: promoted concepts enter the vocabulary as named tier-3 `FeatureDef`s and later rounds may build on them (threat as a line pattern → fork as a move creating two threats); bounded rounds.
  - Naming/readability: auto-generated definition text from each expression (`FeatureExpr` `Display`), stable generated names (`concept_N`), optional human label map (config/TOML) applied at report time; emitted heuristics carry definitions (self-containment).
  - Given-vs-discovered report: fraction of vocabulary used by final heuristics by tier (1/2/3), per heuristic and overall; similarity of each invented concept to the withheld hand features (`ttt.` threat/fork/win/block sets) reported as an extensional-agreement score over the dataset (informational).
  - `discover` extended: `--withhold-tier2`-style option (exact flag per brief) and concept-induction config; outputs concept files, updated vocabulary, heuristics over tier-3, archive entries with concept provenance.
  - Evidence + ADR: `artifacts/reproducibility/concept-induction-determinism.md`, `artifacts/benchmarks/concept-induction.{md,json}`, given-vs-discovered report artifact, ADR for the search/promotion design.
- depends-on: 2
- definition-of-done:
  - `cargo test --workspace` 0 failed, debug wall ≤ 90 s; fmt/clippy/doc gates clean; no new root dependencies beyond Milestone 2 without an ADR.
  - `cargo run -- discover --config <fixture-with-tier2-withheld>` exits 0; the run directory contains ≥ 1 promoted tier-3 concept file with an auto-generated definition and provenance (source dataset run_id, config hash, seed, round), and ≥ 1 emitted heuristic whose rules reference ≥ 1 tier-3 concept and whose archive entry shows loss rate 0.000 vs `perfect` (both colors, harness seeds).
  - The emitted heuristic's pretty-printed report includes the definitions of every tier-3 concept it references (self-containment check via `HeuristicStrategy::validate()` + grep of definition text in the report).
  - Given-vs-discovered report present in the run dir with per-tier fractions; with tier-2 withheld, tier-2 fraction = 0.000 and tier-3 fraction > 0.000.
  - Similarity-to-hand-features score is reported for each invented concept against `ttt.` threat/fork (informational; no threshold).
  - Repeat run byte-identical (hash compare), recorded in `artifacts/reproducibility/concept-induction-determinism.md`.
  - Promotion criteria test: a concept that fails held-out predictive power or does not simplify/strengthen the downstream list is rejected (unit test with a synthetic candidate).
- planner-context: |
    See Architecture, Techniques, Constraints above in full — all apply. Milestone-specific facts:
    - Research-grade work; DoD is deliberately soft: re-inventing hand threat/fork exactly is reported, not required. Hard requirements: tier-3 concepts are actually used by a heuristic that never loses to `perfect`, readable definitions, provenance, determinism.
    - Expression search is custom (no Rust ILP library). Search space: `FeatureExpr` over tier-1 (`PrimitiveFeatures`: free/mine/theirs sets, per-orbit and per-line counts) — `Cmp`, `And`/`Or`/`Not`, `Count`, `SetOp`, `Contains`, `Arith`; bound size/depth; canonical enumeration order (BTree-based) for determinism; dedupe extensionally equivalent candidates on the dataset.
    - Scoring reuses the Milestone 2 dataset and miner: candidate utility = held-out predictive gain (information gain / accuracy on outcome and agreement labels) + downstream effect (re-mine with the candidate in the vocabulary; compare rule count and harness metrics). Keep the expensive downstream check for a shortlisted top-K.
    - Threat-like concepts for tic-tac-toe are expressible as per-line counts (e.g., a line with 2 mine and 1 free) and fork-like as a count over cells creating ≥ 2 threats — the tier-1 feature set must be rich enough (check `derived.rs`; extend mechanically, never hand-code game concepts in core or in the induction stage).
    - Withholding tier-2 means the bundle's supplied extractor is excluded from the dataset and vocabulary, not removed from code.
    - Milestone 2's miner consumes a `FeatureVocabulary`; tier-3 concepts are `FeatureDef::derived` entries appended in dependency order (`FeatureVocabulary::push` forward-reference-free rule).
    - Harness, archive, `discover` exist from Milestones 1–2; extend config, do not restructure.
    - Test budget: debug `cargo test --workspace` ≤ 90 s; induction tests use tiny datasets and bounded search; full runs are release-only benches.
    - Plan-name `strategy-discovery-m3`; artifacts in `implementation-artifacts/`; working branch `milestone/3-concept-induction` (already checked out).
