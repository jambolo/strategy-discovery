# strategy-discovery-m3 — Brief

## Goal

Execute Milestone 3 of `implementation-artifacts/strategy-discovery-project-plan.md` (= `docs/plan.md` Phase 9, soft DoD): search the `FeatureExpr` space over lower-tier features for predicates worth naming as tier-3 concepts, promote them by held-out predictive power plus measurable downstream rule-list improvement, let later rounds stack concepts on concepts, auto-generate readable definitions with stable `concept_N` names and an optional human label map, report the given-vs-discovered vocabulary split and each concept's extensional similarity to the withheld hand features, and extend `discover` so a tier-2-withheld run emits ≥ 1 heuristic that references ≥ 1 tier-3 concept and never loses to `perfect` — all byte-deterministic on repeat runs. Hard requirements: tier-3 concepts actually used by a loss-0 heuristic, readable definitions, provenance, determinism. Soft: re-inventing hand threat/fork exactly is reported (similarity), never required.

## Context

### Repo state at planning time (commit `c997c351f636bc80b7b9bde46b7aee7a309f5f20`, branch `milestone/3-concept-induction`)

- `develop` = `ee071db` (M2 squash) is fully merged; `c997c35` on the working branch edits only `implementation-artifacts/strategy-discovery-project-ledger.md`. Source tree identical to `develop`. Remote default branch `origin/develop`. Squash-merged back by the lead-developer when the milestone DoD passes; workers never touch `develop`.
- Toolchain: `rustc 1.94.1 (e408947bf 2026-03-25)`, edition 2024, single crate `strategy-discovery` 0.1.0, `rustfmt.toml` (`max_width = 132`).
- Baseline gates at `c997c35` (debug, warm): `cargo test --workspace` = 26 `test result:` lines (lib 362 tests + main + doctests + 23 integration binaries), 0 failed, wall ~47 s. fmt/clippy/doc clean. Milestone budget: ≤ 90 s (project cap; ~43 s headroom — fold tests rather than raise it).
- Root `Cargo.toml` `[dependencies]`: anyhow, clap (derive), game-player (git pin `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`), linfa 0.8.1, linfa-trees 0.8.1, ndarray 0.16.1, rand 0.10, rand_chacha 0.10, rayon, serde, serde_json (`float_roundtrip`, no `preserve_order`), thiserror, toml, tracing, tracing-subscriber. `[dev-dependencies]`: assert_cmd, proptest, tempfile. **This milestone adds ZERO root dependencies** — `git diff c997c35 -- Cargo.toml` must stay empty.

### Source-tree facts (at `c997c35`; line counts; re-verify live — edits shift lines)

- `src/core/`: `features.rs` (1119 — the algebra; UNCHANGED this milestone, see D6), `derived.rs` (370), `featurizer.rs` (423), `dsl.rs`, `interpreter.rs`, `symmetry.rs`, `traits.rs`, `mod.rs`.
- `src/discovery/`: `dataset.rs` (496), `mine.rs` (1050), `tree.rs` (415), `discover.rs` (206), `experiment.rs` (1007), `analyze.rs` (builtin registry = agreement, dataset, mine, summary), `bundle.rs` (116), plus corpus/annotate/solver/summary/agreement/evaluate/archive/tournament/config/match_engine, `mod.rs`.
- `src/io/schema.rs` (1613): consts through `DISCOVER_FILE`, `MINE_ENGINE_CART`/`MINE_ENGINE_LINFA_TREES`; types through `DiscoverManifest`.
- `src/cli/`: `mod.rs` (199 — `Command::Analyze` is an inline struct variant with 13 fields; `analyze::run` takes them positionally with `#[allow(clippy::too_many_arguments)]`), `analyze.rs`, `discover.rs` (80), `pipeline.rs` (`run_stages` pub(crate), prints stage lines, never the `pipeline=` trailer), games.rs, error.rs, others.
- `tests/`: 23 integration files incl. `cli_discover.rs` (4 tests), `dataset.rs` (4), `mine.rs` (3), `mining_bench.rs` (1 test, 14 `[bench]` lines), `interpreter_tactics.rs`; `tests/fixtures/`: `generate-small.toml` (pinned run_id `5eafa65f76637df3`: cells=36 games=144 positions=1083; corpus annotate → 390 records; dataset → 190 rows × 70 cols), `discover-small.toml`, `roster-tiny.toml` (random, depth-2, perfect), `strategies-eval.toml`, `strategies-heuristic.toml`, `experiment-small.toml`, `generate-draws.toml`, `generate-small-explicit.toml`.
- `configs/`: `tictactoe-default.toml`, `tictactoe-experiment.toml`, `tictactoe-discover.toml` (M2 pinned full-scale fixture: inline sweep seed 20260823, games_per_cell 40, `random_opening_plies = [0, 1, 2]`, strategies random/depth-2 (minimax 2)/depth-4/perfect (9), evaluators `["default"]`; `[annotate] enabled`; `[analyze] analyzers = ["summary", "agreement", "dataset", "mine"]`; `[mine] engine = "cart"`, `depths = [6, 8, 12, 0]`, `min_leaf = 1`; `[discover] games = 20`, `seed = 0`; `[report] out = "report.md"`). ALL three are FROZEN this milestone.
- `docs/adr/0001..0016` (format: `# ADR NNNN: title` / `## Status` `Accepted — YYYY-MM-DD (Milestone N / plan Phase M)` / `## Context` / `## Decision` / `## Consequences`). Next numbers: 0017, 0018.
- `artifacts/benchmarks/` incl. `mining-induction.{md,json}`; `artifacts/reproducibility/` incl. `discover-determinism.md`. All existing artifacts frozen. Standard header for new evidence (exact form):

```markdown
## Header

| field | value |
| --- | --- |
| date | YYYY-MM-DD |
| git_commit | <full SHA> |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | <exact command> |
```

The `.json` twin carries the same header as a `"header"` object (keys in order `date,git_commit,rustc,os,cpu,ram,build_profile,command`) plus the measured numbers.

- `README.md`: `### analyze` at line ~150, `### discover` at ~299 (its ONLY fenced `text` block is the VERBATIM stdout of `discover --config configs/tictactoe-discover.toml --out target/m2/p3-discover` — a behavior-freeze on the no-induction path), `### Run directory` table at ~421.

### Library API the milestone composes (verified at `c997c35`; re-verify against live source)

```rust
// src/core/features.rs — UNCHANGED this milestone (D6: the existing algebra suffices)
pub enum Tier { Primitive /* "primitive", Display "tier-1" */, Supplied, Invented }  // serde kebab-case; Ord: Primitive < Supplied < Invented
pub enum FeatureValue { Bool, Int(i64), Float, Set(BTreeSet<usize>) }  // type_name() -> "bool"|"int"|"float"|"set"; as_bool/as_int/as_float/as_set
pub enum CmpOp { Eq,Ne,Lt,Le,Gt,Ge }  pub enum ArithOp { Add,Sub,Mul }  pub enum SetOp { Union,Intersection,Difference }
#[serde(tag="op", rename_all="kebab-case")] pub enum FeatureExpr { Const{value}, Ref{name}, Not{expr}, And{exprs}, Or{exprs}, Cmp{#[serde(rename="operator")] op,lhs,rhs}, Arith{operator,..}, Count{expr}, SetOp{operator,..}, Contains{set,element} }
// builders: named,int,boolean,float,set,negate,all,any,compare,arith,count,set_op,contains; references()->BTreeSet<String>; evaluate(&dyn FeatureEnv)
// evaluate facts: And/Or over Bool only (Int operand -> TypeMismatch); Arith over numerics only (Bool -> TypeMismatch); Cmp Lt/Le/Gt/Ge numeric only, Eq/Ne also Bool/Set; Count needs Set
// Display: Ref = name, Cmp = "(lhs op rhs)" with ops == != < <= > >=, Count = "count(expr)", And = "(a and b)", Or = "(a or b)", Not = "(not e)"
pub struct FeatureDef { pub name, pub tier: Tier, pub description, pub expr: Option<FeatureExpr> }  // native()/derived()/is_native(); Display: "name [tier-3] = (expr) -- desc" / "... = <native> -- desc"
pub struct FeatureVocabulary { defs: Vec<FeatureDef> /* private; serializes {"defs":[..]} */ }
// push(def) -> Err(Duplicate) | Err(Unknown) when a derived def references an undefined name (forward-reference-free); validate(); get; contains; defs(); len;
// evaluate(&self, native: &FeatureVector) -> Result<FeatureVector, FeatureError>  — native ∪ derived, computed in list order (THE tier-3 evaluation path)
// closure(&self, names) -> Vec<FeatureDef>  — transitive deps in vocabulary order (self-containment source)
pub struct FeatureVector(pub BTreeMap<String, FeatureValue>);  // FeatureEnv; merged(other) — other wins

// src/core/derived.rs — PrimitiveFeatures<G,R,P>::new(rules, primitives); orbits()/lines()/rules()/primitives()
// definitions() mints, in order: free, mine, theirs (Set); per orbit k: orbit{k}.free (Set), orbit{k}.mine, orbit{k}.theirs, orbit{k}.empty (Int); per line l: line{l}.free (Set), line{l}.mine, line{l}.theirs, line{l}.empty (Int)
// tic-tac-toe: 3 orbits (read live order from SymmetryGroup::orbits — center/corners/edges), 8 lines -> 47 defs. All mover-relative (rules.player_to_move).

// src/core/featurizer.rs — SIDE_TO_MOVE = "side_to_move"
pub struct Featurizer<G> { rules: Arc<dyn GameRules<G>>, primitives, canonicalizer: Option<..>, players, extractors: Vec<Arc<dyn FeatureExtractor<G>>>, vocabulary: FeatureVocabulary }
// new(rules, primitives, canonicalizer, players, supplied: Option<Arc<dyn FeatureExtractor<G>>>) — pushes side_to_move, then PrimitiveFeatures defs, then supplied defs
// vocabulary(); rules(); primitives(); players(); position_count(); side_to_move(state)->Option<usize>; canonical_frame(state)->(State, Permutation);
// extract(state)->FeatureVector  — NATIVE ONLY (side_to_move + extractor outputs; never evaluates derived defs);
// extract_canonical; action_positions(legal, &perm)->Option<Vec<usize>>

// src/discovery/bundle.rs
// GameBundle<G> { name, rules, players, evaluators, default_evaluator, canonicalizer, primitives, supplied_features: Option<Arc<dyn FeatureExtractor<G>>>, default_strategies, roster, full_search_depth, known_canonical_positions }
// featurizer(&self, include_supplied: bool) -> Result<Featurizer<G>, CorpusError>  — the M2 tier-2-withhold switch, prepared for this milestone
// engine_bundle(&self, evaluator) -> EngineBundle { rules, evaluator, featurizer: self.featurizer(true).ok().map(Arc::new) }  — play-time featurizer site 1
// src/games/tictactoe/mod.rs::engine_bundle() — play-time featurizer site 2 (builds Featurizer::new(.., Some(TicTacToeFeatures)) directly)

// src/core/interpreter.rs — RuleInterpreterProvider::new(heuristic, featurizer) validates heuristic.validate() AND every NATIVE def in heuristic.definitions is a name in featurizer.vocabulary() else DslError::UnavailableFeatures.
// RuleInterpreter::choose evaluates env = heuristic.definitions.evaluate(&featurizer.extract(&canonical)) — derived (tier-3) defs already work at play time; natives must exist in the play featurizer.

// src/core/dsl.rs — HeuristicStrategy { name, kind, definitions: FeatureVocabulary, rules, fallback }
// references() = union of every rule condition + selector + fallback refs (DIRECT refs, not transitive); undefined_references(); validate(); ordered_rules(); Display pretty-print

// src/discovery/dataset.rs
pub fn build_dataset<G>(bundle, featurizer, records: &[AnnRec<G>], source: DatasetSource{run_id, annotations_mode}) -> Result<Dataset<G>, CorpusError>
// KEY FACT: it iterates featurizer.vocabulary().defs() and looks values up in native = featurizer.extract(&canonical) — a DERIVED def would be "feature missing from extraction" (Precondition). D4 changes this to vocabulary.evaluate(&native).
// row = one non-terminal CANONICAL state; occurrences increment on collapse; columns kinds from first row; classes = set-kind columns in vocab order;
// qualifying: classes with non-empty C_legal = legal ∩ class and C_legal ⊆ optimal; label = best qualifying by (-tier_rank /* Invented 2 > Supplied 1 > Primitive 0 */, |C_legal| asc, vocab index asc), else "none".
pub fn dataset_from_context<G>(ctx) -> Result<Dataset<G>, CorpusError>  // reads annotate.json + annotations; currently hardcodes ctx.bundle.featurizer(true) — D4 threads a spec
pub struct DatasetAnalyzer;  // "dataset": writes dataset.jsonl itself, returns dataset.json manifest; render -> "## Dataset" + "### Labels"

// src/discovery/mine.rs
// split_train_holdout(rows, seed, fraction) -> (train, holdout)  — private; fraction 0.0 = all-train no RNG; else ChaCha8Rng::seed_from_u64 shuffle, last floor(rows*fraction) held out, both sorted
// split_predicates(kind, name, threshold, engine): bool -> Not(name)/name; int -> Le/Gt(name, floor); set -> Le/Gt(count(name), floor); float per engine
// classify_leaf: soundness relabel — class maximizing #rows whose qualifying contains it, ties lowest class index, max 0 -> "none"
// mine_one(dataset, train, holdout, engine, depth, min_leaf, featurizer) -> MinedHeuristic  — definitions = featurizer.vocabulary().closure(references) (SELF-CONTAINMENT SOURCE), validate() asserted
// Miner<G>{featurizer: Arc<Featurizer<G>>, params: MineParams} : StrategyGenerator<G> — Input=Dataset<G>, Candidate=MinedHeuristic; generate(dataset, seed)
// MineAnalyzer "mine": options.mine.validate(); dataset_from_context; featurizer = ctx.bundle.featurizer(true); writes MiningReport as heuristics.json; render -> "## Mine" + per-candidate "### name"/"#### Rules"/"#### Feature definitions"
// src/discovery/tree.rs — cart: fit(matrix,labels,n_classes,max_depth,min_leaf)->Tree; integer-exact Gini, ties -> first (column, threshold) — a tier-3 column APPENDED LAST loses exact ties to earlier columns, so an emitted list changes iff the concept column strictly wins a split

// src/discovery/analyze.rs
pub struct AnalyzeOptions { pub analyzers: Vec<String>, pub thresholds: DiversityThresholds, pub strict: bool, pub mine: MineParams }
// EXACTLY 12 struct-literal sites at c997c35: src/cli/analyze.rs 1, src/discovery/agreement.rs 3, analyze.rs 6, experiment.rs 1, summary.rs 1 — re-grep `AnalyzeOptions {` when adding a field
// Analyzer<G> { name, description, requires_annotations, run(ctx, options)->AnalyzerOutput, render(&Value)->String /* Markdown starting "## {Name}" */ }
// AnalyzeContext { bundle, corpus_dir, run_id, games, positions, annotations() /* lazy */ }; AnalyzeContext::new(..) is pub
// builtin_registry() registers agreement, dataset, mine, summary; analyze_outputs runs in options order, writes each output.file, then analyze.json (FROZEN shape)

// src/discovery/experiment.rs
// ExperimentConfig<A> { schema_version, name, game, out, generate, #[serde(default)] annotate, analyze, mine: MineParams, discover: DiscoverSection, report }  — deny_unknown_fields
// resolve_experiment(config, base_dir, bundle, registry, out_override) -> ResolvedExperiment { name, game, out, sweep, generate, annotate: Option<AnnotateOptions>, annotate_mode, analyze: AnalyzeOptions, discover: ResolvedDiscover, report_out }
// validates schema_version, name, game==bundle, threads, mine.validate(), analyzer names registered + no dupes + annotations requirement

// src/discovery/discover.rs — run_discover(bundle, resolved, config_hash) reads out/run.json + out/heuristics.json, evaluates every candidate, archives with Provenance{source:"discover", .., discovery: Some(DiscoveryProvenance{..})}, writes evaluation.json + discover.json; returns DiscoverOutcome{report, manifest, novelties, archive_entries}
// src/cli/discover.rs — --config/--out; hash = config_hash(&config) AS LOADED, BEFORE forcing ["dataset","mine"] into analyzers; run_stages(all four); run_discover; print_result; final "discover={name} run_id={} heuristics={} archive_entries={} out={}"; strict check LAST (exit 3 after files written)

// src/io/schema.rs (all frozen unless noted): consts .. DATASET_FILE "dataset.jsonl", DATASET_MANIFEST_FILE "dataset.json", HEURISTICS_FILE "heuristics.json", DISCOVER_FILE "discover.json"
// MineParams { engine "cart", depths [8], min_leaf 1, seed 0, holdout_fraction 0.0 } + validate()  — UNCHANGED this milestone
// DatasetColumn { name, tier: Tier, kind, description }; DatasetManifest { .., tiers: Vec<String>, columns, classes, rows, annotated, nonterminal, collapsed, label_counts, value_counts, side_to_move_counts, unlabeled }
// DatasetRow<S> { schema_version, canonical_state, side_to_move, value: i8, occurrences, legal, optimal, qualifying, label, values: Vec<f64> }  — pub fields, constructible in tests
// MinedHeuristic { name, engine, max_depth, min_leaf, heuristic, rules, leaves, depth, train_rows, holdout_rows, train_accuracy, train_soundness, holdout_accuracy, holdout_soundness, fallback_rows, evidence }
// MiningReport { schema_version, game, run_id, params, dataset: DatasetManifest, candidates }
// DiscoveryProvenance { experiment, config_hash, corpus_run_id, annotations_mode, miner, params: CandidateParams, mine_seed, holdout_fraction, tiers, dataset_rows, candidate }  — gains ONE optional field (D14)
// io helpers: write_json_pretty (pretty + "\n"), read_json, write_jsonl/read_jsonl (compact + "\n"), config_hash(&T)->hex16(fnv1a64(compact json))
```

### Pinned facts that must remain true (regression gates)

- Pinned corpus run_ids `5eafa65f76637df3`, `933a94e742497e81`, `74bba44805e514a7`, `dea7db96340e923d`, `988795b19ac38e3c` reproduce byte-for-byte (`tests/cli_pipeline.rs`, `tests/cli_experiment.rs`, `tests/corpus.rs`).
- Milestone 1 pinned evaluate: `annotate --exhaustive --game tictactoe --out D` → `mode=exhaustive annotated=5478 terminal=958 disagreements=0`; `evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations D --out E` reproduces the pinned stdout and SHA-256s `evaluation.json = 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a`, `archive/archive.json = 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a`, `archive/entries.jsonl = 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8` (proves `Provenance` serialization of non-discover entries is byte-unchanged after D14's optional field).
- Milestone 2 pinned full-scale run: `discover --config configs/tictactoe-discover.toml --out target/m2/p3-discover` (debug) prints the ten-line stdout block embedded verbatim as the README `### discover` fenced `text` block — `run_id=bd760ff0ace48705`, `cells=48 games=1920 positions=14887`, `report=... bytes=48001`, `evaluation=78bcde7c9113b250`, candidates `mined-d6-l1` 0.050/0.970/1.000, `mined-d8-l1` 0.000/0.979/0.039, `mined-d12-l1` 0.000/0.990/0.031, `mined-d0-l1` 0.000/0.990/0.004, `archive_entries=4`; run dir = 16 files; `dataset.json` rows 557, columns 70, classes 17, collapsed 1734; `config_hash=cbcaffd750bb707b`. This MUST still reproduce exactly (the no-induction path is byte-frozen): diff of the fresh run's stdout against the README block must be empty when run with `--out target/m2/p3-discover`.
- `tests/cli_discover.rs` byte-determinism (4 tests) and every other existing test keep passing unmodified except sites D3 names explicitly.
- Tic-tac-toe structural facts: 9 positions, 8 lines (`LINES`), lines through a cell: center 4, corners 3, edges 2 (max per cell = 4); 3 orbits; exhaustive annotation 5478 records / 958 terminal; exhaustive dataset 627 rows × 70 cols; generate-small dataset 190 rows × 70 cols; tictactoe-discover corpus dataset 557 rows × 70 cols. `featurizer(true)` vocabulary = 70 defs (`side_to_move` + 47 tier-1 + 22 `ttt.*`), asserted by `src/games/tictactoe/corpus.rs::featurizer_vocabulary_order_and_count` — that test keeps passing UNCHANGED because `featurizer(include_supplied)` stays non-extended (D3).
- Tier-2 supplied defs (similarity targets, and the withheld set): `ttt.line{0..7}.x`, `ttt.line{0..7}.o` (Int), `ttt.threats.mine`, `ttt.threats.theirs` (Int), `ttt.winning_cells`, `ttt.blocking_cells`, `ttt.fork_cells` (Set), `ttt.win_available` (Bool) — 22 total, mover-relative for the strategic six.
- `cart` tie-breaking: first candidate in (column index, threshold) order wins exact ties — so a promoted concept column (appended after tier-1) appears in a re-mined list iff it STRICTLY improves a split somewhere; D8's promotion gate exploits this.

### Known coupling and stale sites (fix only inside a step that owns the file)

- `src/discovery/bundle.rs::engine_bundle` (line ~53) and `src/games/tictactoe/mod.rs::engine_bundle()` are the ONLY two play-time featurizer construction sites; both must switch to the extended spec (D3) in ONE step or mined-over-extended heuristics fail `UnavailableFeatures` at play.
- `dataset_from_context` is the single context→dataset path; both `DatasetAnalyzer` and `MineAnalyzer` call it. Its signature change (D4) touches both in-crate call sites; tests call `build_dataset` directly with `bundle.featurizer(true)` and stay valid.
- `Command::Analyze` inline variant + positional `analyze::run` cannot absorb ~15 more flags; D13 refactors to a clap `AnalyzeArgs` struct — `tests/cli_analyze.rs`/`cli_agreement.rs` assert flag BEHAVIOR (names, exit codes, stdout), which must stay byte-compatible.
- The README `### discover` fenced `text` block is a live gate on the M2 fixture's stdout; nothing this milestone may change that stdout. New stdout lines appear ONLY for induction-enabled configs (D13).
- Baseline concept-word hit (planner-verified live and at `c997c35`; `git diff c997c35..HEAD -- src/discovery/config.rs` is empty — M3 never touched the file): the ONLY non-test match for `grep -E '\b(threat|fork|corner|center|edge)\b'` over the pre-`#[cfg(test)]` region of every `.rs` under `src/core src/discovery src/io src/cli` is `src/discovery/config.rs` line 40 (line numbers at `c997c35`; the file's `#[cfg(test)]` starts at line 338) — the `NamedOpening` rustdoc example. Phase 4 fix, inside whichever step the decomposer gives the file: on that one line change `name = "center"` to `name = "opening-a"` — nothing else on the line, no other line, docs-only (no behavior, no doctest exists in that comment, no test edits; the `"center"` literals at lines 377/429/601/604/666/669 are `#[cfg(test)]` fixtures and STAY). The game-NAME awk loop is clean at baseline (0 hits). DoD item 11's sweep text is deliberately UNCHANGED — after this reword it honestly reports 0.

## Decisions (the Milestone 3 design — fixed here so the decomposer projects, not invents)

### D1. Module layout and game neutrality

- New library modules: `src/discovery/induce.rs` (candidate grammar, scoring, dedupe, promotion rounds, similarity, `ConceptsAnalyzer` + render), `src/discovery/vocabulary.rs` (`VocabularyAnalyzer` + render — the given-vs-discovered report). Both generic over `G: EngineGame + CorpusGame`; neither names a game or a game concept (`threat`/`fork`/`center`/`corner`/`edge` forbidden outside `#[cfg(test)]`). `src/discovery/mod.rs` gains the two `pub mod`s + re-exports (`ConceptsAnalyzer`, `VocabularyAnalyzer`, induction entry points) — one ordered owner-chain, never co-parallel (M2 lesson).
- Changed: `src/core/derived.rs` (D2), `src/core/featurizer.rs` (D3), `src/discovery/bundle.rs` + `src/games/tictactoe/mod.rs` (D3), `src/discovery/dataset.rs` (D4), `src/discovery/mine.rs` (D12), `src/discovery/analyze.rs` (registry + options), `src/discovery/experiment.rs` (D5), `src/discovery/discover.rs` + `src/cli/discover.rs` (D13, D14), `src/cli/mod.rs` + `src/cli/analyze.rs` (D13), `src/io/schema.rs` + `src/io/mod.rs` (consts/types, re-exports).
- New schema consts: `CONCEPTS_FILE = "concepts.json"`, `VOCABULARY_FILE = "vocabulary.json"` (re-exported from `src/io/mod.rs`). New record types (D5, D10, D11, D14) live in `src/io/schema.rs`, non-generic, so induce.rs/vocabulary.rs/discover.rs can be written in parallel against them.
- `src/core/features.rs` is UNCHANGED (D6 shows the existing algebra suffices). `SCHEMA_VERSION` stays 1; every new document carries `schema_version: 1`.

### D2. Mechanical tier-1 extension (`src/core/derived.rs`; ADR 0017)

- `PrimitiveFeatures` gains a boolean `extended` field: `PrimitiveFeatures::new(rules, primitives)` keeps EXACTLY today's 47-def behavior (`extended = false`); new constructor `PrimitiveFeatures::extended(rules, primitives)` sets it. When set, `definitions()`/`extract()` append two mechanical families derived ONLY from `lines()` + occupancy (no symmetry, no game names), after the existing per-line block, in this order:
  - **Family A — line-signature counts (Int).** Let `L` = max line length. For every signature `(a, b)` with `a + b <= L` and some line of length `>= a + b` (a = positions held by the mover, b = by opponents), in (a asc, b asc) order: `lines.mine{a}.theirs{b}` = number of lines with exactly `a` mover positions and `b` opponent positions. Description text (exact template): `number of lines with exactly {a} positions held by the player to move and {b} held by opponents`. Tic-tac-toe: 10 defs.
  - **Family B — cell-membership sets (Set).** Let `Kmax` = max over positions p of the number of lines containing p. For every signature `(a, b)` with some line of length `> a + b` (a free cell can lie on it), in (a asc, b asc) order, and every `k` in `1..=Kmax` (asc): `cells.mine{a}.theirs{b}.ge{k}` = the set of EMPTY positions p lying on at least `k` lines whose signature is exactly `(a, b)`. Description: `empty positions on at least {k} lines that each have exactly {a} positions held by the player to move and {b} held by opponents`. Tic-tac-toe: 6 signatures × 4 = 24 defs.
- Tic-tac-toe extended tier-1 = 47 + 34 = 81 defs; extended play/dataset vocabulary with supplied = 1 + 81 + 22 = 104; withheld = 1 + 81 = 82. Extensional expectations to ASSERT IN TESTS ONLY (never encode in src): `cells.mine2.theirs0.ge1` ≙ `ttt.winning_cells`, `cells.mine0.theirs2.ge1` ≙ `ttt.blocking_cells`, `cells.mine1.theirs0.ge2` ≙ `ttt.fork_cells`, `lines.mine2.theirs0` ≙ `ttt.threats.mine`, `lines.mine0.theirs2` ≙ `ttt.threats.theirs`.
- Ring-game unit tests (derived.rs `#[cfg(test)]` — 4 positions, 4 length-2 lines, each position on 2 lines): extended defs = base 23 + Family A 6 (`(a,b)` with `a+b<=2`) + Family B 6 (3 signatures × k∈{1,2}) = 35; assert names, order, and hand-computed values after 0/1/2 moves; `new()` still yields 23.

### D3. Featurizer spec, derived defs, and play-time wiring

- `src/core/featurizer.rs` gains `#[derive(Clone, Copy, Debug, PartialEq)] pub struct FeaturizerSpec { pub include_supplied: bool, pub extended: bool }` with `FeaturizerSpec::base()` = `{ include_supplied: true, extended: false }` (M2-equivalent) and `Featurizer::with_spec(rules, primitives, canonicalizer, players, supplied, spec)`; existing `Featurizer::new(..)` delegates with `extended: false` (all current call sites untouched). `with_spec` builds `PrimitiveFeatures::extended(..)` when `spec.extended`, and drops `supplied` when `!spec.include_supplied`.
- `Featurizer::push_derived(&mut self, def: FeatureDef) -> Result<(), FeatureError>`: rejects `def.is_native()` (as `FeatureError::Unknown(def.name)`? NO — add nothing to the error enum: reject with `FeatureError::Duplicate` is wrong too; PIN: `push_derived` returns `Err(FeatureError::Unknown(name))` only via the vocabulary's own checks and asserts `!def.is_native()` with a `debug_assert!` + a documented contract that callers pass derived defs; the real gate is `vocabulary.push(def)`). It appends to the internal vocabulary; `extract()` stays native-only (consumers evaluate derived via `vocabulary().evaluate`).
- `GameBundle::featurizer_with(&self, spec: FeaturizerSpec) -> Result<Featurizer<G>, CorpusError>`; existing `featurizer(include_supplied)` delegates with `extended: false` — so `game_bundle().featurizer(true)` stays 70 defs and the corpus.rs vocabulary test passes unchanged.
- Play-time featurizers become extended at BOTH sites in ONE step: `GameBundle::engine_bundle` fills `featurizer: self.featurizer_with(FeaturizerSpec { include_supplied: true, extended: true }).ok().map(Arc::new)`; `src/games/tictactoe/mod.rs::engine_bundle()` switches to `Featurizer::with_spec(.., Some(Arc::new(TicTacToeFeatures)), FeaturizerSpec { include_supplied: true, extended: true })`. Adding natives to the play env is behavior-neutral for existing heuristics (their conditions reference only carried defs) — every M1/M2 pin must still hold, asserted by the phase gate. New test: a heuristic whose definitions include natives `lines.mine2.theirs0` and `cells.mine2.theirs0.ge1` builds through the registry and plays through the match engine (extends `tests/interpreter_tactics.rs`).

### D4. Dataset over derived vocabularies (`src/discovery/dataset.rs`)

- `build_dataset` changes one thing: after `let native = featurizer.extract(&canonical);` insert `let env = featurizer.vocabulary().evaluate(&native).map_err(|e| CorpusError::Precondition(format!("evaluating derived features: {e}")))?;` and every subsequent lookup (`columns` seeding, kind checks, `values` encoding, class `qualifying` computation) reads `env` instead of `native`. For an all-native vocabulary `env == native`, so every existing output is byte-identical (existing tests prove it).
- `dataset_from_context<G>(ctx, spec: FeaturizerSpec, derived: &[FeatureDef]) -> Result<Dataset<G>, CorpusError>`: builds `ctx.bundle.featurizer_with(spec)?`, pushes every `derived` def in order (`FeatureError` → `CorpusError::Config`), then proceeds as today. Both in-crate call sites updated (DatasetAnalyzer, MineAnalyzer); helper `fn induction_spec(params: &InductionParams) -> FeaturizerSpec` = `{ include_supplied: !(params.enabled && params.withhold_tier2), extended: params.enabled && params.extended_tier1 }` lives beside `InductionParams` consumers in `src/discovery/induce.rs` (pub(crate), reused by dataset/mine analyzers via re-export or a shared location — decomposer picks the module, behavior as stated).
- `DatasetAnalyzer::run` builds the BASE dataset: `dataset_from_context(ctx, induction_spec(&options.induction), &[])` — `dataset.json`/`dataset.jsonl` always describe the GIVEN features (withhold/extension applied, no tier-3). With induction disabled this is byte-identical to M2.

### D5. `InductionParams` (`src/io/schema.rs`; `[induction]` table; ADR 0018)

```rust
#[serde(deny_unknown_fields)] pub struct InductionParams {   // every field #[serde(default = ..)]; derives like MineParams
    pub enabled: bool,                // false
    pub withhold_tier2: bool,         // false — exclude the supplied extractor from dataset+vocabulary (code untouched)
    pub extended_tier1: bool,         // true  — mint D2 families when enabled
    pub rounds: usize,                // 2     — max promotion rounds (>= 1)
    pub max_thresholds: usize,        // 4     — per numeric atom (>= 1)
    pub beam: usize,                  // 64    — level-1 predicates kept for combos (>= 1)
    pub top_k: usize,                 // 8     — shortlist size for the downstream probe (>= 1)
    pub max_promoted: usize,          // 4     — per round (>= 1)
    pub min_train_gain: f64,          // 0.01  — (>= 0.0)
    pub min_holdout_gain: f64,        // 0.005 — (>= 0.0)
    pub holdout_fraction: f64,        // 0.25  — in [0, 1); 0.0 = no holdout (holdout filter skipped; tests only)
    pub probe_depth: usize,           // 6     — cart probe depth (0 = unlimited)
    pub soundness_tolerance: f64,     // 0.005 — allowed soundness drop when rules shrink (>= 0.0)
    pub min_soundness_gain: f64,      // 0.001 — required soundness rise when rules only tie (>= 0.0)
    pub compare_atoms: bool,          // false — enable Family D atom-pair predicates (D6)
    pub seed: u64,                    // 0     — induction train/holdout shuffle
    pub label_map: Option<PathBuf>,   // None  — TOML table `name = "label"`, applied at render time only
}
```

- `validate() -> Result<(), CorpusError>`: bounds above, plus `withhold_tier2 && !enabled` → `Config("[induction] withhold_tier2 requires enabled = true")`. Error strings follow the `[mine]` precedent (`[induction] field ...`).
- `ExperimentConfig` gains `#[serde(default)] pub induction: InductionParams` (top-level `[induction]` table). `AnalyzeOptions` gains `pub induction: InductionParams` (Default = default; ALL 12 literal sites updated). `resolve_experiment` calls `config.induction.validate()`, resolves `label_map` against `base_dir`, and copies into `resolved.analyze.induction`.

### D6. Candidate grammar and enumeration (v1; `src/discovery/induce.rs`; ADR 0018)

- The algebra is NOT extended: v1 enumerates over existing ops `Cmp`, `Count`, `And`, `Or`, `Not` (plus `Cmp(Gt, atomA, atomB)` when `compare_atoms`). `Arith`/`SetOp`/`Contains` remain available in the algebra for hand-written defs but are not enumerated in v1 — recorded with rationale in ADR 0018 (boolean structure carries the tree-compression value; set-valued candidates need class-scoring machinery deferred with the rest of v2).
- Inputs per row: `env_i = vocabulary.evaluate(&featurizer.extract(&rows[i].canonical_state))` computed once per row (the same featurizer/vocabulary the dataset was built with, INCLUDING previously promoted concepts in later rounds). Numeric atom values cached as `i64`; predicate results cached as bit vectors over rows.
- **Atoms** (in vocabulary/column order; `side_to_move` EXCLUDED — it is a confound control, not concept material): numeric atoms = `Ref(c)` for every `int` column, `Count(Ref(c))` for every `set` column; predicate atoms = `Ref(c)` for every `bool` column (this is how round ≥ 2 stacks on promoted concepts and, when tier-2 is present, on `ttt.win_available`).
- **Level 1** (candidate sequence order): every predicate atom; then per numeric atom, thresholds `T` = distinct values of the atom over TRAIN rows ascending, truncated to the first `max_thresholds`; ops in order `[Eq, Ge]`; for `Ge` skip `t == min(T)` (constant-true on train). Note recorded in ADR: `Ge` level-1 candidates are single-split-equivalent and exist mainly as combo material — promotions are expected to concentrate in `Eq` and combos; the D8 probe rejects split-equivalent concepts naturally (they cannot strictly beat the original column at a tie).
- **Family D** (only when `compare_atoms`): `Cmp(Gt, atomA, atomB)` for every ordered pair of numeric atoms (A ≠ B, both orders), appended after level 1. Default OFF; not on the DoD path.
- **Level 2**: rank all deduplicated, gain-filtered level-1 (∪ D) predicates by (train_gain desc, sequence order asc); keep the first `beam`. Then, in beam order: `Not(p)` for each beam member; then for each beam pair `i < j`: `And([p_i, p_j])` then `Or([p_i, p_j])`. Level-2 candidates go through the same dedupe/gain filters.
- **Dedupe** (applies to every candidate, in sequence order): compute the full-row bit vector (train + holdout, dataset row order). Drop as `constant` when all rows equal; drop as `duplicate` when the vector equals (a) the boolified vector of any existing `bool` column, or (b) any earlier KEPT candidate's vector, or (c) any concept promoted earlier in the same run. First occurrence wins (deterministic).

### D7. Scoring and shortlist

- Targets per row: `T_value` = the row's `value` (−1/0/1) and `T_label` = the row's `label` string (classes + `"none"`). Both come from the CURRENT round's dataset; since v1 concepts are `bool` columns, classes and labels are IDENTICAL across rounds and trials (only columns grow).
- Split: `split_train_holdout(rows, params.seed, params.holdout_fraction)` (reuse mine.rs's helper — promote it to `pub(crate)` in a shared location; identical semantics).
- Information gain of predicate bit vector `b` against target `T` over an index set `S`: `IG = H(T|S) − Σ_{v∈{0,1}} (|S_v|/|S|)·H(T|S_v)`, entropy base-2, `0·log(0) = 0`. `train_gain = max(IG(T_value; train), IG(T_label; train))`; `holdout_gain = Some(max(..over holdout..))` when a holdout exists, else `None`.
- Filters: keep iff `train_gain >= min_train_gain` AND (`holdout_gain` is `None` OR `holdout_gain >= min_holdout_gain`). Rejection reasons are counted, not stored per candidate.
- Shortlist: rank kept candidates by (`holdout_gain` if present else `train_gain`) desc, then `train_gain` desc, then sequence order asc; take the first `top_k`.

### D8. Promotion (downstream probe), rounds, naming

- Per round: baseline probe = `mine_one`-equivalent single cart fit over the CURRENT dataset (engine `cart` ALWAYS — never `linfa-trees`, which is not byte-stable; depth `probe_depth`, `min_leaf` 1, ALL rows train, no holdout). Record `(rules, train_soundness)` as `before`.
- For each shortlisted candidate in rank order, until `max_promoted` promotions this round: re-dedupe against concepts promoted this round (skip as `duplicate`); build the trial vocabulary = current + `FeatureDef::derived(name, Tier::Invented, description, expr)` where `name = concept_{n}` (n = 1-based GLOBAL promotion counter) and `description` = `invented concept (round {r})`; rebuild the trial dataset (same rows, one extra bool column); probe again → `after`.
- **Promotion predicate** (pin exactly; unit-testable as a pure function): promoted iff the probe's emitted heuristic `references()` contains the trial concept name AND (
  - `after.rules < before.rules` AND `after.train_soundness >= before.train_soundness - soundness_tolerance`, OR
  - `after.rules <= before.rules` AND `after.train_soundness >= before.train_soundness + min_soundness_gain`
  ). Otherwise `rejected-downstream`.
- On promotion: the concept joins the working vocabulary and dataset; `before := after`; remaining shortlist trials run against the updated baseline (sequential greedy). Round ends after the shortlist; the next round re-enumerates (D6) over the augmented vocabulary iff rounds remain AND this round promoted ≥ 1. Names are stable across runs by construction (deterministic order).
- Promoted concepts are the ONLY tier-3 defs; they are `bool`-kind in v1. Emitted `FeatureDef` Display therefore reads e.g. `` concept_1 [tier-3] = ((lines.mine2.theirs0 >= 1) or ...) -- invented concept (round 1) `` — the auto-generated definition text the DoD greps.

### D9. Similarity to hand features (informational)

- Targets = every def of `bundle.supplied_features` (present or withheld — withholding excludes tier-2 from the dataset, not from code; a game without a supplied extractor yields an empty list). Target values computed per row from the supplied extractor over `rows[i].canonical_state`.
- Boolify: `Bool` → itself; `Int` → `> 0`; `Float` → `> 0.0`; `Set` → non-empty. `agreement(concept, target)` = fraction of ALL dataset rows (train + holdout) where the concept bit equals the boolified target bit.
- Per promoted concept: `similarity: Vec<SimilarityEntry { feature: String, agreement: f64 }>`, sorted agreement desc then feature name asc (deterministic; first entry = best match). Informational only — no threshold anywhere.

### D10. `concepts` analyzer and report (`src/discovery/induce.rs`)

- `ConceptsAnalyzer` — `name() = "concepts"`, `requires_annotations() = true`. `run`: `options.induction.validate()?`; if `!options.induction.enabled` → `CorpusError::Config("analyzer \`concepts\` requires [induction] enabled = true (or --induce)")` (exit 2). Builds the base dataset via `dataset_from_context(ctx, induction_spec(..), &[])`, runs D6–D9, loads the label map if configured (TOML top-level table → `BTreeMap<String, String>`; missing file → `CorpusError::Config`), and returns `AnalyzerOutput::new(CONCEPTS_FILE, &ConceptReport, true)`.
- Persisted shapes (`src/io/schema.rs`):

```rust
pub struct ConceptProvenance { pub run_id: Option<String>, pub params_hash: String /* config_hash(&effective InductionParams) */, pub seed: u64, pub round: usize }
pub struct ConceptScores { pub train_gain: f64, pub holdout_gain: Option<f64>, pub probe_rules_before: usize, pub probe_rules_after: usize, pub probe_soundness_before: f64, pub probe_soundness_after: f64 }
pub struct SimilarityEntry { pub feature: String, pub agreement: f64 }
pub struct PromotedConcept { pub name: String, pub definition: String /* FeatureDef Display */, pub def: FeatureDef, pub kind: String /* "bool" in v1 */, pub round: usize, pub scores: ConceptScores, pub provenance: ConceptProvenance, pub similarity: Vec<SimilarityEntry> }
pub struct ShortlistEntry { pub expr: String /* FeatureExpr Display */, pub train_gain: f64, pub holdout_gain: Option<f64>, pub outcome: String /* "promoted:concept_{n}" | "rejected-downstream" | "duplicate" */ }
pub struct RoundReport { pub round: usize, pub columns: usize, pub enumerated: usize, pub deduplicated: usize /* dropped constant+duplicate */, pub scored: usize, pub passed: usize, pub shortlisted: Vec<ShortlistEntry>, pub promoted: Vec<String> }
pub struct ConceptReport { pub schema_version: u32, pub game: String, pub run_id: Option<String>, pub params: InductionParams /* effective */, pub withheld: Vec<String> /* supplied def names excluded, else [] */, pub rows: usize, pub train_rows: usize, pub holdout_rows: usize, pub base_columns: usize, pub base_classes: usize, pub rounds: Vec<RoundReport>, pub promoted: Vec<PromotedConcept>, pub labels: BTreeMap<String, String> /* label map verbatim, {} when none */ }
```

- `render` → `## Concepts`: overview `| metric | value |` table (rounds, enumerated, scored, promoted, withheld tier-2 count); `### Promoted` — one bullet per concept: `` `{definition}` `` with suffix ` (label: {label})` when `labels` has the name; `### Similarity` table `| concept | feature | agreement |` listing each concept's top 3 entries ({:.3}); `### Rounds` table `| round | columns | enumerated | deduplicated | scored | passed | shortlisted | promoted |`. Markdown rules apply (blank line after every heading, around lists/tables, `| --- |`, floats {:.3}).

### D11. `vocabulary` analyzer — the given-vs-discovered report (`src/discovery/vocabulary.rs`)

- `VocabularyAnalyzer` — `name() = "vocabulary"`, `requires_annotations() = false`. `run`: reads `ctx.corpus_dir/heuristics.json` (missing → `CorpusError::Precondition("heuristics.json not found; run the \`mine\` analyzer first")`); for each candidate: `refs = candidate.heuristic.references()` (DIRECT refs of rules + fallback); resolve each name's tier from the candidate's OWN definitions (self-containment guarantees resolution); build `UsageBreakdown { referenced: refs.len(), counts, fractions }` with keys `"primitive"`, `"supplied"`, `"invented"` ALWAYS all present (0 / 0.0 when absent). `overall` = the same over the UNION of refs across candidates. `tiers` = column counts per tier from the mining report's embedded dataset manifest. Returns `AnalyzerOutput::new(VOCABULARY_FILE, &VocabularyReport, true)`.

```rust
pub struct UsageBreakdown { pub referenced: usize, pub counts: BTreeMap<String, usize>, pub fractions: BTreeMap<String, f64> }
pub struct CandidateUsage { pub name: String, pub usage: UsageBreakdown }
pub struct VocabularyReport { pub schema_version: u32, pub game: String, pub run_id: Option<String>, pub tiers: BTreeMap<String, usize>, pub candidates: Vec<CandidateUsage>, pub overall: UsageBreakdown }
```

- `render` → `## Vocabulary`: `| candidate | referenced | tier-1 | tier-2 | tier-3 |` — one row per candidate plus a final `overall` row, fractions {:.3}. With tier-2 withheld the DoD reads `0.000` in the tier-2 column and `> 0` in tier-3.
- `builtin_registry` registers both new analyzers; sorted names become `agreement, concepts, dataset, mine, summary, vocabulary` (`analyze --list-analyzers` prints six tab lines — `tests/cli_agreement.rs::list_analyzers_lists_both_builtins` counts per-name lines and survives; strengthen `tests/cli_analyze.rs::lists_registered_analyzers` to six).

### D12. `mine` consumes promoted concepts

- `MineAnalyzer::run` becomes: `options.mine.validate()?; options.induction.validate()?;` compute `spec = induction_spec(&options.induction)`. If `options.induction.enabled`: read `ctx.corpus_dir/concepts.json` (missing → `CorpusError::Precondition("concepts.json not found; run the \`concepts\` analyzer before \`mine\`")`), `derived = report.promoted.iter().map(|p| p.def.clone())`. Else `derived = []`. Then `dataset = dataset_from_context(ctx, spec, &derived)?`; `featurizer = ctx.bundle.featurizer_with(spec)?` + `push_derived` each def (so `closure` can pull concept definitions into emitted heuristics); `Miner` unchanged. With induction disabled every byte of M2 behavior is preserved.
- The augmented manifest inside `MiningReport.dataset` then shows tier-3 columns (`tiers` = e.g. `["primitive", "invented"]`) — this is what D14 provenance and D11 `tiers` read.

### D13. CLI — `analyze` flags, `discover` wiring and stdout

- `src/cli/mod.rs`: refactor `Command::Analyze { .. }` to `Analyze(analyze::AnalyzeArgs)` (clap derive `Args`), preserving EVERY existing flag name, value shape, default, and error/exit behavior byte-for-byte. New flags (all optional; merged over `InductionParams::default()` by a `induction_params(..)` sibling of `mine_params`, validated before anything runs): `--induce` (SetTrue → `enabled`), `--withhold-tier2` (SetTrue), `--induction-rounds N`, `--induction-max-thresholds N`, `--induction-beam N`, `--induction-top-k N`, `--induction-max-promoted N`, `--induction-min-train-gain F`, `--induction-min-holdout-gain F`, `--induction-holdout F`, `--induction-probe-depth N`, `--induction-soundness-tolerance F`, `--induction-min-soundness-gain F`, `--induction-compare-atoms BOOL`, `--induction-extended-tier1 BOOL`, `--induction-seed S`, `--label-map FILE` (path used as given). Standalone `analyze` reads induction parameters ONLY from flags (M2 `--mine-*` precedent) — stage-independence commands must pass flags matching the config's `[mine]` and `[induction]` tables.
- `src/cli/discover.rs`: gains `--induce` and `--withhold-tier2` switches; they force the loaded config's `[induction] enabled`/`withhold_tier2` to `true` AFTER `config_hash` is computed (the hash stays "config as loaded" — the M2 contract; effective induction params are captured instead by `params_hash` in D10/D14 provenance; ADR 0018 records this). Forced-analyzer logic: `required = if induction.enabled { ["dataset", "concepts", "mine", "vocabulary"] } else { ["dataset", "mine"] }`; append missing in that order; then validate relative order of whichever required names are present (`dataset < concepts < mine < vocabulary` by index) → violation is `CorpusError::Config` (exit 2).
- Stdout (only when induction is enabled; no-induction stdout is byte-frozen): after the four stage lines and before the `strategy=` lines, `discover` reads `out/concepts.json` and prints exactly one line: `concepts={promoted} evaluated={scored} rounds={rounds} withhold_tier2={bool}` where `promoted = report.promoted.len()`, `scored = Σ rounds[i].scored`, `rounds = rounds.len()`, `withhold_tier2 = params.withhold_tier2`. Then `print_result` and the final `discover=` line, unchanged. Strict check stays last.

### D14. Provenance (`ConceptsProvenance`; ADR 0018 — same justification pattern as M2's D9)

- `DiscoveryProvenance` gains `#[serde(default, skip_serializing_if = "Option::is_none")] pub concepts: Option<ConceptsProvenance>` where `ConceptsProvenance { pub params_hash: String, pub withhold_tier2: bool, pub rounds_run: usize, pub promoted: usize, pub concepts: Vec<String> /* promoted names in order */, pub seed: u64 }`. Populated by `run_discover` from `out/concepts.json` when the file exists AND induction was enabled; `None` otherwise — every previously written document is byte-unchanged (M1 pinned SHA-256s and the M2 pinned run prove it). `SCHEMA_VERSION` stays 1.

### D15. Fixtures

- `configs/tictactoe-concepts.toml` (the DoD fixture; phase 4 owns final values): `schema_version = 1`, `name = "tictactoe-concepts"`, `game = "tictactoe"`, `out = "../runs/tictactoe-concepts"`; `[generate.sweep]` BYTE-IDENTICAL to `configs/tictactoe-discover.toml`'s (seed 20260823 etc. — the corpus then reproduces run_id `bd760ff0ace48705` and its known-good coverage/rows, isolating the M3 delta to analyze-onward); `[annotate] enabled = true`; `[analyze] analyzers = ["summary", "agreement", "dataset", "concepts", "mine", "vocabulary"]`; `[mine] engine = "cart"`, `depths = [6, 8, 12, 0]`, `min_leaf = 1`; `[induction] enabled = true`, `withhold_tier2 = true`, `rounds = 2` (other keys defaulted); `[discover] games = 20`, `seed = 0`; `[report] out = "report.md"`. Run dir = 18 files (M2's 16 + `concepts.json` + `vocabulary.json`).
- `tests/fixtures/concepts-small.toml` (budget fixture; tests override `--out`): `name = "concepts-small"`, `[generate] sweep = "generate-small.toml"`; `[analyze] analyzers = ["summary", "agreement", "dataset", "concepts", "mine", "vocabulary"]`; `[mine] depths = [6]`; `[induction] enabled = true`, `withhold_tier2 = true`, `rounds = 1`, `beam = 24`, `top_k = 4`, `max_promoted = 2`, `min_train_gain = 0.001`, `min_holdout_gain = 0.0005`, `holdout_fraction = 0.25`; `[discover] games = 2`, `roster = "roster-tiny.toml"`. The owning phase may retune `[induction]` values until the small run promotes ≥ 1 concept (record final values in the step report); if no honest tuning achieves it on the generate-small corpus, escalate via revision, never fake it. Run dir = 18 files (discover-small's 15 + `agreement.json` + `concepts.json` + `vocabulary.json` — same file set as the full-scale fixture).
- `tests/fixtures/concept-labels.toml`: top-level TOML table, e.g. `concept_1 = "first invented concept"` (mechanism demo only; label text arbitrary).
- Runtime-written configs (for the flag-equivalence test) follow the M2 precedent: absolute sweep/roster paths with forward slashes via `CARGO_MANIFEST_DIR` + `.replace('\\', "/")`; fixture files on disk are never byte-compared (autocrlf).

### D16. Tests

- Unit (`src/core/derived.rs`, `src/core/featurizer.rs`): D2 ring-game extended families; `with_spec` matrix (supplied × extended), `push_derived` order/duplicate/unknown behavior; `featurizer(true)` unchanged at 70 for tic-tac-toe (existing corpus.rs test).
- Unit (`src/discovery/induce.rs`, over SYNTHETIC in-memory `Dataset` literals — `DatasetRow` fields are pub; no game): enumeration order and counts on a tiny hand-built dataset (deterministic sequence, thresholds, beam cut); dedupe drops constants, duplicate-of-bool-column, duplicate-of-earlier; gain computation against hand-computed IG values; `rejects_candidate_without_holdout_gain` (a candidate predictive on train, shuffled on holdout → absent from shortlist/promotions; assert via RoundReport counts + promoted); `rejects_candidate_without_downstream_improvement` (promotion predicate as a pure function: `after.rules == before.rules` and soundness flat → rejected; also the referenced-name guard); promotion accepts a genuinely compressing candidate; run-twice → identical `ConceptReport` JSON.
- Unit (`src/discovery/vocabulary.rs`): fractions from a hand-built `MiningReport` (all three tier keys present; `0.000` for absent tiers).
- Integration `tests/dataset.rs` (+1): dataset built with a pushed derived def gains a `bool` column with correct values; classes unchanged.
- Integration `tests/interpreter_tactics.rs` (+1): D3 extended-native heuristic plays (registry + match engine, no `UnavailableFeatures`).
- Integration `tests/cli_concepts.rs` (new binary; every path under `CARGO_TARGET_TMPDIR`): (a) `discover --config tests/fixtures/concepts-small.toml --out A` exits 0; stdout has the four stage lines + `concepts=` line + `strategy=mined-d6-l1` + `evaluation=` + `discover=`; `A` holds all 18 files; `concepts.json` parses, `params` echoes the fixture, `withheld` lists 22 `ttt.` names, ≥ 1 promoted with `definition` containing ` [tier-3] = ` and `provenance.params_hash` 16 hex chars and `similarity` non-empty; `heuristics.json`'s candidate validates (`HeuristicStrategy::validate`) and `references()` intersects the promoted names; `dataset.json` `tiers == ["primitive"]`, no column name starts with `ttt.`, columns == 82; `vocabulary.json` overall `fractions["supplied"] == 0.0` and `fractions["invented"] > 0.0`; `report.md` contains `## Concepts`, `### Promoted`, a `` `concept_ `` backticked definition, `## Vocabulary`, and the mined candidate's `#### Feature definitions` block contains `concept_`. (b) second run into `B` → all 18 files byte-identical to `A`, stdout identical modulo `out=`. (c) flag equivalence: runtime-written config pair (identical but for `[induction] enabled/withhold_tier2` present vs absent); run the flagless-config one with `--induce --withhold-tier2`; `concepts.json`, `heuristics.json`, `vocabulary.json`, `dataset.json` byte-identical across the two runs (`discover.json`/`archive/entries.jsonl` differ by `config_hash` — not compared). (d) on a copy `C` of `A`: `analyze --corpus C --analyzers dataset,concepts,mine,vocabulary --mine-depths 6 --induce --withhold-tier2 --induction-rounds 1 --induction-beam 24 --induction-top-k 4 --induction-max-promoted 2 --induction-min-train-gain 0.001 --induction-min-holdout-gain 0.0005 --induction-holdout 0.25` (flags = the fixture's `[induction]` table plus `--mine-*` flags matching its `[mine]` table; substitute retuned values) leaves those four outputs byte-identical; `report --input C` stdout == `C/report.md`. (e) label map: `analyze` over a copy with `--label-map tests/fixtures/concept-labels.toml` (path made absolute) → re-rendered report contains `(label: `. (f) errors → exit 2: `--analyzers concepts` without `--induce`; a config with `withhold_tier2 = true, enabled = false`; a discover config listing `mine` before `concepts`.
- `tests/induction_bench.rs` (new; non-failing `[bench]` lines, generate-small corpus): enumeration/scoring wall ms, candidates enumerated/deduplicated/scored/passed, probe fit ms, promoted count, per-round line.
- Budget: `cli_concepts` ≈ 10–14 s wall (parallel with other binaries), everything else seconds. Workspace total target ≤ 90 s; phase budgets in the roadmap. 28 `test result:` lines after the two new binaries.

### D17. Documentation and evidence

- ADRs (Status `Accepted — 2026-08-2X (Milestone 3 / plan Phase 9)`): `docs/adr/0017-mechanical-tier1-extension.md` (the two families, realizability rules, why they are mechanical not hand-coded, play-vs-dataset extension split, the 47→81 tic-tac-toe consequence); `docs/adr/0018-concept-induction.md` (grammar v1 + omitted ops rationale, scoring/filters, dedupe, probe promotion predicate + cart-tie argument, rounds/stacking, naming/labels, similarity, analyzer chain + file handoffs, `ConceptsProvenance` + params_hash + config-hash-as-loaded stance, SCHEMA_VERSION stays 1).
- `README.md`: `### analyze` gains the two analyzers and the `--induce`/`--withhold-tier2`/`--induction-*`/`--label-map` flags; `### discover` gains an `[induction]` config table (every key with its REAL serde default — diff against live code, M2 30-readme lesson), the two switches, the `concepts=` stdout line description, and run-dir additions; `### Run directory` gains rows for `concepts.json` and `vocabulary.json`; a `#### Readability example` (inside `### discover`) quoting ≥ 1 real promoted definition + its labeled rendering VERBATIM from the pinned full-scale run — written in the FINAL phase only, so it can never go stale against tuning (M2 README-staleness lesson). The existing `### discover` fenced stdout block is untouched. `CLAUDE.md`: the `src/discovery/` architecture bullet gains "concept induction"; nothing else.
- `artifacts/reproducibility/concept-induction-determinism.md`: standard header (debug stated), `## Scope`, `## Commands` (the actually-run script verbatim; scripts must not contain the literal verdict strings — build them by concatenation, M2 lesson), `## Runs` SHA-256 table for run-1/run-2 over all 18 files, verdict lines exactly: `run-2 vs run-1 files (18/18 byte-identical), identical: YES` and `run-2 vs run-1 stdout modulo out=, identical: YES`; 0 `identical: NO`. NO serial variant (the M2 config_hash lesson; the milestone DoD demands repeat-run identity only).
- `artifacts/benchmarks/concept-induction.{md,json}` (release): per-round enumerated/deduplicated/scored/passed/shortlisted/promoted, induction wall, probe fit ms, full `discover --config configs/tictactoe-concepts.toml` wall (median of 3), per-candidate loss/agreement/rules, given-vs-discovered overall fractions. `.md` tables re-rendered FROM the `.json` (single source of truth), floats {:.3}.
- Project gate report `implementation-artifacts/strategy-discovery-m3-gate-report.md`: one `### Item N` per DoD item, verbatim outputs, `Overall verdict: PASS`.

## Constraints

- Quality gates on every merged step: `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` warning-free; every `pub` item documented; `rustfmt.toml` unchanged.
- Root `Cargo.toml` and `Cargo.lock` UNCHANGED (`git diff c997c35 -- Cargo.toml Cargo.lock` empty). Never `polars`, `smartcore`, `arrow`, `parquet`, `criterion`, `bincode`, `sha2`, regex. `src/` never depends on `spikes/`.
- Frozen: `SCHEMA_VERSION = 1`; every existing record/document shape (sole exception: D14's optional `DiscoveryProvenance.concepts`); existing file-name consts; `config_hash`; seed formulas; the CLI contract of existing subcommands (new flags are additive; existing stdout byte-identical for existing configs); `pipeline` stage order; report section order for existing analyzers on existing configs; the pinned run_ids, the M1 pinned SHA-256s, and the M2 pinned full-scale stdout/README block. `MineParams` shape unchanged.
- Never modify: `docs/plan.md`, `.github/**`, existing `artifacts/**` files, `docs/component-evaluation.md`, ADRs 0001–0016, `configs/tictactoe-default.toml`, `configs/tictactoe-experiment.toml`, `configs/tictactoe-discover.toml`, `tests/fixtures/generate-*.toml`, `tests/fixtures/experiment-small.toml`, `tests/fixtures/discover-small.toml`, `tests/fixtures/roster-tiny.toml`, `tests/fixtures/strategies-*.toml`, `tests/invariants.rs`, `docs/adding-a-game.md`, the README `### discover` fenced stdout block.
- Determinism: every persisted output and stdout line byte-identical across runs for the same inputs — no timestamps, durations, absolute paths (except `out=` echoes), `HashMap` order; maps `BTreeMap`; JSONL compact + `\n`; `.json` pretty + trailing `\n`; RNG = `ChaCha8Rng` via documented seeds only; induction, probe, and every determinism-evidence path use `cart` exclusively; f64 scoring uses plain IEEE ops (log2), rendered {:.3}.
- Game-name rule: `src/cli/games.rs` stays the only non-test `.rs` outside `src/games/` naming a concrete game (awk check per M2). No game concept (`threat|fork|corner|center|edge`) in `src/core/`, `src/discovery/`, `src/io/`, `src/cli/` outside `#[cfg(test)]`; the D2 family names (`lines.*`, `cells.*`) and induction code are structural by construction. ONE pre-existing non-test hit exists at baseline `c997c35` — the `NamedOpening` rustdoc example in `src/discovery/config.rs` (see Known coupling and stale sites); phase 4 rewords that one doc line (docs-only), after which the sweep reports 0 with no exceptions and no comment-stripping.
- Nothing on the discovery path depends on exhaustive enumeration; the DoD run is corpus-mode.
- Markdown (acceptance greps): blank line after every heading, blank lines around lists/tables, `| --- |` never `|---|`, every fence tagged, American spelling, floats `{:.3}`. Heading checker and `grep -c -- '|---'` per M2.
- Tests: integration tests write only under `CARGO_TARGET_TMPDIR`; CLI e2e via `assert_cmd` or `CARGO_BIN_EXE_strategy-discovery`; per-binary pass counts derived from the live branch at decomposition time (`grep -c '#\[test\]'`), never assumed; any step changing analyzer/strategy behavior runs a FULL `cargo test --lib` in its worktree (M2 lesson). Debug `cargo test --workspace` wall by phase end: Phase 1 ≤ 60 s, Phase 2 ≤ 75 s, Phase 3 ≤ 85 s, Phase 4 ≤ 90 s.
- Environment: Windows 11, Git Bash + pwsh, `core.autocrlf = true` (never byte-compare checked-out files; byte comparisons only between files a run itself wrote); no `python`; JSON checks via `pwsh -NoProfile -Command "Get-Content -Raw FILE | ConvertFrom-Json"` (beware `0 -eq ''` coercion on zero-valued fields — M2 lesson); hashes via `Get-FileHash -Algorithm SHA256`/`sha256sum` (lowercase); ONE cargo command per worker tool call; Bash outputs > ~8 KB truncate (redirect long output to files; author files with Write/Edit); worktrees under `C:/Users/John/Projects/worktrees/`; scratch under gitignored `target/m3/`; full-scale debug `discover` needs a 600000 ms timeout; `cargo test` stderr carries INFO/TRACE lines — never grep raw stderr for arbitrary words.
- Commit style: plain descriptive messages; one commit per step including its report `implementation-artifacts/strategy-discovery-m3-<id>-report.md`.

## Assumptions

- A cart tree over the withheld+extended tier-1 dataset (depths including 0, min_leaf 1, the pinned corpus) yields ≥ 1 loss-0 candidate vs `perfect`, and at least one promoted concept survives into ≥ 1 loss-0 candidate's rules after tuning `[induction]`/`[mine]` knobs (rounds, beam, top_k, max_promoted, gains, probe_depth, depths). Phase 4 tunes `configs/tictactoe-concepts.toml` until the conjunction (loss 0.000 AND tier-3 reference) holds for at least one candidate and records the tuning; if no honest tuning achieves it, the supervisor sends an amendment note rather than weakening the DoD.
- The generate-small corpus (190 rows) supports ≥ 1 promotion under the concepts-small fixture's low thresholds; if not, the fixture is retuned (still honest) or the e2e promotion assertion escalates via revision.
- Induction cost at full scale (≈ 557 rows × ≈ 82 columns, ≤ ~5,000 candidates/round, ≤ 16 probe fits) is seconds in release and well under the debug discover timeout.
- `generate` into an existing run directory overwrites its files (M2-verified), so repeat-run comparisons may reuse fresh sibling dirs.

## Out of scope

- Any `FeatureExpr` algebra change; enumerating `Arith`/`SetOp`/`Contains` (v2, ADR-documented); set- or numeric-valued promoted concepts; per-candidate full-harness scoring during promotion (the probe is rule-count/soundness only; the harness scores final candidates as in M2); pruning/merging of mined rule lists; a `linfa`-based induction path; new games; `evaluate`/`pipeline` behavior changes; schema version bump; serial-vs-parallel evidence for induction (repeat-run only); editing frozen files; upstream `game-player` changes; progress bars, JSON logs.

## Definition of Done (project)

All run from the repo root on the integrated milestone branch, debug unless stated.

1. `cargo check --workspace --all-targets`, `cargo test --workspace` (0 failed, 28 `test result:` lines), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` (0 warnings) all exit 0; warm `cargo test --workspace` wall ≤ 90 s (`Measure-Command`); `git diff c997c35 -- Cargo.toml Cargo.lock` empty.
2. `cargo run -- discover --config configs/tictactoe-concepts.toml --out target/m3/concepts-1` exits 0 and prints, in order: the four stage lines, one `concepts=` line with `concepts >= 1` and `withhold_tier2=true`, ≥ 1 `strategy=mined-` line with `kind=heuristic-rules`, one `evaluation=` line, one `discover=` line; the run dir holds exactly 18 files including `concepts.json` and `vocabulary.json`.
3. `target/m3/concepts-1/concepts.json`: ≥ 1 entry in `promoted`, each with `def.tier == "invented"`, a `definition` string containing ` [tier-3] = `, `kind == "bool"`, `scores` populated, and `provenance` = `{ run_id == the run's run_id, params_hash: 16 lowercase hex chars, seed, round >= 1 }`; `withheld` lists the 22 `ttt.` names; every promoted concept has a non-empty `similarity` list whose entries name `ttt.` features with agreement in [0, 1] (threat/fork included among targets; informational, no threshold).
4. In `target/m3/concepts-1/archive/entries.jsonl` at least one entry satisfies ALL of: `evaluation.headline.loss_rate_vs_reference == 0.0` with `losses == 0` on both seats against `perfect` (`games_per_pairing == 20`, roster `ttt-benchmark-v1`); the same-named candidate in `heuristics.json` passes `HeuristicStrategy::validate()` and its `references()` contains ≥ 1 promoted `concept_` name; `provenance.discovery.concepts` is populated (`params_hash`, `withhold_tier2 == true`, `rounds_run >= 1`, `promoted >= 1`, `concepts` non-empty, `seed`); `provenance.discovery.tiers == ["primitive", "invented"]`.
5. Self-containment: in `target/m3/concepts-1/report.md`, that candidate's `#### Feature definitions` block contains ≥ 1 `` `concept_ `` definition line (the definitions of every referenced tier-3 concept travel with the heuristic).
6. `target/m3/concepts-1/vocabulary.json`: `overall.fractions["supplied"] == 0.0` and `overall.fractions["invented"] > 0.0`; per-candidate rows present; `report.md` contains `## Vocabulary`. `target/m3/concepts-1/dataset.json`: `tiers == ["primitive"]`, `columns` == 82, no column named `ttt.*`.
7. `cargo run -- discover --config configs/tictactoe-concepts.toml --out target/m3/concepts-2` produces a directory whose all 18 files are SHA-256-identical to `target/m3/concepts-1` and identical stdout modulo `out=`; recorded in `artifacts/reproducibility/concept-induction-determinism.md` (standard header, both D17 verdict lines `identical: YES`, 0 `identical: NO`).
8. Promotion-criteria tests: `cargo test rejects_candidate` runs ≥ 2 tests including names containing `rejects_candidate_without_holdout_gain` and `rejects_candidate_without_downstream_improvement`, all passing.
9. Stage independence: `cargo test --test cli_concepts` passes including the D16 (d) standalone-analyze byte-identity test and the (c) flag-equivalence test; `cargo run -- analyze --list-analyzers` prints six lines (`agreement`, `concepts`, `dataset`, `mine`, `summary`, `vocabulary`).
10. Regression: the three M1 pinned SHA-256s reproduce (`1487b18b…`, `6834c0e3…`, `8f5a1bd2…`); `cargo test --test cli_pipeline --test corpus --test cli_experiment --test cli_discover --test dataset --test mine` all pass; fresh `cargo run -- discover --config configs/tictactoe-discover.toml --out target/m2/p3-discover` stdout diffs EMPTY against the README `### discover` fenced block; `git diff c997c35 --stat` over every path in the Never-modify list prints nothing.
11. Docs: `docs/adr/0017-*.md` and `0018-*.md` exist with the four sections and Milestone 3 status lines; README has the `[induction]` table, the new flags, run-dir rows for `concepts.json`/`vocabulary.json`, and the `#### Readability example` quoting a real promoted definition; `CLAUDE.md` discovery bullet mentions concept induction; game-name awk loop and the game-concept grep sweep over `src/core src/discovery src/io src/cli` report 0 violations outside `#[cfg(test)]`.
12. `artifacts/benchmarks/concept-induction.md` + `.json` exist (standard release header, `.md` agrees with `.json` cell for cell); `tests/induction_bench.rs` runs non-failing in debug printing `[bench]` lines.
13. Every step's worker report committed with its step; ledger complete; gate report `implementation-artifacts/strategy-discovery-m3-gate-report.md` ends `Overall verdict: PASS`.
