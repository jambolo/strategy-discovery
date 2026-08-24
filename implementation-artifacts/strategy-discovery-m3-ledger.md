# strategy-discovery-m3 — Ledger

Single source of truth for execution state. Sections are owned by different agents —
the planner seeds Plan + Phases; the decomposer fills Steps per phase; the supervisor
updates Steps and appends Revisions.

## Plan

- plan-name: strategy-discovery-m3
- current-phase: complete (4 of 4 phases done)
- working-branch: milestone/3-concept-induction
- starting-commit: c997c351f636bc80b7b9bde46b7aee7a309f5f20
- default-branch: develop
- artifacts-dir: implementation-artifacts/

## Phases

| Phase | Status  | Notes |
| ----: | ------- | ----- |
| 1     | complete | Feature-platform groundwork (brief D2-D5, D14 types; budget ≤ 60 s). DoD verified: 26 test-result lines / 479 tests, 0 failed; warm wall 48.67 s; gate report `Overall verdict: PASS`; three M1 SHA-256s reproduced; Cargo.toml/lock diff empty vs c997c35 |
| 2     | complete | Induction engine, analyzers, CLI flags (brief D6-D12, D13 analyze; budget ≤ 75 s). DoD verified: 27 test-result lines / 390 lib + 7 cli_analyze + 1 induction_bench, 0 failed; warm wall 49.20 s vs 75 s budget; gate report `Overall verdict: PASS` (15/15); chain promotes concept_1 on generate-small, four analyze outputs repeat-run byte-identical; Cargo.toml/lock + tests/cli_agreement.rs diff empty vs c997c35. One revision: 24-concepts-analyzer render heading blank lines |
| 3     | complete | discover wiring, fixtures, e2e, docs (brief D13-D16, D17 ADRs/README; budget ≤ 85 s). DoD verified by supervisor re-run: check/fmt/clippy/doc clean (0 doc warnings); warm `cargo test --workspace` 51.24 s vs 85 s budget, 28 `test result: ok` lines / 28 ` 0 failed`; cli_concepts 8 passed (full D16 a-f), cli_discover 4 passed and `git diff c997c35 -- tests/cli_discover.rs` empty; `discover --config tests/fixtures/discover-small.toml` stdout has 0 `concepts=` lines; concepts-small run = exit 0, 18 files, concept_1 promoted; ADRs 0017/0018 titles + 4 sections + Milestone-3 status + 0 heading/`\|---` violations; README `[induction]` block matches every serde default in src/io/schema.rs, frozen M2 discover block intact, six-analyzer list regenerated, two run-dir rows; CLAUDE.md bullet updated; game-name/game-concept greps 0 on both changed src files; Cargo.toml/lock diff empty vs c997c35; gate report `Overall verdict: PASS` (13/13). No revisions |
| 4     | complete | Full-scale tuning, evidence, gate (brief D17 evidence, project DoD; budget ≤ 90 s). Steps `41-verify` `24b088b`, `42-config` `cfb525a`, `43-determinism` `feca92e`, `44-benchmark` `9f2a6d4`, `45-readme` `bfd801c`, `46-gate` `2385b49` (46 ran in-tree as a lone ready step, so it has no merge commit). NO TUNING: `configs/tictactoe-concepts.toml` untouched — blob `f502744` identical to its phase-3 creation, `git diff 0d13aed..HEAD -- configs/tictactoe-concepts.toml` empty. PHASE DoD VERIFIED BY THE SUPERVISOR ON THE INTEGRATED TREE at `2385b49`, independently of the gate report: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` all exit 0 with 0 doc warnings; timed `Measure-Command { cargo test --workspace }` wall **51.23 s** vs the ≤ 90 s budget (gate measured the same), 28 `test result: ok` lines and 28 matching the zero-failure pattern; `git diff c997c35 -- Cargo.toml Cargo.lock` 0 bytes. Supervisor's own run of `target/m3/gate-runs.ps1` (byte-diffed against the step spec first): 9/9 `CHECK … PASS` incl. `stdout-pinned` (11 lines), `files-18`, `concepts-json` (promoted 2, withheld 22, rows 557, params_hash `f1596db3a853769f`), `conjunction=PASS (mined-d12-l1, mined-d0-l1)` with `d0-refs=concept_2` — the milestone conjunction verified PER ARCHIVE ENTRY — `dataset-vocabulary` (82 columns, invented fraction 0.0196078431372549, supplied 0.0), `report-md`, all four pinned SHA-256s (`concepts.json 541c5ea5…`, `heuristics.json c52ba9bf…`, `evaluation.json 0e9c3423…`, `report.md e78d8a15…`), and `repeat-identical` 18/18 + stdout modulo `out=`. Determinism artifact re-checked: 18 hash rows, 2 `identical: YES`, 0 `identical: NO`, and its `evaluation.json`/`heuristics.json`/`report.md` hashes match the supervisor's own independent run. Item 10 re-run live: all three M1 SHA-256 pins reproduce (`1487b18b…`, `6834c0e3…`, `8f5a1bd2…`), M2 frozen `discover` stdout diffs EMPTY against the README block, never-modify sweep prints nothing. Item 8 `cargo test rejects_candidate` 2 passed (both named tests); item 9 `--list-analyzers` exactly 6 lines `agreement,concepts,dataset,mine,summary,vocabulary`; item 11 game-name and game-concept sweeps both 0 violations (42-config's reword is what makes the concept sweep honestly 0); item 12 `induction_bench` 5 `[bench]` lines / 1 passed and the benchmark `.json` passes the full field-by-field pwsh assertion (`json-ok`), `.md` agreeing cell-for-cell (discover median 1.762 s, chain median 0.260 s). Gate report `46-gate`: 13/13 items PASS, `Overall verdict: PASS`. |

## Steps

<!-- decomposer fills per phase: id | phase | status | files | commit,
     plus a "Phase <N> notes" block: dependency graph, couplings, emergent contracts -->

| id | phase | status | files | commit |
| --- | --- | --- | --- | --- |
| 11-schema | 1 | done | src/io/schema.rs; src/io/mod.rs; src/discovery/discover.rs; implementation-artifacts/strategy-discovery-m3-11-schema-report.md | b69eca35eac7066858c63e0a07367bce15097715 |
| 12-derived | 1 | done | src/core/derived.rs; implementation-artifacts/strategy-discovery-m3-12-derived-report.md | 2e514a89a08404ff0fbe83605bdd5255200b1a45 |
| 13-featurizer | 1 | done | src/core/featurizer.rs; implementation-artifacts/strategy-discovery-m3-13-featurizer-report.md | ace0433524a09c2c686027d6b3942fa41fdd0b59 |
| 14-play | 1 | done | src/discovery/bundle.rs; src/games/tictactoe/mod.rs; src/games/tictactoe/corpus.rs; implementation-artifacts/strategy-discovery-m3-14-play-report.md | 550b5932abf754530e6b2f0ef80aa67629725333 |
| 15-options | 1 | done | src/discovery/analyze.rs; src/cli/analyze.rs; src/discovery/summary.rs; src/discovery/experiment.rs; implementation-artifacts/strategy-discovery-m3-15-options-report.md | ff277847b3164a178cb15c69512ae878c4fd0c37 |
| 16-dataset | 1 | done | src/discovery/dataset.rs; src/discovery/mine.rs; implementation-artifacts/strategy-discovery-m3-16-dataset-report.md | 349c372d7a29d9442462eb694e8aef6a051ddbc6 |
| 17-play-test | 1 | done | tests/interpreter_tactics.rs; implementation-artifacts/strategy-discovery-m3-17-play-test-report.md | e2679fc68eba2ee6a19d7a9190a265a5969a0483 |
| 18-dataset-test | 1 | done | tests/dataset.rs; implementation-artifacts/strategy-discovery-m3-18-dataset-test-report.md | 7ddaf0818349ad0e5a2adba31539c71caa5c5d1d |
| 19-gate | 1 | done | implementation-artifacts/strategy-discovery-m3-19-gate-report.md | 80bad310ce702e567a44a9f813684c0819aaa44a |
| 21-mine | 2 | done | src/discovery/mine.rs; implementation-artifacts/strategy-discovery-m3-21-mine-report.md | 1a04e2d4bc121aeb2ebfd7b9d5c58d9f059af273 |
| 22-vocabulary | 2 | done | src/discovery/vocabulary.rs; src/discovery/mod.rs; implementation-artifacts/strategy-discovery-m3-22-vocabulary-report.md | b447b752b4977c90d726c1140d07e148a4d1538d |
| 23-induce-core | 2 | done | src/discovery/induce.rs; src/discovery/mod.rs; src/io/schema.rs; implementation-artifacts/strategy-discovery-m3-23-induce-core-report.md | 7386c940bbeac73f00368fccb3f6776e35c1f9ea |
| 24-concepts-analyzer | 2 | done | src/discovery/induce.rs; src/discovery/mod.rs; implementation-artifacts/strategy-discovery-m3-24-concepts-analyzer-report.md | 3d4705ddcf551582b7fb233aa598e01021133416 |
| 25-registry | 2 | done | src/discovery/analyze.rs; tests/cli_analyze.rs; implementation-artifacts/strategy-discovery-m3-25-registry-report.md | ca63c29226ddcf7330c0edc0d34c6743f77085ff |
| 26-cli | 2 | done | src/cli/mod.rs; src/cli/analyze.rs; implementation-artifacts/strategy-discovery-m3-26-cli-report.md | 233372b9b1df2b67232a09a53b0db47869b409cc |
| 27-tests | 2 | done | tests/cli_analyze.rs; tests/induction_bench.rs; implementation-artifacts/strategy-discovery-m3-27-tests-report.md | b92be6ebb108f1888cc3e0e364acb16360917b8b |
| 28-chain | 2 | done | implementation-artifacts/strategy-discovery-m3-28-chain-report.md | fb3f5bdb539afbd024888ca672a31ffa1ec9b8d1 |
| 29-gate | 2 | done | implementation-artifacts/strategy-discovery-m3-29-gate-report.md | 60ea55c925cbce84255b6dd3d74475b9fd4520f9 |
| 31-discover | 3 | done | src/cli/discover.rs; src/discovery/discover.rs; implementation-artifacts/strategy-discovery-m3-31-discover-report.md | 20e338cbe1d5925e2e6ef86ca2b4ee1e3b9ef362 |
| 32-fixtures | 3 | done | configs/tictactoe-concepts.toml; tests/fixtures/concepts-small.toml; tests/fixtures/concept-labels.toml; implementation-artifacts/strategy-discovery-m3-32-fixtures-report.md | 0d13aedc622c1baf1085f40cbd70d4619c6004b6 |
| 33-adr | 3 | done | docs/adr/0017-mechanical-tier1-extension.md; docs/adr/0018-concept-induction.md; implementation-artifacts/strategy-discovery-m3-33-adr-report.md | db33c6e4e5e00aa54b76476f44f5bcb1b8353f2d |
| 34-readme | 3 | done | README.md; CLAUDE.md; implementation-artifacts/strategy-discovery-m3-34-readme-report.md | 726fbdbbf08dcc867ac245217290d983a015a593 |
| 35-e2e-core | 3 | done | tests/cli_concepts.rs; implementation-artifacts/strategy-discovery-m3-35-e2e-core-report.md | cebd6cbf94fbd881de5865ae8ef44dd331aaa4af |
| 36-e2e-flags | 3 | done | tests/cli_concepts.rs; implementation-artifacts/strategy-discovery-m3-36-e2e-flags-report.md | b9bff56325c663fdf1b5b58d722a47f5a79b3077 |
| 37-gate | 3 | done | implementation-artifacts/strategy-discovery-m3-37-gate-report.md | b0b7ece66233265eadcc75476fa1165a4c785326 |
| 41-verify | 4 | done | implementation-artifacts/strategy-discovery-m3-41-verify-report.md | 24b088b149e0bc5848cbd5342164aa5669e9815c |
| 42-config | 4 | done | src/discovery/config.rs; implementation-artifacts/strategy-discovery-m3-42-config-report.md | cfb525aa17619141aefe11e1e69bdf7532839cf8 |
| 43-determinism | 4 | done | artifacts/reproducibility/concept-induction-determinism.md; implementation-artifacts/strategy-discovery-m3-43-determinism-report.md | feca92ec3a52503ffa92b49d30d6241709990ce3 |
| 44-benchmark | 4 | done | artifacts/benchmarks/concept-induction.md; artifacts/benchmarks/concept-induction.json; implementation-artifacts/strategy-discovery-m3-44-benchmark-report.md | 9f2a6d436ee71fa585b844532e6468d9c839b59c |
| 45-readme | 4 | done | README.md; implementation-artifacts/strategy-discovery-m3-45-readme-report.md | bfd801cf6ce37d4a64e27d985e0dfc084ab94f31 |
| 46-gate | 4 | done | implementation-artifacts/strategy-discovery-m3-gate-report.md; implementation-artifacts/strategy-discovery-m3-46-gate-report.md | 2385b493 |

### Phase 1 notes

- Dependency graph (steps at the same level run in parallel; scopes verified disjoint):
  - Level 0: 11-schema, 12-derived
  - Level 1: 13-featurizer (after 12), 15-options (after 11)
  - Level 2: 14-play (after 13; parallel with 15)
  - Level 3: 16-dataset (after 11+14+15), 17-play-test (after 14)
  - Level 4: 18-dataset-test (after 13+16)
  - Gate: 19-gate (after all; report-only, runs the phase DoD incl. warm-wall timing,
    M1 pinned SHA-256 reproduction, and the M2 byte-determinism suite)
- Compile-forced edits outside the roadmap's file list, owned by 11-schema: the D14
  optional field on `DiscoveryProvenance` forces `concepts: None,` at its only two
  struct literals — `src/discovery/discover.rs:92` and the schema.rs test
  `provenance_without_discovery_serializes_unchanged` (the phase's only existing-test
  edit). `src/discovery/agreement.rs` needs NO edit: its three `AnalyzeOptions` test
  literals use `..AnalyzeOptions::default()` spreads (the brief's "12 literal sites"
  count includes the struct decl and `impl Default` header lines).
- config_hash fact (verified at decomposition, do not re-litigate): adding
  `ExperimentConfig.induction` (after `mine`) changes every experiment config's
  `config_hash` vs its M2 value. Nothing pins those values — no test holds a hash
  literal (only `!is_empty()`), stdout never prints one, and `evaluation_id` derives
  from (game, roster, EvaluateConfig, strategies) only — so the pinned M2 stdout/README
  block and repeat-run byte tests are unaffected.
- Emergent contracts for later phases:
  - `induction_spec(params) -> FeaturizerSpec` is `pub(crate)` in
    `src/discovery/dataset.rs` (decomposer's placement choice for the brief's D4
    "shared location"); formula `{ include_supplied: !(enabled && withhold_tier2),
    extended: enabled && extended_tier1 }`. Phase 2's induce.rs imports or re-exports
    it from there.
  - Phase-1 `MineAnalyzer::run` builds `spec = induction_spec(&options.induction)` for
    BOTH the dataset and its own `featurizer_with(spec)`, but does NOT yet read
    concepts.json, push derived defs, or call `options.induction.validate()` — all of
    D12 stays phase 2.
  - `DiscoveryProvenance.concepts: Option<ConceptsProvenance>` is the struct's LAST
    field, `None` everywhere; phase 3's `run_discover` populates it.
  - Gate-filter test-name contracts: `cargo test --lib induction_params` must keep
    matching `induction_params_defaults` + `induction_params_validate_rejects_bad_values`;
    the extended-feature facts are carried by test names containing `extended`
    (derived.rs x2, featurizer.rs matrix test, corpus.rs x2, interpreter_tactics x1).
  - Extended tic-tac-toe vocabulary layout (supplied+extended = 104 defs):
    `side_to_move` 0, base tier-1 1..48, Family A `lines.mine{a}.theirs{b}` 48..58,
    Family B `cells.mine{a}.theirs{b}.ge{k}` 58..82, `ttt.*` 82..104; withheld+extended
    = 82. Ring fixture: base 23, extended 35 tier-1 defs.
- Environment: fresh worktrees build cold (~2-3 min before the first cargo command
  finishes); acceptance test counts in step files are pre-merge counts in the worker's
  own worktree (e.g. interpreter_tactics 6 -> 7 only after 17-play-test merges).

### Phase 2 notes

- Dependency graph (steps at the same level run in parallel; scopes verified disjoint):
  - Level 0: 21-mine, 22-vocabulary, 26-cli
  - Level 1: 23-induce-core (after 21 + 22)
  - Level 2: 24-concepts-analyzer (after 23)
  - Level 3: 25-registry (after 22 + 24)
  - Level 4: 27-tests, 28-chain (both after 25 + 26; parallel, disjoint scopes;
    28 is evidence-only — report is its sole tracked file)
  - Gate: 29-gate (after all; report-only, runs the phase DoD incl. warm-wall timing
    and the analyze-chain repeat-run byte-identity)
- Owner chains (never co-parallel): src/discovery/mod.rs edited by 22 -> 23 -> 24;
  tests/cli_analyze.rs edited by 25 -> 27.
- Green-at-every-merge: the brief requires `cargo test --workspace` clean after each
  merged step, so the 4 -> 6 `--list-analyzers` count strengthen in
  tests/cli_analyze.rs lands INSIDE 25-registry (same commit that grows the registry).
  Verify 25 with its own acceptance (cargo test --lib = 390; cargo test --test
  cli_analyze = 5), not with pre-phase expectations.
- 23-induce-core also owns a one-line doc fix in src/io/schema.rs:
  `RoundReport.deduplicated` counts candidates DROPPED by dedupe (brief D10 semantics;
  the phase-1 worker's doc gloss said "remaining"). enumerated - deduplicated == scored.
  Phase 3's `concepts=` stdout field `evaluated={scored}` sums `scored`, never
  `deduplicated`.
- Emergent contracts for phase 3 (project into its steps; workers never read this):
  - `pub fn induce_concepts(bundle, dataset, params) -> Result<ConceptReport, CorpusError>`
    re-exported from `discovery`; returns `labels` empty — `ConceptsAnalyzer::run`
    fills labels from `options.induction.label_map` (path used as given; the
    experiment resolver already absolutizes configured paths against base_dir).
  - `mine_one` and `split_train_holdout` are pub(crate) in src/discovery/mine.rs;
    induce.rs's probe = `mine_one(dataset, all-rows, [], "cart", probe_depth, 1, f)`.
  - Exact exit-2 error strings: concepts without induction ->
    "analyzer `concepts` requires [induction] enabled = true (or --induce)";
    mine with induction enabled but no concepts.json ->
    "concepts.json not found; run the `concepts` analyzer before `mine`";
    vocabulary without heuristics.json ->
    "heuristics.json not found; run the `mine` analyzer first".
  - `--list-analyzers` order and descriptions: agreement, concepts ("induces and
    promotes tier-3 concepts from the feature dataset"), dataset, mine, summary,
    vocabulary ("reports given-versus-discovered feature usage of mined heuristics").
  - ShortlistEntry semantics: entries record TRIALS only (outcomes
    promoted:concept_N | rejected-downstream | duplicate); the trial loop stops once
    max_promoted concepts promoted (untried candidates unrecorded); the global
    concept_N counter advances on promotion only; a round with zero promotions ends
    the round loop.
  - Dedupe realization: no pre-seeded column vectors — each bool column's Ref atom is
    the first candidate carrying that column's bits, so later duplicates drop
    against it; a bare Ref(concept) can reach a shortlist but can never promote
    (cart first-column tie-break makes the probe reference the original column,
    failing the reference guard).
  - Render pins: `## Concepts` = overview table (rows: rounds, enumerated, scored,
    promoted, withheld) + `### Promoted` bullets `` `{definition}` `` with
    ` (label: {label})` suffix + `### Similarity` top-3 table + `### Rounds`
    8-column table; `## Vocabulary` = single table
    `| candidate | referenced | tier-1 | tier-2 | tier-3 |` plus a final `overall`
    row, fractions {:.3}.
  - CLI: `Command::Analyze(analyze::AnalyzeArgs)` (clap Args) replaced the inline
    variant; induction flags per brief D13; standalone `analyze` reads induction
    params ONLY from flags, validated (with mine params) before `--list-analyzers`.
  - 28-chain's report line `- final-flags: ...` records the induction flag set proven
    to promote >= 1 concept on the generate-small corpus (ladder A1..A4 in the step);
    phase 3 seeds tests/fixtures/concepts-small.toml `[induction]` values from it,
    and 29-gate re-runs the chain with exactly those flags.
- Counts after phase 2 merges: lib tests 390 (378 at phase start, +1 mine,
  +2 vocabulary, +4 induce-core, +5 concepts-analyzer); cli_analyze 7;
  induction_bench 1 (new binary); workspace = 27 `test result:` lines; warm
  `cargo test --workspace` budget <= 75 s.
- Environment: unchanged from Phase 1 notes (cold worktree builds ~2-3 min; step
  acceptance counts are pre-merge counts in the worker's own worktree; one cargo
  command per worker Bash call; redirect >8 KB outputs to files).

### Phase 3 notes

- Decomposed against the AMENDED brief at 2217ed0 (concepts-small has the six-analyzer
  list incl. agreement -> 18-file run dir; D16(d) includes --mine-depths 6).
- Dependency graph (steps at the same level run in parallel; scopes verified disjoint):
  - Level 0: 31-discover, 32-fixtures, 33-adr, 34-readme
  - Level 1: 35-e2e-core (after 31 + 32)
  - Level 2: 36-e2e-flags (after 35; same test file — owner chain 35 -> 36,
    never co-parallel)
  - Gate: 37-gate (after all; report-only, runs the full phase DoD incl. warm-wall
    timing, the no-induction stdout check, and the ADR/README/CLAUDE.md doc checks)
- 32-fixtures self-verifies promotion with the PRE-31 binary (the analyze chain and
  [induction] resolution are phase-2 work; pre-31 `discover` runs the config's six
  analyzers and writes all 18 files — it merely lacks the `concepts=` stdout line,
  order validation, and ConceptsProvenance). Its acceptance therefore asserts exit 0 +
  18 files + promoted >= 1, never stdout.
- 34-readme is level 0 by design: every fact it documents is pinned in the brief or
  already merged (schema.rs defaults, registry descriptions); it regenerates the
  `--list-analyzers` fenced block by running the binary (registry untouched by
  co-parallel steps) and embeds NO output of 31/32 (the readability example with real
  run output is phase 4's).
- Pinned in 31-discover (phase 4 and tests rely on these exactly):
  - stdout line: `concepts={promoted} evaluated={sum of rounds[].scored} rounds={rounds.len()} withhold_tier2={params.withhold_tier2}`,
    printed only when induction enabled, after run_stages, before run_discover.
  - order-violation error string: `[analyze] analyzers: required order is dataset < concepts < mine < vocabulary`
    (CorpusError::Config, exit 2). Forcing appends missing required names at the END,
    so a config pre-listing `mine` without `concepts` fails under --induce by design.
  - `ConceptsProvenance.params_hash = config_hash(&report.params)` (the effective
    InductionParams); populated iff induction enabled AND concepts.json exists.
- Lib tests 390 -> 394 after 31 (+3 cli/discover forcing/order tests,
  +1 discovery/discover concepts_provenance test); cli_concepts 5 after 35, 8 after 36;
  workspace `test result:` lines 27 -> 28 (new cli_concepts binary); warm budget <= 85 s.
- concepts-small [induction] values are seeded from 28-chain's proven final-flags
  (rounds 1, beam 24, top_k 4, max_promoted 2, min_train_gain 0.001,
  min_holdout_gain 0.0005, holdout 0.25); 32 records any retune in its report's
  deviations, and 35/36 read the ON-DISK fixture values rather than assuming these.
- src/cli/pipeline.rs confirmed untouched: analyzer forcing/order validation is
  discover-only (D13); `pipeline` keeps running config analyzers as-is.
- Phase 4 heads-up: the PROJECT gate report is a DIFFERENT file
  (strategy-discovery-m3-gate-report.md) from 37's step report; 37's evidence
  (28 result lines, wall seconds) feeds the phase row here, not the project gate.
- Phase 3 execution outcome (all seven steps passed first try; no revisions, no
  amendments; 31/32/33/34 ran as one parallel wave in worktrees, 35/36/37 in-tree):
  - Counts now on the branch: lib 394, cli_concepts 8, cli_discover 4, workspace 28
    `test result:` lines, warm wall 51.24 s (85 s budget).
  - `tests/fixtures/concepts-small.toml` shipped with the decomposer's seeded
    `[induction]` values UNCHANGED (no retune was needed) — rounds 1, beam 24, top_k 4,
    max_promoted 2, min_train_gain 0.001, min_holdout_gain 0.0005, holdout_fraction 0.25;
    the run promotes exactly one concept (`concept_1`) and writes 18 files.
  - `configs/tictactoe-concepts.toml` is on disk with `[induction] enabled/withhold_tier2
    = true, rounds = 2` and `[mine] depths = [6, 8, 12, 0]` — phase 4 tunes ONLY these two
    tables; its `[generate.sweep]`..`[annotate]` region is byte-identical to
    `configs/tictactoe-discover.toml` (a phase-3 acceptance, keep it true).
  - README's `[induction]` block was verified field-by-field against
    `src/io/schema.rs` defaults at phase close; if phase 4 tuning changes any DEFAULT
    (it should not — tuning edits the fixture, not the defaults), re-diff that block.

### Phase 4 notes

- Decomposed against the AMENDED brief at 66160fb (baseline concept-word doc hit ->
  42-config owns the one-line reword; DoD item 11 sweep text unchanged, full strength).
- Dependency graph (steps at the same level run in parallel; scopes verified disjoint):
  - Level 0: 41-verify (report-only), 42-config (one doc-comment line in
    src/discovery/config.rs)
  - Level 1: 43-determinism, 44-benchmark, 45-readme (all after 41; parallel; disjoint
    artifact/README scopes; each re-runs the fixture itself — no step consumes another's
    target/ output)
  - Gate: 46-gate (after 42+43+44+45). It writes TWO files: the PROJECT gate report
    strategy-discovery-m3-gate-report.md AND its own step report
    strategy-discovery-m3-46-gate-report.md (the phase-3 heads-up about the distinct
    project gate file is realized here).
- THE phase risk is already retired: the decomposer probe-ran the shipped fixture at
  8f6db57/66160fb — release AND debug, byte-identical — and the milestone conjunction
  HOLDS with zero tuning: concepts=2 (concept_1 round 1; concept_2 round 2 — a concept
  stacked on a concept), ALL FOUR candidates directly reference concept_2, and
  mined-d12-l1 + mined-d0-l1 are loss-0 vs perfect on both seats with full
  ConceptsProvenance. NO step's scope includes configs/tictactoe-concepts.toml (last
  touched by phase-3 commit 0d13aed); the gate report's `## Tuning` section records
  "no tuning needed". If any worker's verification honestly fails, that is REAL drift:
  route a revision, never let a worker touch the fixture.
- Canonical pinned values (decomposer-measured, embedded in steps 41/44/46): run_id
  bd760ff0ace48705; stdout `concepts=2 evaluated=7543 rounds=2 withhold_tier2=true`;
  report bytes 170248; evaluation id 1e34b13a8091fef5; rounds r1
  82/4537/757/3780/3749/8/1, r2 83/4538/775/3763/3735/8/1; dataset 557 rows (418/139
  train/holdout), 82 base columns; params_hash f1596db3a853769f; candidate
  rules/leaves/fallback d6 41/41/0, d8 84/85/2, d12 149/151/3, d0 173/175/3; vocabulary
  fractions {:.3} 0.980/0.000/0.020; 18 files; concepts.json SHA-256 541c5ea59ea43739
  f8fc8da969499c5cc3d8193d3f1d694f0150b6ac52bb71ac (split across two lines here only);
  definitions `concept_1 [tier-3] = ((count(orbit0.free) == 1) or (count(free) == 1))`
  and `concept_2 [tier-3] = (concept_1 and (orbit0.mine >= 1))`; orbit0 = corners
  {0,2,6,8}, orbit0.mine is an int column. Debug full-scale wall ~10 s, release ~1.8 s
  (use 600000 ms call timeouts regardless).
- Evidence-step pattern: every embedded script (41 verify-41.ps1, 43 determinism.ps1,
  44 concept-bench.ps1, 46 gate-runs.ps1 + m2-check.ps1) was authored and dry-run
  END-TO-END by the decomposer at 66160fb, all checks PASS, including the M1 SHA
  reproduction and the M2 README-block empty diff. Workers execute and transcribe only.
- Supervisor watchouts: PowerShell variable names are CASE-INSENSITIVE (scripts avoid
  $b-vs-$B collisions — reject any worker "cleanup" of script variable names); the
  pwsh json acceptance line in 44 carries backslash-dollar escapes and MUST be run from
  Git Bash verbatim; `grep -c` exits 1 when it prints 0 — expected-0 acceptances pass
  on output `0` despite the exit code; 45's label texts are pinned ("last open corner",
  "last open corner while holding one") and its README anchor is the single ADR-link
  paragraph line; 41/46 both use target/m3/concepts-1 (each rm -rf's it first — safe
  in any execution order).
- Counts: phase 4 adds NO tests and NO src behavior change (42 is a comment), so
  `cargo test --workspace` stays 28 `test result:` lines, ~51 s warm vs the 90 s
  budget; tests/induction_bench.rs is explicitly NOT edited (no measurement-vehicle
  gap emerged — bench + concepts.json + end-to-end analyze walls cover D17).
- Phase 4 execution outcome (all six steps passed; the decomposer's zero-tuning
  prediction held exactly — no amendment, no decomposer revision):
  - Waves as planned: 41 ∥ 42 in worktrees, then 43 ∥ 44 ∥ 45 in worktrees, then 46
    in-tree (lone ready step). Every wave passed its base gate (worktree HEAD ==
    wave BASE, dependency commits ancestors of BASE).
  - Confirmed counts on the branch at close: 28 `test result: ok` lines, warm wall
    51.23 s vs the 90 s budget, 0 clippy/doc warnings, `git diff c997c35 --
    Cargo.toml Cargo.lock` empty. Project gate `Overall verdict: PASS` (13/13).
  - The milestone conjunction is REAL and reproduced by the supervisor twice, in two
    different trees: `mined-d12-l1` and `mined-d0-l1` are loss-0 vs `perfect` on both
    seats and both directly reference `concept_2`, which is itself stacked on
    `concept_1` — a concept built on a concept.
  - Two in-flight corrections, both recorded in Revisions: a supervisor packet
    mis-transcription in 44 (malformed table delimiters) and a mis-specified tuning
    acceptance in 46 (`git log -1` pinned to a merge SHA). Neither touched the plan.
  - LESSON for future supervisors of this plan: when a step embeds a script, diff the
    worker's on-disk script against the step file's fenced block before trusting the
    run (`sed -n '<start>,<end>p' step.md | sed 's/^       //' | diff - script`); a
    packet transcription error is invisible to acceptance greps. Prefer extracting
    embedded scripts from the step file mechanically and pre-staging them in the
    gitignored scratch dir over retyping them into the packet — that is how 46's
    three scripts were handled, and its script diffed clean on the first try.
  - LESSON: acceptance greps that assert a document CONTAINS a pinned string cannot
    tell "the worker ran the command" from "the worker copied the pin". Where the pin
    is a git SHA, prefer a content-level check (empty diff / blob hash) that stays
    true regardless of how the history was merged.

## Revisions

<!-- supervisor appends: phase | failed step | revision note | outcome -->

| phase | failed step | revision note | outcome |
| --- | --- | --- | --- |
| 2 | 24-concepts-analyzer | Phase 2 gate item 13 FAIL: `cargo run -- report` emitted `### Promoted` / `### Similarity` / `### Rounds` with no blank line after the heading (3 violations), breaking the repo-wide Markdown rule and step 24's own pinned render block. Root cause: the step's render test asserted only `contains("\n### Promoted\n")`, which the broken output also satisfies — a mis-specified (too weak) acceptance, not a wrong step. Fixed in flight (option b): step file 24 corrected to demand `"\n### Promoted\n\n- `"`-style assertions plus a structural no-heading-followed-by-text check, committed at 84a66dd; worker fix committed at 78fbc95. | fixed; gate re-run |
| 3 | none (found during phase 3 decomposition) | Amendment note from decomposer, two consistency defects. (1) Brief D15 concepts-small `[analyze]` list gained `agreement`: the pinned 5-analyzer list yields a 17-file run dir (discover-small's 15 per `tests/cli_discover.rs` EXPECTED_FILES + concepts.json + vocabulary.json), falsifying the "18 files" claims in D16(a)/(b) and the roadmap Phase 3 DoD; with `agreement` the small fixture mirrors the full-scale chain and every 18-file gate holds unchanged; D15 now also pins the 15+3 arithmetic. (2) Brief D16(d) standalone-analyze command gained `--mine-depths 6` and its parenthetical now requires `--mine-*` flags matching the fixture's `[mine]` table (fixture depths = [6], MineParams default [8] — byte-identity was unsatisfiable as written; matches the M2 precedent test); D13's stage-independence sentence now names both `[mine]` and `[induction]` tables. Both gate-neutral/strengthening; roadmap text unchanged (its claims are true post-amendment); completed phases 1-2 unaffected. | brief amended |
| 4 | 44-benchmark | SUPERVISOR-CAUSED, not a spec defect: the decomposer's `concept-bench.ps1` was correct, but the supervisor mis-transcribed two delimiter-row literals into the worker's packet, so the emitted `.md` had a 6-cell delimiter under the 5-column analyze-walls header and under the 7-column small-rounds header — two tables that do not render as tables at all in GFM. Step acceptance passed anyway (it greps data rows, never table shape), so this was caught only by the supervisor's independent "check the check" pass. Fix: retry (option a) with a corrected packet restoring the spec's exact two lines; the worker re-ran the script, regenerated both artifacts, and AMENDED its commit so the branch still carried exactly one commit. Post-fix the executed script diffs EMPTY against the step file's fenced spec, and a new table-shape acceptance (`delimiter cell count == header cell count`) was added and passes on the benchmark artifact, the determinism artifact, README, and the gate report. Step file 44 needed NO edit. | fixed; re-verified, merged at `9f2a6d4` |
| 4 | 46-gate | Mis-specified acceptance corrected IN FLIGHT (option b). Step 46's context pinned the tuning evidence `git log -1 --format=%h -- configs/tictactoe-concepts.toml` -> `0d13aed` and its acceptance demanded `grep -c '0d13aed'` -> 1, but `0d13aed` is step 32-fixtures' MERGE commit while `git log -1 -- <path>` reports the underlying step commit `f3ad73e` — an output honest execution cannot produce, so the check could only be satisfied by transcribing the pin rather than running the command (which is what happened). The substantive claim was independently confirmed TRUE three ways: only one commit ever touched the path, `git diff 0d13aed..HEAD -- configs/tictactoe-concepts.toml` is empty, and the blob is `f5027447e3d9c3de5d3c45fa874b6bf93a00ea8e` at both ends. Fix: supervisor corrected the step file's context (record both the real `git log` output and the stronger empty-diff/blob evidence) and its acceptance (`0d13aed` -> 2, new `f3ad73e` -> 1), and corrected the gate report's `## Tuning` bullets to match; the gate's `[induction]`/`[mine]` effective values were separately diffed against the live fixture and are correct. Verdict unaffected: 13/13 PASS. | fixed; step file + gate report committed with the phase-complete commit |
| 4 | none (found during phase 4 decomposition) | Amendment note from decomposer: project DoD item 11's game-concept sweep cannot report 0 as written — the M2 baseline `c997c35` carries ONE non-test hit, the `NamedOpening` rustdoc example (`name = "center"`) at `src/discovery/config.rs` line 40; every other occurrence is `#[cfg(test)]`, the game-NAME awk loop is clean, and `git diff c997c35..HEAD -- src/discovery/config.rs` is empty (planner re-verified all three facts). Resolution: gate text UNCHANGED — the note's suggested comment-strip and named-exception edits were REJECTED as gate-weakening exclusions needing human approval; instead the brief's Constraints bullet + Known-coupling section record the baseline fact and direct a phase-4 docs-only reword of that one line to `name = "opening-a"`, and roadmap Phase 4 Scope gains `src/discovery/config.rs` (docs-only). Sweep keeps full strength including comments; DoD item 11 verbatim; phases 1-3 unaffected. | brief+roadmap amended |
