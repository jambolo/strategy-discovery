# strategy-discovery-m2 — Ledger

Single source of truth for execution state. Sections are owned by different skills —
the planner seeds Plan + Phases; the decomposer fills Steps per phase; the supervisor
updates Steps and appends Revisions.

## Plan

- plan-name: strategy-discovery-m2
- current-phase: complete (4 of 4 phases done)
- working-branch: milestone/2-heuristic-mining
- starting-commit: 39ec658829bb8ec0c430a856b9ac03b4ae747719
- default-branch: develop
- artifacts-dir: implementation-artifacts/

## Phases

| Phase | Status  | Notes |
| ----: | ------- | ----- |
| 1     | complete | Contracts: featurizer, interpreter, dependencies, schema, config (brief D1-D3, D9; budget ≤ 50 s). DoD verified by the supervisor on the integrated tree: 22 `test result` lines all `ok`/`0 failed`; warm `cargo test --workspace` 44.0 s (gate measured 41.8 s) vs ≤ 50 s; fmt/clippy/doc/check exit 0, 0 doc warnings; Cargo.toml diff = exactly 3 added lines, single `ndarray v0.16.1`; `heuristic-rules` plays (`strategy=empty kind=heuristic-rules games=6 unfinished=0`, exit 0); featurizer 70/48 vocabulary tests pass; all three Milestone 1 SHA-256 pins and every pinned stdout line reproduce byte-for-byte; game-name/game-concept hygiene clean. |
| 2     | complete | Feature dataset and miner as analyzers (brief D4-D6; budget ≤ 65 s). DoD verified by the supervisor on the integrated tree at `a93b3e3`, independently of the gate report: `cargo check --workspace --all-targets`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` all exit 0 with 0 doc warnings; `cargo test --workspace` = 25 `test result` lines, every one `ok`, 0 failed; warm wall 46.0 s (gate measured 44.4 s) vs ≤ 65 s. `cargo test --test dataset` 4 passed (incl. `dataset_collapses_symmetric_duplicates`, `winning_cells_always_qualify`); `cargo test --test mine` 3 passed; `cargo test --lib discovery::tree` 6 passed (all six named tests). E2E in a supervisor-owned fresh dir `target/m2/sup-run`: generate + annotate + `analyze --analyzers summary,agreement,dataset,mine --mine-depths 4,6` exit 0, wrote `dataset.jsonl`/`dataset.json`/`heuristics.json`, `analyze.json` lists all four analyzers; repeat analyze left all six outputs byte-identical (`sha256sum -c` 6/6 OK); `heuristics.json` parses through `ConvertFrom-Json`. `report --input` printed `## Dataset`/`### Labels`/`## Mine`/`### mined-d4-l1`/`### mined-d6-l1` (1 each), `#### Rules` and `#### Feature definitions` (2 each), 2 rule lines matching the full `— support N, sound N (0.xxx), outcomes W/D/L a/b/c` regex, 2 `otherwise:` lines, 0 heading violations, 0 `\|---`. `analyze --list-analyzers` = exactly 4 tab lines (agreement, dataset, mine, summary); `analyze --mine-engine x` exit 2. `mining_bench -- --nocapture` = 1 `[bench] dataset-build` + 6 `[bench] mine engine=` lines. Game-name/game-concept awk checks exit 0 on dataset.rs, tree.rs, mine.rs, discovery/analyze.rs, cli/analyze.rs. |
| 3     | complete | `discover`, fixtures, e2e tests, ADRs, README (brief D7, D8, D10; budget ≤ 75 s). DoD verified by the supervisor on the integrated tree at `0a798a8`, independently of the gate report: `cargo check --workspace --all-targets`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` all exit 0 with 0 doc warnings; `cargo test --workspace` = 26 `test result` lines, every one `ok`, 0 failed; warm wall 46 s (gate measured 45.5 s) vs ≤ 75 s. Per-binary counts on the integrated tree: `cli_discover` 4, `cli_pipeline` 9 (incl. `pipeline_exhaustive_annotation_mode`), `cli_experiment` 4, `cli_evaluate` 4, `cli_analyze` 5, `cli_report` 3, `cli_agreement` 3 — the phase-2 binaries unchanged. Full-scale `cargo run -- discover --config configs/tictactoe-discover.toml --out target/m2/p3-discover` (fresh dir) exit 0, printing all four stage lines, four `strategy=mined-` lines, `evaluation=` and `discover=tictactoe-discover`; its stdout diffs EMPTY against the README's `### discover` `text` block (captured in a different worktree in an earlier session — cross-session, cross-worktree byte determinism); run dir holds 16 files incl. `dataset.jsonl`/`dataset.json`/`heuristics.json`/`discover.json`/`archive/entries.jsonl`. `discover --config nope.toml` exit 2. ADRs 0014/0015/0016 each: correct `# ADR 00NN:` title, `Accepted — 2026-08-23 (Milestone 2 / plan Phase 8)`, all four sections once, 0 heading-blank-line violations, 0 `\|---`, 0 untagged fences. README: `### discover` ×1, `## Usage` `discover` line ×1, `--mine-depths` documented, `[annotate] mode = "corpus"` + `[mine]` + `[discover]` in the config example, four new `### Run directory` rows plus `evaluate, discover` on the three archive/evaluation rows, 0 `not implemented`; `CLAUDE.md` Commands line lists `discover`. Game-name awk and game-concept grep clean on `src/cli/discover.rs` and `src/discovery/discover.rs`. Gate report `31-gate`: 14/14 items PASS, `Overall verdict: PASS`. |
| 4     | complete | Evidence, full-scale tuning, gate (brief D10 evidence, project DoD; budget ≤ 80 s). Steps `32` `4f9a9d8`, `33` `5c9e77d`, `34` `a55885f`, `35` `ad47850`; `32` and `35` ran in-tree (lone ready steps) so they have no merge commit. NO RETUNING: `configs/tictactoe-discover.toml` and `README.md` were never touched — the phase-3 pinned fixture already met the target, confirmed live, so the phase-3 README `### discover` staleness blocker resolved as a no-op. One human-approved brief/roadmap amendment (`6148998`, DoD 7 serial comparison) plus a decomposer revision of steps `32`/`35` (`0cfb937`); details in the Revisions table. PHASE DoD VERIFIED BY THE SUPERVISOR ON THE INTEGRATED TREE, independently of the gate report: `cargo check --workspace --all-targets`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo doc --no-deps --workspace` all exit 0 with 0 clippy and 0 doc warnings; `cargo test --workspace` exit 0 with 26 `test result:` lines, all `ok`, 0 `FAILED`; warm `Measure-Command` wall 47.30 s (gate measured 47.00 s) vs the ≤ 80 s budget — ~33 s headroom, phase 3 was 46 s so step `33`'s exhaustive bench addition cost ~1 s of wall (it runs concurrently with the other test binaries). Frozen-file `git diff 39ec658 --stat` over all eight paths prints nothing; `git diff 39ec658 -- tests/invariants.rs` empty; `git diff 39ec658 -- Cargo.toml` = exactly the three `+linfa/+linfa-trees/+ndarray` lines, no removals; `grep -rn "spikes/" src --include=*.rs \| grep -v "//"` empty; game-name awk loop 0 violations and game-concept sweep 0 hits. Determinism (DoD 7) re-verified from the gate's own run dirs by re-hashing every file with `sha256sum`: discover-2 vs discover-1 16/16 equal; discover-serial vs discover-1 14/16 equal with the difference set EXACTLY {`discover.json`, `archive/entries.jsonl`}; live `config_hash` values `cbcaffd750bb707b` (parallel) / `c3c8b37144634b33` (serial) matching the pins; both files byte-equal after normalizing each run's own value. Because discover-1 had already been rewritten by Item 8's `analyze` when the supervisor re-hashed, that 16/16 result independently re-proves Item 8's byte-identical-rewrite claim too. DoD 4 re-verified per archive entry: `mined-d8-l1`, `mined-d12-l1`, `mined-d0-l1` at `loss_rate_vs_reference == 0` with `0/0` by-seat losses against `perfect` (`mined-d6-l1` is `0.05` with `0/2` — consistent), `games_per_pairing 20`, roster `ttt-benchmark-v1`, `provenance.source == "discover"`, `corpus_run_id == discover.json.run_id == bd760ff0ace48705`, `seed 0`, and ALL 11 `provenance.discovery.*` leaves populated on all four entries (`miner cart-v1`, `tiers primitive,supplied`, `dataset_rows 557`, `mine_seed 0`; note `mine_seed` and `mined-d0-l1`'s `max_depth` are legitimately `0` — a naive PowerShell emptiness test coerces `0 -eq ''` and falsely reports them missing). DoD 3: `report.md` has `## Mine` ×1, `#### Rules` ×4, `#### Feature definitions` ×4 and 118 lines matching the full `^[0-9]+\. if .* then play a position in .* — support N, sound N (0.xxx), outcomes W/D/L a/b/c$` regex; `heuristics.json` 4 candidates, all with a non-null `heuristic`. DoD 6: `dataset.json` columns 70, rows 557, collapsed 1734, 17 classes, `label_counts` present. DoD 9: all three Milestone 1 SHA-256 pins reproduce byte-for-byte (`1487b18b…`, `6834c0e3…`, `8f5a1bd2…`). DoD 10: ADRs 0014/0015/0016 each have the correct `# ADR 00NN:` title, the `Accepted — 2026-08-23 (Milestone 2 / plan Phase 8)` status, all four sections, 0 heading violations, 0 `\|---`; README has `### discover` and Run-directory rows for all four new files; `CLAUDE.md` Commands line lists `discover`. GATE REPORT ACCURACY (not just its greps): the `## Tuning` fixture table was diffed key-by-key against the live `configs/tictactoe-discover.toml` (seed 20260823, games_per_cell 40, plies [0,1,2], random/depth-2/depth-4/perfect at minimax depths 2/4/9, evaluators ["default"], `[mine]` cart / [6,8,12,0] / min_leaf 1, `[discover]` games 20 seed 0, analyzers summary,agreement,dataset,mine) — every value correct; its candidate table (0.050/0.970/16/0, 0.000/0.979/24/1, 0.000/0.990/38/5, 0.000/0.990/40/5) matches the supervisor's own read of discover-1's `discover.json` + `heuristics.json`. Gate report `35-gate`: 12/12 items PASS plus the Frozen-files group, `Overall verdict: PASS`. |

## Steps

Every step's `files_in_scope` also includes its own report `implementation-artifacts/strategy-discovery-m2-<id>-report.md` (omitted from the rows below).

| id | phase | status | files | commit |
| --- | --- | --- | --- | --- |
| 01-deps | 1 | done | Cargo.toml, Cargo.lock | 261076b |
| 02-featurizer | 1 | done | src/core/traits.rs, src/core/featurizer.rs, src/core/mod.rs | 4894144 |
| 03-schema-types | 1 | done | src/io/schema.rs, src/io/mod.rs, src/cli/evaluate.rs, src/discovery/archive.rs | 6250621 |
| 04-bundle-featurizer | 1 | done | src/discovery/bundle.rs, src/strategy/registry.rs, src/games/tictactoe/corpus.rs, src/games/tictactoe/mod.rs | e9cb0cb |
| 05-interpreter | 1 | done | src/core/dsl.rs, src/core/interpreter.rs, src/strategy/registry.rs, tests/match_engine.rs, tests/cli_evaluate.rs | 776d6f2 |
| 06-interpreter-tactics | 1 | done | tests/interpreter_tactics.rs | 94d8552 |
| 07-analyze-options | 1 | done | src/discovery/analyze.rs, src/discovery/agreement.rs, src/discovery/summary.rs, src/cli/analyze.rs, src/discovery/experiment.rs | 2a0c4fc |
| 08-experiment-config | 1 | done | src/discovery/experiment.rs, src/discovery/mod.rs | 5e82f94 |
| 09-gate | 1 | done | report only (implementation-artifacts/strategy-discovery-m2-09-gate-report.md) | 8a1c6d5 |
| 10-stale-unimplemented-tests | 1 | done | src/discovery/evaluate.rs, src/discovery/tournament.rs | 9a61668 |
| 11-dataset | 2 | done | src/discovery/dataset.rs, src/discovery/mod.rs | e090ff6 |
| 12-tree | 2 | done | src/discovery/tree.rs, src/discovery/mod.rs | 0c5d85f |
| 13-mine | 2 | done | src/discovery/mine.rs, src/discovery/mod.rs | 3318b7b |
| 14-dataset-tests | 2 | done | tests/dataset.rs | 2ad785f |
| 15-mine-linfa | 2 | done | src/discovery/mine.rs | 841d9d2 |
| 16-analyze-wiring | 2 | done | src/discovery/analyze.rs, src/cli/analyze.rs, src/cli/mod.rs, tests/cli_analyze.rs | a3abdd1 |
| 17-mine-tests | 2 | done | tests/mine.rs | 1b5850c |
| 18-mining-bench | 2 | done | tests/mining_bench.rs | 5cef0a5 |
| 19-cli-mine-e2e | 2 | done | tests/cli_analyze.rs, tests/cli_report.rs | 62a09a8 |
| 20-gate | 2 | done | report only (implementation-artifacts/strategy-discovery-m2-20-gate-report.md) | a93b3e3 |
| 21-pipeline-stages | 3 | done | src/cli/pipeline.rs | f2b84e2 |
| 22-discover-lib | 3 | done | src/discovery/discover.rs, src/discovery/mod.rs | ff85658 |
| 23-fixtures | 3 | done | configs/tictactoe-discover.toml, tests/fixtures/discover-small.toml | 33ff160 |
| 24-adr-dataset | 3 | done | docs/adr/0014-feature-dataset.md | f9fccff |
| 25-adr-miner | 3 | done | docs/adr/0015-heuristic-miner.md | 5fcbd9f |
| 26-adr-interpreter | 3 | done | docs/adr/0016-rule-interpreter.md | fc483eb |
| 27-cli-discover | 3 | done | src/cli/discover.rs, src/cli/mod.rs, src/cli/evaluate.rs | e3e2053 |
| 28-pipeline-exhaustive-test | 3 | done | tests/cli_pipeline.rs | 51ebb97 |
| 29-discover-e2e | 3 | done | tests/cli_discover.rs | 68fb252 |
| 30-readme | 3 | done | README.md, CLAUDE.md | 74960a7 (+ supervisor integration commit 54c6cf7) |
| 31-gate | 3 | done | report only (implementation-artifacts/strategy-discovery-m2-31-gate-report.md) | 0a798a8 |
| 32-determinism | 4 | done | artifacts/reproducibility/discover-determinism.md | 4f9a9d8 (in-tree, no merge commit) |
| 33-bench-exhaustive | 4 | done | tests/mining_bench.rs | 5c9e77d |
| 34-mining-induction | 4 | done | artifacts/benchmarks/mining-induction.md, artifacts/benchmarks/mining-induction.json | a55885f (+ supervisor integration commit, see Revisions) |
| 35-gate | 4 | done | implementation-artifacts/strategy-discovery-m2-gate-report.md (project gate report; plus its own worker report) | ad47850 (in-tree, no merge commit) |

### Phase 1 notes

- Dependency graph: `01`, `02`, `03` start immediately in parallel (disjoint scopes). `04` needs `02`; `07` needs `03`; `05` needs `04`; `08` needs `03` + `07`; `06` needs `05`; `10` needs `05`; `09` (gate) needs `01`, `06`, `08`, `10` (transitively everything). Maximum concurrency: `{01, 02, 03}` then `{04, 07}` then `{05, 08}` then `{06, 10}` (disjoint scopes, no content coupling) then `{09}` — waves may overlap where a slow independent step is still running.
- Same-file sequential overlaps (deliberate, ordered by `depends_on`, NOT co-parallel): `src/strategy/registry.rs` is edited by `04` (EngineBundle field) then `05` (build_heuristic_rules + tests); `src/discovery/experiment.rs` by `07` (one-line `mine:` literal) then `08` (sections + resolution).
- Atomicity couplings honored by the decomposition: adding `EngineBundle.featurizer` breaks all three struct-literal sites at once, so `04` owns registry.rs + bundle.rs + tictactoe mod.rs together; making `heuristic-rules` playable flips `Unimplemented` assertions at FOUR sites, not two — `tests/match_engine.rs`, `tests/cli_evaluate.rs` (owned by `05`), plus the in-lib `#[cfg(test)]` tests in `src/discovery/evaluate.rs` and `src/discovery/tournament.rs` (missed by `05`'s scope, added as revision step `10`). All remaining `Unimplemented` occurrences are legitimate and must stay: the `evolutionary`/`llm` kinds and the `src/cli/error.rs` exit-code mapping.
- Revision (post-`05` merge): `05` is correct and stays `done`; full `cargo test --lib` on the integrated branch failed (341 passed; 2 failed) because `05`'s action list ran only module-filtered lib tests, hiding the two in-lib stale assertions. `10-stale-unimplemented-tests` flips them (renames: `heuristic_rules_plays_and_is_evaluated`, `heuristic_rules_under_test_plays_and_is_scored`; asserts `Ok` shape, `games`/`unfinished`, never seed-dependent win/draw/loss counts) and removes the then-dangling `MatchError`/`StrategyError` test imports. `09`'s depends_on gained `10`. Lesson for later phases: any step that changes strategy-kind behavior must run a FULL `cargo test --lib` in its own worktree, not only module filters.
- Emergent contracts later phases must honor: tic-tac-toe featurizer vocabulary order `side_to_move` + 47 tier-1 + 22 `ttt.*` = 70 definitions (48 without supplied); `EngineBundle.featurizer: Option<Arc<Featurizer<G>>>` with the registry error string `requires a feature context` when `None`; `DslError::UnavailableFeatures(Vec<String>)`; `RuleInterpreter` evaluates in the canonical frame and `Maximize` scores successors from the NEXT mover's (opponent's) perspective — to be documented in ADR 0016 (phase 3), not a bug; `AnalyzeOptions.mine: MineParams` is defaulted everywhere until phase 2 adds `--mine-*` flags and the `dataset`/`mine` analyzers; `ResolvedExperiment.annotate_mode` + `ResolvedExperiment.discover: ResolvedDiscover` exist but nothing consumes them until phase 3 (`discover` subcommand, pipeline exhaustive branch); the phase 2/3 record contracts (`MineParams`, `Dataset*`, `Mining*`, `Discover*`, `Provenance.discovery`) live in `src/io/schema.rs`, re-exported from `src/io/mod.rs`.
- Registry test helper contract: `strategy::registry` tests keep `fn bundle()` with `featurizer: None` (exercises the no-featurizer error path); featurizer-bearing tests build over `crate::games::tictactoe::engine_bundle()`.
- Bootstrap: after `01` merges, a fresh worktree's first `cargo check`/`test` cold-builds `linfa`/`linfa-trees`/`ndarray` (adds tens of seconds once per worktree); no source references them until phase 2 — unused dependencies are not a clippy error.
- Budget: Milestone 1 baseline warm `cargo test --workspace` was 41.4-42.6 s; phase 1 adds only fast tests (heaviest: `interpreter_tactics` plays a handful of games, `cli_evaluate` adds one 6-game evaluate run) — the ≤ 50 s phase budget has margin. The gate (`09`) measures it.

### Phase 2 notes

- Dependency graph: `11-dataset` → `12-tree` → `13-mine` are serialized because each owns
  `src/discovery/mod.rs` (module declaration + re-export must land WITH each new module so its
  tests compile; the brief's "one step per phase edits each mod.rs" is honored as one ordered
  owner-chain, never co-parallel). `14` needs `11` only and runs alongside `12`/`13`/`15`/`16`.
  After `13`: `{15-mine-linfa, 16-analyze-wiring}` in parallel (disjoint scopes). `17`, `18` need
  `15`; `19` needs `16`; `{17, 18, 19}` co-parallel (disjoint test files). `20-gate` needs
  `{14, 17, 18, 19}` (transitively everything). Waves: `{11}` → `{12, 14}` → `{13}` → `{15, 16}`
  → `{17, 18, 19}` → `{20}`.
- Same-file sequential overlaps (ordered by `depends_on`, NOT co-parallel): `src/discovery/mod.rs`
  by `11` → `12` → `13`; `src/discovery/mine.rs` by `13` → `15`; `tests/cli_analyze.rs` by
  `16` → `19`.
- Transient window: between the `13` and `15` merges, `[mine] engine = "linfa-trees"` passes
  `MineParams::validate()` but fails at mining with `` "mine engine `linfa-trees` is not wired
  yet" `` — deliberate; no phase-DoD command samples that window and `16`'s tests use `cart` only.
- Stale-site audit done at decomposition: growing `builtin_registry` to four analyzers breaks only
  `src/discovery/analyze.rs::builtin_registry_names` (owned by `16`);
  `tests/cli_agreement.rs::list_analyzers_lists_both_builtins` counts per-name lines and survives;
  `tests/cli_analyze.rs::lists_registered_analyzers` was containment-only and is strengthened by
  `16`; the "known analyzers:" error texts are only `contains`-checked anywhere. Phase 1 lesson
  applied: `16` runs a full `cargo test --lib` plus the five analyze-adjacent integration binaries
  in its own worktree.
- Emergent contracts phase 3 must honor: `dataset_from_context` in `src/discovery/dataset.rs` is
  the ONE analyzer path from `AnalyzeContext` to `Dataset` (the `mine` analyzer never reads
  `dataset.jsonl`; `discover` must reuse the analyzers, not re-derive); `heuristics.json` is a
  `MiningReport` readable via `read_json`; candidate names are `mined-d{depth}-l{min_leaf}` (cart)
  / `mined-linfa-d{depth}-l{min_leaf}`, depth = the REQUESTED `[mine] depths` entry; report
  anchors are `## Dataset`, `### Labels`, `## Mine`, `### {candidate}`, `#### Rules` (numbered
  lines with an em dash `—` before `support`), an `otherwise:` paragraph, `#### Feature
  definitions` bullets; predicate emission floors linfa midpoints for int/set columns so both
  engines emit identical `Int` thresholds on integral data (float columns differ: `Le/Gt` data
  value for cart, `Lt/Ge` midpoint for linfa); `MineAnalyzer` maps `GeneratorError::Failed` to
  `CorpusError::Config` (exit 2); `cart` is the only engine on any determinism-evidence path.
- Budget: phase 1 warm `cargo test --workspace` wall was 44.0 s vs the ≤ 65 s phase budget. Cost
  containment built into the steps: `tests/dataset.rs` and `tests/mine.rs` each run exactly ONE
  exhaustive annotation per process behind a `OnceLock` (~3-5 s per binary), `tests/mining_bench.rs`
  benches over the generate-small corpus (release + exhaustive numbers belong to phase 4), the e2e
  additions cost ~3 s (`cli_analyze`) and ~2.5 s (`cli_report`). Estimate ≈ 62 s; gate Item 3
  measures.
- Planning-time empirics (verified against the amended brief at `60c4973`): exhaustive dataset ≈
  627 rows × 70 columns, 17 set-kind classes, 4520 non-terminal records; 321 rows have
  `ttt.winning_cells > 0` (27 of them legitimately label a smaller Supplied class — the amended
  D4 test (c) asserts qualify + Supplied-tier label, NOT label equality); unlabeled ≈ 5 rows
  (~0.8%, far under the ~15% amendment threshold in the roadmap risk note).
- Execution record: the phase ran in six waves as planned. The wave-4 supervisor invocation was
  killed mid-flight by a session limit, leaving two orphaned worktrees each holding ONE unmerged,
  unverified worker commit (`15-mine-linfa` at `047e9c1`, `16-analyze-wiring` at `624a51f`, both
  based on `96752ec`). The resuming supervisor treated them as ordinary unverified claims: re-ran
  every acceptance command in each worktree (both full `cargo test --lib`, fmt, clippy, doc, the
  greps, the five analyze-adjacent integration binaries for `16`, and both CLI invocations),
  confirmed each diff was a subset of its `files_in_scope` with exactly one commit, then merged.
  Both passed unchanged — no rework. Lesson: an interrupted wave costs only re-verification, and
  worktrees must never be discarded before verification.
- Post-integration counts phase 3 must not break (measured on the integrated tree): `cargo test
  --workspace` = 25 `test result` lines, all `ok`; `--lib` 359; `--test dataset` 4; `--test mine`
  3; `--test mining_bench` 1; `--test cli_analyze` 5; `--test cli_report` 3; `--test cli_agreement`
  3; `--test cli_pipeline` 8; `--test cli_experiment` 4. Warm `cargo test --workspace` wall 46.0 s,
  leaving ~29 s of headroom under the Phase 3 ≤ 75 s budget.
- Small-corpus (generate-small, 190 dataset rows × 70 columns) mining numbers from
  `tests/mining_bench.rs`, useful as phase 4 tuning priors: cart d4 = 11 rules / 11 leaves /
  soundness 0.926, d8 = 22 / 24 / 0.968, d0 = 27 / 29 / 0.984; linfa-trees d4 = 9 / 9 / 0.921,
  d8 = 17 / 19 / 0.937, d0 = 17 / 19 / 0.937 (linfa saturates before depth 8 here). Repeat
  `analyze` over the same corpus rewrote all six outputs byte-identically (sha256 verified), so
  the D8 determinism work in phase 3 starts from a byte-stable analyze stage.
- Decomposition lesson for phase 3: step `19`'s acceptance pinned `cargo test --test cli_report`
  at `running 4 tests` on the belief that three tests pre-existed; the file held two. Any
  acceptance that counts tests in an EXISTING binary must be derived from the working branch at
  decomposition time (`grep -c '#\[test\]' <file>`), never assumed — `cli_analyze`'s count in the
  same step was derived that way and was correct.

### Phase 3 notes

- Dependency graph: `{21, 22, 23, 24, 25, 26}` start immediately in parallel (disjoint scopes:
  pipeline refactor, discover library module, fixtures, three ADRs). `27-cli-discover` needs
  `21 + 22 + 23`; `28-pipeline-exhaustive-test` needs `21`; `{27, 28}` co-parallel (disjoint).
  `29-discover-e2e` needs `27`; `30-readme` needs `27`; `{29, 30}` co-parallel (disjoint).
  `31-gate` needs `{24, 25, 26, 28, 29, 30}` (transitively everything). Waves:
  `{21, 22, 23, 24, 25, 26}` → `{27, 28}` → `{29, 30}` → `{31}`. No same-file overlap anywhere
  in the phase, sequential or parallel: each file has exactly one owning step.
- Decomposed against the brief AT amendment `a0a5625` (D8 test (c) and project DoD 8 gained
  `--mine-*` flags on the standalone `analyze` commands — flagless byte-identity was
  unsatisfiable because standalone `analyze` reads mine parameters only from flags). Step `29`
  embeds the amended command (`--mine-depths 6`).
- Emergent contracts fixed by this decomposition (phase 4 and the supervisor must honor):
  `run_stages` (src/cli/pipeline.rs, pub(crate)) prints the four stage lines but NEVER the
  `pipeline=` trailer — `pipeline` prints the trailer, `discover` prints its own final
  `discover={name} run_id={} heuristics={} archive_entries={} out={}` line; exhaustive annotate
  mode skips the corpus-file `require_file` preconditions (so `--stages annotate` works into an
  empty dir); `discover.json`'s `archive` field uses forward-slash relative form via
  `archive_display` (absolute paths pass through as `Display`); miner ids `cart-v1` /
  `linfa-trees-0.8.1` map in `src/discovery/discover.rs::miner_id`; `discover` computes
  `config_hash` on the experiment config AS LOADED, BEFORE forcing `dataset`/`mine` into the
  analyzer list; `Provenance.annotations_run_id`/`annotations_mode` mirror `evaluate` (from
  `report.config.annotations`), while `DiscoveryProvenance.annotations_mode` comes from the
  loaded annotate metadata via `mode_name`.
- README coupling (phase 4 warning): step `30` embeds the VERBATIM stdout of
  `discover --config configs/tictactoe-discover.toml --out target/m2/p3-discover` as the ONLY
  fenced `text` block of `### discover`, and gate item 13 extracts that block and diffs it
  exactly against a fresh run. Phase 4 retunes `configs/tictactoe-discover.toml`, which changes
  that stdout — phase 4 MUST refresh the README block in the same step that retunes the config
  (README.md is not currently in the phase 4 scope list; add it via revision/amendment when
  tuning starts, or the phase 3 gate evidence goes stale silently).
- Worktree-relative counts: step `27`'s acceptance pins `cargo test --test cli_pipeline` at
  `running 8 tests` — valid only in `27`'s own worktree (base lacks co-parallel `28`); the
  integrated count after `28` merges is 9, and gate items 5/8 use the integrated numbers
  (26 `test result:` lines workspace-wide, 9 cli_pipeline tests). Verify `27` in its worktree,
  never on the post-`28` integrated tree.
- Budget: phase 2 warm `cargo test --workspace` wall was 46.0 s vs the ≤ 75 s phase budget.
  New cost: `cli_discover` ≈ 9-10 s wall (4 tests in parallel threads; heaviest runs two
  discover-small runs ≈ 4 s each), `cli_pipeline` +≈2 s (exhaustive case runs alongside
  existing tests), lib additions negligible. Estimate ≈ 57-59 s; gate item 6 measures.
- Full-scale debug `discover` (step `30` and gate item 10) takes minutes — expected and
  acceptable; both write `target/m2/p3-discover`, and the gate `rm -rf`s the directory first so
  nothing relies on `generate`'s overwrite behavior.
- Environment reminders baked into the step packets: one cargo command per worker tool call;
  Bash output > ~8 KB truncates (gate redirects long outputs to `target/m2/` files); TOML
  written from tests embeds absolute sweep/roster paths with forward slashes
  (`CARGO_MANIFEST_DIR` + `.replace('\\', "/")`); no regex crate in dev-deps — report-line
  checks use `starts_with`/`contains`.
- Execution record: the phase ran in the four planned waves with ZERO decomposer round-trips
  and zero retries — `{21, 22, 23, 24, 25, 26}` → `{27, 28}` → `{29, 30}` → `{31}`. Every step
  passed supervisor re-verification on its first submission; every decomposer-predicted stdout
  value was exact (notably step `27`'s pinned discover-small line set, down to
  `cells=36 games=144 positions=1083`, `distinct_games=115 canonical_coverage=0.312
  decisive_fraction=0.549`, and `archive_entries=1`). The only correction was a supervisor
  integration commit on step `30`'s README (see the Revisions row): two `[discover]` example
  values were factually wrong although every acceptance grep passed. `31-gate` ran IN-TREE (no
  worktree) as a lone ready step, after the ledger was committed and the tree was clean.
- Post-integration counts phase 4 must not break (measured on the integrated tree at
  `0a798a8`): `cargo test --workspace` = 26 `test result` lines, all `ok`; `--lib` 362;
  `--test cli_discover` 4; `--test cli_pipeline` 9; `--test cli_experiment` 4;
  `--test cli_evaluate` 4; `--test cli_analyze` 5; `--test cli_report` 3;
  `--test cli_agreement` 3; `--test dataset` 4; `--test mine` 3; `--test mining_bench` 1.
  Warm `cargo test --workspace` wall 46 s, leaving ~34 s of headroom under the Phase 4
  ≤ 80 s budget.
- PHASE 4 BLOCKER (carried from the decomposition, now confirmed against live evidence): the
  README's `### discover` fenced `text` block is the VERBATIM stdout of
  `discover --config configs/tictactoe-discover.toml --out target/m2/p3-discover`, and gate
  item 13 diffs it EXACTLY (empty diff, no "modulo out=" allowance) against a fresh run. The
  supervisor reproduced that empty diff from a different worktree in a later session, so the
  coupling is real and tight. Phase 4 retunes `configs/tictactoe-discover.toml`, which changes
  that stdout — `README.md` is NOT in the Phase 4 scope list in the roadmap, so phase 4 must
  add it (revision or amendment) to whichever step retunes the config, and refresh the block in
  that SAME step, or the phase 3 gate evidence goes stale silently.
- Phase 4 tuning priors from the phase-3 full-scale run (`configs/tictactoe-discover.toml` as
  pinned: seed 20260823, games_per_cell 40, `random_opening_plies = [0, 1, 2]`, strategies
  random/depth-2/depth-4/perfect, `[mine] engine = "cart" depths = [6, 8, 12, 0] min_leaf = 1`,
  `[discover] games = 20`): run_id `bd760ff0ace48705`, 48 cells / 1920 games / 14887 positions,
  2291 corpus annotations, `canonical_coverage=0.902 decisive_fraction=0.526
  diversity_pass=true`, `report.md` 48001 bytes, evaluation `78bcde7c9113b250` over roster
  `ttt-benchmark-v1` (reference `perfect`, 280 games per candidate), archive_entries 4.
  Candidate loss rates vs `perfect` ALREADY at the Phase 4 target for three of four:
  `mined-d6-l1` 0.050 (agreement 0.970, novelty 1.000), `mined-d8-l1` 0.000 (0.979, 0.039),
  `mined-d12-l1` 0.000 (0.990, 0.031), `mined-d0-l1` 0.000 (0.990, 0.004). Phase 4's "no
  candidate reaches loss rate 0.000" risk therefore looks unlikely to fire with these values;
  note `mined-d8-l1` has 19 losses yet loss_rate_vs_reference 0.000 (the headline is measured
  against the REFERENCE pairing only, not the whole roster) — do not confuse the two when
  writing the evidence artifacts.

### Phase 4 notes

- Decomposed against the roadmap AT amendment `ef2df9c` (Phase 4 scope gained
  `tests/mining_bench.rs`, additive-only, as the D10/DoD-11 exhaustive + holdout measurement
  vehicle — no committed vehicle could measure the exhaustive dataset-build time or holdout-0.2
  fit metrics: the CLI analyze stage hard-requires `run.json`, and the bench fitted only
  generate-small at holdout 0.0).
- Dependency graph: `{32-determinism, 33-bench-exhaustive}` start immediately in parallel
  (disjoint scopes: reproducibility artifact vs test file). `34-mining-induction` needs `33`
  (parses the new `[bench]` line formats — a verbatim contract; any format change in `33` must
  revise `34`). `35-gate` needs `32` + `34` (transitively `33`). Waves: `{32, 33}` → `{34}` →
  `{35}`; `34` may launch while `32` is still running.
- NO RETUNING NEEDED — decided at decomposition against live evidence: `git diff
  0a798a8..3255c6e` touches only the ledger, so code + config at the phase-4 base are byte-identical
  to what the phase-3 supervisor verified full-scale (run_id `bd760ff0ace48705`, three of four
  candidates at `loss_rate_vs_reference=0.000`), and byte-determinism guarantees reproduction.
  Therefore `configs/tictactoe-discover.toml` and `README.md` are deliberately owned by NO
  step and stay untouched; the README `### discover` stdout block stays valid (the phase-3
  README-staleness blocker resolves as a no-op); the roadmap's "final tuned values" are the
  pinned values, recorded in the gate report's `## Tuning` section per the phase DoD.
- Contingency: `32` (drift gate: run_id + pinned mined-d12-l1 line), `34` (TARGET-MISS marker)
  and gate items 3/4/7 each independently detect target or determinism drift and FAIL rather
  than improvise. If any fires, the supervisor sends a revision note; an actual tuning step
  would then need `configs/tictactoe-discover.toml` + `README.md` in ONE step's scope
  (refreshing the README `### discover` stdout block in the same commit) plus a roadmap scope
  amendment for README.md.
- Full-scale pins for revision context (from the README block / phase-3 run): stdout 10 lines;
  candidates `mined-d6-l1` 0.050/0.970/1.000, `mined-d8-l1` 0.000/0.979/0.039 (19 losses
  roster-wide yet 0 vs `perfect` — the headline measures the reference pairing only),
  `mined-d12-l1` 0.000/0.990/0.031, `mined-d0-l1` 0.000/0.990/0.004; evaluation
  `78bcde7c9113b250`; run dir = 16 files.
- Supervisor execution notes: run `35` IN-TREE as a lone ready step after `32`/`33`/`34` are
  merged and the ledger commit lands (phase-3 `31-gate` precedent) — `35` asserts both
  evidence artifacts on its base, so they must be merged first, and its runs live under the
  gitignored `target/m2/`. Full-scale debug `discover` runs need a 600000 ms Bash-call timeout
  (parallel ≈ 1-4 min, serial ≈ 2-5 min; the release runs in `34` are much faster). Long
  outputs go to files (8 KB truncation); one cargo command per worker call; fresh `--out` dirs
  for every discover invocation; lowercase SHA-256 everywhere.
- Budget: pre-phase warm `cargo test --workspace` wall 46 s vs ≤ 80 s; `33` adds ~4-7 s
  (one exhaustive annotation + six sub-second fits inside `mining_bench`), expect ~50-55 s.
  Gate item 1 measures. `[bench]` line count after `33` = exactly 14 (1+6 existing, 1+6 new).

- EXECUTION RECORD (phase 4, COMPLETE). Waves: `{32, 33}` → `{34}` → `{32 re-run}` → `{35}`.
  The first `{32, 33}` wave ran in worktrees; `34` in a worktree; step `32`'s re-run and the
  `35` gate ran IN-TREE as lone ready steps (clean tree, ledger committed, warm build — the
  phase-3 `31-gate` precedent). Step `32` failed its first attempt on a real brief defect, was
  unblocked by a human-approved amendment, and passed on re-run. `33`, `34` and `35` each
  passed on first submission. Nothing was worked around and no gate was weakened locally.
- Verified by the supervisor ON THE INTEGRATED BRANCH (independent re-runs, not worker reports):
  - Step `33`: `cargo test --test mining_bench -- --nocapture` exit 0, `test result: ok. 1 passed`,
    exactly 14 `[bench]` lines (7 carrying `source=exhaustive`), 1 `dataset-build rows=`,
    6 `mine engine=`, 3 `mine source=exhaustive engine=cart `, 3 `... engine=linfa-trees `,
    6 with a numeric `holdout_soundness=`; `cargo fmt --all --check` and
    `cargo clippy --workspace --all-targets --all-features -- -D warnings` exit 0. Additivity
    confirmed by diff: one deletion (the `use ...annotate::{...}` line, extended), 58 insertions,
    every pre-existing `[bench]` line byte-unchanged and still matching the phase-2 recorded
    values (cart 11/11/0.926, 22/24/0.968, 27/29/0.984; linfa 9/9/0.921, 17/19/0.937, 17/19/0.937);
    one real `#[test]` (the second `#\[test\]` grep hit is inside the module doc comment).
    New exhaustive line reports rows=627 columns=70, matching the decomposition's planning empirics.
  - Step `34`: all 12 acceptance commands re-run and matched; see the Revisions row for the three
    defects found BEYOND acceptance and the live re-measurement that validated the numbers.
  - NO RETUNING WAS NEEDED, now confirmed against live behavior rather than inference: an
    independent supervisor `discover --config configs/tictactoe-discover.toml` run at the phase-4
    base reproduced all ten pinned stdout lines byte-for-byte, including
    `run_id=bd760ff0ace48705`, `cells=48 games=1920 positions=14887`, `report=...bytes=48001`,
    `evaluation=78bcde7c9113b250`, and `mined-d8-l1` / `mined-d12-l1` / `mined-d0-l1` at
    `loss_rate_vs_reference=0.000`. `configs/tictactoe-discover.toml` and `README.md` were never
    touched, so the phase-3 README `### discover` stdout block remains valid and the phase-3
    README-staleness blocker resolved as a genuine no-op.
  - Frozen-file gate (roadmap Phase 4 DoD, third bullet) re-run on the integrated branch:
    `git diff 39ec658 --stat -- docs/plan.md docs/component-evaluation.md
    docs/adr/0001-game-player-minimax-adapter.md docs/adr/0013-serde-json-float-roundtrip.md
    configs/tictactoe-default.toml configs/tictactoe-experiment.toml
    artifacts/benchmarks/tournament-throughput.md artifacts/reproducibility/tournament-determinism.md`
    prints nothing — PASS.
  - Project DoD 2 re-run: `git diff 39ec658 -- Cargo.toml` changed lines are exactly
    `+linfa = "0.8.1"`, `+linfa-trees = "0.8.1"`, `+ndarray = "0.16.1"`, no removals — PASS.
  - Project DoD 10 code-hygiene halves re-run: the game-name awk loop over every
    `grep -rl tictactoe src --include=*.rs` file outside `src/games/` and `src/cli/games.rs`
    reports 0 violations; the game-concept sweep over `src/core src/discovery src/io src/cli`
    (`threat|fork|corner|center|edge` outside `#[cfg(test)]` and comments) reports 0 hits — PASS.
- MILESTONE 2 DEFINITION OF DONE — final status at the phase-complete commit, every item
  checked by the supervisor on the integrated tree independently of the gate report (the gate
  report `implementation-artifacts/strategy-discovery-m2-gate-report.md` records 12/12 items
  PASS plus the Frozen-files group and ends `Overall verdict: PASS`; the evidence below is the
  supervisor's own re-run, not a restatement of it):
  1. PASS — `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo fmt --all
     --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
     `cargo doc --no-deps --workspace` all exit 0; 26 `test result:` lines, all `ok`, 0
     `FAILED`; 0 clippy warnings, 0 doc warnings; warm `Measure-Command` wall 47.30 s ≤ 80 s.
  2. PASS — `git diff 39ec658 -- Cargo.toml` changed lines are exactly `+linfa = "0.8.1"`,
     `+linfa-trees = "0.8.1"`, `+ndarray = "0.16.1"`, no removals; `grep -rn "spikes/" src
     --include=*.rs | grep -v "//"` empty.
  3. PASS — full-scale `discover` into `target/m2/discover-1` exits 0 and prints the pinned
     ten-line stdout block; `heuristics.json` 4 candidates each with a non-null `heuristic`;
     `report.md` has `## Mine` ×1, `#### Rules` ×4, `#### Feature definitions` ×4 and 118
     lines matching the full rule regex.
  4. PASS — `archive/entries.jsonl` 4 entries; `mined-d8-l1`, `mined-d12-l1`, `mined-d0-l1` at
     `loss_rate_vs_reference == 0` with `0/0` by-seat losses vs `perfect`; `games_per_pairing
     20`; roster `ttt-benchmark-v1`; `provenance.source == "discover"`; `corpus_run_id ==
     discover.json.run_id == bd760ff0ace48705`; `seed 0`; all 11 `provenance.discovery.*`
     leaves populated on all four entries (`miner cart-v1`, `tiers primitive,supplied`,
     `dataset_rows 557`, `mine_seed 0`); `agreement_rate` numeric on every entry.
  5. PASS — `cargo test --test interpreter_tactics --test match_engine --test cli_evaluate`
     all ok / 0 failed with the three required test names present, and `evaluate` on the
     heuristic fixture exits 0 printing `strategy=empty kind=heuristic-rules` (gate Item 5;
     covered by the supervisor's own clean `cargo test --workspace`).
  6. PASS — `discover-1/dataset.jsonl` and `dataset.json` exist; `columns` 70, `rows` 557,
     `collapsed` 1734, 17 `classes`, `label_counts` present; `cargo test --test dataset` 4
     passed including `*collapses_symmetric_duplicates*`.
  7. PASS under the amended (human-approved) contract — discover-2 vs discover-1 16/16
     SHA-256-identical; discover-serial vs discover-1 14/16 identical with the difference set
     exactly {`discover.json`, `archive/entries.jsonl`}, both byte-equal after normalizing each
     run's own `config_hash` (`cbcaffd750bb707b` parallel / `c3c8b37144634b33` serial); stdout
     identical modulo `out=` for all three runs. Recorded in
     `artifacts/reproducibility/discover-determinism.md` (8-row header, exactly 4
     `identical: YES`, 0 `identical: NO`, both hash values stated, `config_hash` named as the
     sole differing field). All 48 hashes in that artifact's `## Runs` table were re-computed
     by the supervisor with `sha256sum` and match.
  8. PASS — the re-`analyze` of discover-1 left all six outputs byte-identical (independently
     re-proved: after that rewrite, discover-1 still hashes 16/16 equal to discover-2);
     `report --input` stdout equals `report.md`; the `discover-copy` mine rewrite matches;
     `analyze --list-analyzers` lists agreement, dataset, mine, summary.
  9. PASS — `annotate --exhaustive` prints the pinned `mode=exhaustive annotated=5478
     terminal=958 disagreements=0`; `evaluate` reproduces the pinned stdout; all three
     Milestone 1 SHA-256 pins reproduce byte-for-byte (`1487b18b…`, `6834c0e3…`, `8f5a1bd2…`);
     `cargo test --test cli_pipeline --test corpus --test cli_experiment --test invariants` all
     ok; `git diff 39ec658 -- tests/invariants.rs` empty.
  10. PASS — ADRs 0014/0015/0016 each have the correct `# ADR 00NN:` title, the
      `Accepted — 2026-08-23 (Milestone 2 / plan Phase 8)` status, all four sections, 0 heading
      violations and 0 `|---`; README has `### discover` and Run-directory rows for
      `dataset.jsonl`, `dataset.json`, `heuristics.json`, `discover.json`; `CLAUDE.md` Commands
      line lists `discover`; game-name awk loop 0 violations; game-concept sweep 0 hits (so the
      gate report's justification list is legitimately empty).
  11. PASS — `artifacts/benchmarks/mining-induction.md` + `.json` present and agreeing cell for
      cell; `.header.build_profile == "release"`, 6 `exhaustive_fits`, `discover_wall.run_id
      == bd760ff0ace48705`, `median_wall_s == 0.826`; the `.md` carries the 8-row header, 12
      engine/depth rows and the anchored `median_wall_s: 0.826` line; `cargo test --test
      mining_bench -- --nocapture` non-failing in debug printing 14 `[bench]` lines. The
      release wall and every candidate figure were independently re-measured by the supervisor
      (own run: 0.842 s; candidate table reproduced exactly).
  12. PASS — worker reports for steps 32, 33, 34 and 35 are committed and tracked; this ledger
      carries `done` Steps rows with commit SHAs for all four; the gate report ends
      `Overall verdict: PASS`.
- Also satisfied: the roadmap Phase 4 DoD frozen-files clause — `git diff 39ec658 --stat` over
  all eight frozen paths prints nothing.
- Milestone artifacts produced by this phase: `artifacts/reproducibility/discover-determinism.md`,
  `artifacts/benchmarks/mining-induction.md` + `.json`,
  `implementation-artifacts/strategy-discovery-m2-gate-report.md`, and the additive
  `tests/mining_bench.rs` exhaustive/holdout section. No library code changed in phase 4.
- REVISED (decomposer, post-amendment `6148998` — the human-approved config_hash-normalized
  serial comparison contract): step files `32-determinism` and `35-gate` re-projected from the
  amended brief (project DoD 7, D10 determinism bullet) and amended roadmap Phase 4 DoD; step
  ids and the dependency graph unchanged (`32` depends_on `[]`, `35` depends_on
  `[32-determinism, 34-mining-induction]`); `33`/`34` stay done/merged, their files untouched;
  `32` back to `pending` (was `blocked`). `32` changes: verify stage now asserts run-2 vs
  run-1 strict 16/16 files + stdout, and run-serial vs run-1 exactly 14/16 with the differing
  set exactly {discover.json, archive/entries.jsonl}, both byte-identical after replacing each
  run's own config_hash value with the placeholder `CONFIG_HASH`; per-run occurrence counts
  asserted (1x in discover.json, 4x in archive/entries.jsonl); measured values written to
  `target/m2/det/config-hash.txt` and drift-gated against the pins `cbcaffd750bb707b`
  (parallel) / `c3c8b37144634b33` (serial); the four verdict lines are the brief-D10 verbatim
  shapes (exactly 4 YES, 0 NO); the artifact gains a `### config_hash` block (both values, the
  sole-differing-field statement, the different-document / frozen-hash reason) with anchored
  acceptance greps; the `## Runs` table keeps 16 rows of REAL hashes (the two serial cells
  legitimately differ — 64-hex count stays 16); the verify script must throw `MISMATCH:` on
  any deviation and must NOT contain the contiguous strings `identical: YES` / `identical: NO`
  in its source (it is pasted verbatim into an artifact whose totals must be exactly 4 / 0 —
  the step-34 `median_wall_s` script-fence lesson applied in advance), so honest failure = no
  artifact + `status: fail`, never a NO verdict. `35` changes (context only; acceptance block
  untouched): Item 7 restated under the amended contract (discover-2 strict 16/16;
  discover-serial 14/16 outright + the two files byte-identical after `CONFIG_HASH`
  normalization with the hash-value pins as drift checks; both stdout comparisons whole-stdout
  modulo out-dir, `run_id=`/`evaluation=` included; artifact checks now exactly 4
  `identical: YES`, 0 `identical: NO`, both config_hash values present); Item 11 baseline
  facts pinned to the merged mining-induction artifacts (`.discover_wall.median_wall_s` 0.826,
  `.discover_wall.run_id` bd760ff0ace48705, the four-row `## Candidates` table 16/0, 24/1,
  38/5, 40/5 with 3-decimal rates) and the anchored `grep -c '^median_wall_s: '` check
  (unanchored counts 2 via the verbatim script fence). NO RETUNING confirmed again:
  `configs/tictactoe-discover.toml` and `README.md` untouched, so `32`'s drift gate and gate
  Items 3/4/7 stay pinned as written.

## Revisions

<!-- supervisor appends: phase | failed step | revision note | outcome -->

| phase | failed step | revision note | outcome |
| --- | --- | --- | --- |
| 1 | 01-deps | Two acceptance greps were mis-specified and unsatisfiable by honest work; corrected in flight (no decomposer round-trip, no code change). (a) `git diff 39ec658 -- Cargo.toml \| grep -ciE 'features\|default-features'` → `0` counts the WHOLE diff, and the unchanged context line `clap = { version = "4.6.6", features = ["derive"] }` sits within three lines of the alphabetical insertion point, so it reports `1` for a correct diff; restricted to changed lines it reports `0`. (b) `cargo tree -e normal -d \| grep -c ndarray` → `0` counts every line of the duplicate-subtree output; the PRE-EXISTING `either v1.17.0` duplicate (present at base `e77f55f`, verified in a base worktree) carries `ndarray v0.16.1` in its subtree, so it reports `4`; anchored to duplicate ROOTS (`grep -cE '^ndarray v'`) it reports `0`. | step file `strategy-discovery-m2-01-deps.md` acceptance + action 4 corrected and committed; step verified PASS on the substantive checks (exactly three plain `+` lines, no removals, `1 file changed, 3 insertions(+)`, single `ndarray v0.16.1`, no forbidden crates in the lock, check/fmt/clippy exit 0) and merged as `261076b` |
| 1 | 05-interpreter | Post-merge integration regression, not visible inside the step's own worktree. Making `heuristic-rules` playable flips `StrategyError::Unimplemented` assertions at FOUR sites; step 05's `files_in_scope` and context named only two (`tests/match_engine.rs`, `tests/cli_evaluate.rs`), and its action list ran only module-filtered lib tests (`cargo test --lib core::interpreter`, `--lib strategy::registry`), never a full `cargo test --lib`. Acceptance re-run by the supervisor on the integrated branch: `cargo test --lib` → expected `0 failed`, observed `FAILED. 341 passed; 2 failed` — `discovery::evaluate::tests::heuristic_rules_is_an_error` and `discovery::tournament::tests::heuristic_rules_under_test_is_an_error_not_a_loss`, both asserting `Err(CorpusError::Match(MatchError::Strategy(StrategyError::Unimplemented { .. })))` for a spec that now returns `Ok`. Root cause: incomplete step scope + module-filtered verification. Suggested fix: one new atomic step scoped to the two files, asserting the `Ok` shape without pinning seed-dependent win/draw/loss counts. | Sent to the decomposer; revision committed `765aab4`, which added step `10-stale-unimplemented-tests` (depends_on `05-interpreter`), added `10` to `09-gate`'s `depends_on`, and corrected the Phase 1 atomicity note to FOUR sites. Step 05 verified correct and left `done`/merged (`776d6f2`); no rollback. |
| 1 | phase-1 DoD | Roadmap Phase 1 DoD sub-clause "`cargo tree -p strategy-discovery -e normal \| grep -c "ndarray v"` reports a single ndarray version" was unsatisfiable by honest work: `grep -c` counts matching LINES, and `ndarray v0.16.1` appears on 5 lines of the tree (once per dependent path via `linfa`, `linfa-trees`, `ndarray-rand`, ...), so the command prints `5` while the property it names (one distinct ndarray version) holds. Amended in place to `cargo tree -p strategy-discovery -e normal \| grep -oE "ndarray v[0-9.]+" \| sort -u \| wc -l` reports `1` (`ndarray v0.16.1`) — counts DISTINCT versions. Gate-strengthening clarification; underlying constraint unchanged and satisfied. Ripple checked: brief Context/D-Constraints/project DoD 2 state the constraint as prose or exact-diff form (no defective command); Phase 4 DoD `git diff --stat` clause unrelated; step files `01-deps` (acceptance) and `09-gate` (Item 7) already use the distinct-version form — no other edits. | roadmap amended |
| 2 | phase-2 decomposition (D4 `tests/dataset.rs` step) | Amendment note from the decomposer carried two D4 defects. (1) STALE FACT, applied: the class-order example (brief line 199) listed the 17 set-kind classes as `free, orbit0.free .. line7.free, mine, theirs, ttt.*`, but live `PrimitiveFeatures::definitions()` (src/core/derived.rs) pushes `free, mine, theirs` first, then per-orbit, then per-line — example reordered in place to `free, mine, theirs, orbit0.free, orbit1.free, orbit2.free, line0.free .. line7.free, ttt.winning_cells, ttt.blocking_cells, ttt.fork_cells`. Illustrative only (the sentence mandates reading the live order); ripple checked: the example echoes nowhere else. (2) UNSATISFIABLE TEST CLAUSE, NOT applied: D4 Tests (c) `and has label == "ttt.winning_cells"` contradicts D4's own specificity-first label rule — empirically 27/321 exhaustive rows with `ttt.winning_cells > 0` legitimately label a smaller qualifying Supplied class (`unlabeled` = 5/627 ≈ 0.8%, far under the ~15% threshold, so the label rule is healthy). Relaxing the test assertion is gate-weakening → escalated needs-human with the proposed replacement (`and has a Supplied-tier label (label != "none")`); roadmap test name `*winning_cells_always_qualify*` covers only the kept qualify assertion, so no roadmap edit either way. Phase 2 decomposition blocked on the decision. | brief amended (example only); defect 2 of the note pending human decision |
| 2 | phase-2 decomposition (D4 `tests/dataset.rs` step) | Defect 2 of the amendment note (escalated needs-human in the prior row) APPROVED by the human: D4 Tests (Phase 2) item (c) clause `and has label == "ttt.winning_cells"` was unsatisfiable under D4's specificity-first label rule (27/321 exhaustive rows with `ttt.winning_cells > 0` legitimately label a smaller qualifying Supplied class); replaced in place with `and has a Supplied-tier label (label != "none")` — both parts guaranteed (a Supplied class qualifies and tier ranks first), so the clause stays satisfiable AND checkable. The label preference rule is UNCHANGED; the alternative (special-casing winning classes in the rule) was explicitly declined by the human. Ripple re-verified after edit: zero `label ==` echoes remain in brief/roadmap; roadmap Phase 2 DoD test name `*winning_cells_always_qualify*` covers only the kept qualify assertion — no roadmap edit; project DoD never references the clause; no phase 2 step files exist yet. Phase 2 decomposition unblocked. | brief amended (human-approved) |
| 2 | 19-cli-mine-e2e | Acceptance mis-specified and unsatisfiable by honest work, corrected in flight before launch (no decomposer round-trip, no code change). The `cargo test --test cli_report` clause read "`running 4 tests`, `4 passed` (the three pre-existing plus report_renders_dataset_and_mine_sections)", but `tests/cli_report.rs` on the integrated branch holds exactly TWO tests (`report_renders_a_run_directory`, `report_missing_input_is_rejected` — verified by `grep -c '#\[test\]'` = 2 and by `cargo test --test cli_report` printing `running 2 tests` in the 16-analyze-wiring worktree), so the step's own single added test yields THREE, never four. Clause rewritten to `running 3 tests`, `3 passed` with both pre-existing names spelled out. The `cli_analyze` clause (`running 5 tests`) was checked against the same branch (4 pre-existing after `16`) and is correct — untouched. | step file `strategy-discovery-m2-19-cli-mine-e2e.md` acceptance corrected and committed before the wave launched |
| 3 | phase-3 decomposition (D8 test (c) / project DoD 8) | Amendment note from the decomposer: the standalone `analyze` commands in D8 Tests (c) and project DoD 8 carried no `--mine-*` flags yet demanded `heuristics.json` byte-identity with the discover run's — unsatisfiable by honest work, since D6 deliberately routes mine parameters through CLI flags each defaulting to `MineParams::default()` (`depths = [8]`; verified live in `src/cli/mod.rs` flags and `src/io/schema.rs` defaults), nothing reads mine parameters back from the run directory, and D7 pins the fixtures at `[mine] depths = [6]` (discover-small) and `[6, 8, 12, 0]` (tictactoe-discover) — so the flagless command mines depth 8 (`mined-d8-l1`, `params.depths = [8]`) and can never byte-match; making the fixtures default-valued instead would contradict D7 and D8 (a) (`params.max_depth == 6`). Gate-clarifying amendment applied in place, no human round-trip (byte-identity retained and the flag-vs-`[mine]`-table plumbing equivalence now additionally exercised; classification per the phase-1 ndarray-grep precedent, not the gate-weakening D4-label one): D8 test (c) command gains `--mine-depths 6`; both DoD 8 standalone `analyze` commands gain `--mine-depths 6,8,12,0`; both sites state the rule that the `--mine-*` flags must match the config's `[mine]` table so Phase 4's permitted `[mine]` retuning (its DoD records the final depths) substitutes values instead of re-breaking the item; roadmap Phase 3 DoD summary phrase gains the same matching-flags rider. Ripple checked: D6 correct as designed; Phase 2 DoD already used explicit `--mine-depths 4,6`; roadmap Phase 4 "DoD 3, 4, 6, 7, 8, 9, 11 commands pass verbatim" inherits the fix through DoD 8 itself; grep sweep over brief/roadmap/ledger shows no other flagless echoes; no phase 3 step files exist yet; completed phase 1-2 steps ran against other clauses and stay untouched. | brief + roadmap amended |
| 3 | 30-readme | Worker-level factual miss caught by supervisor review, NOT by the step's acceptance (which passed in full). Step `30` action 3e says "append `[mine]` and `[discover]` tables showing every key with its default"; the worker wrote two wrong example values in the README `### Experiment config` `[discover]` table: `roster = "ttt-benchmark-v1"` (the key is `Option<PathBuf>` — a roster TOML path resolved against the experiment file's directory, per `DiscoverSection::roster` in `src/discovery/experiment.rs`; a roster ID there is a file-not-found at run time) and `max_plies = 0  # default unlimited` (the key is `Option<usize>`; `0` parses as `Some(0)`, a zero-ply cap, while unlimited means omitting the key). Every other documented default was verified correct against `DiscoverSection`/`default_discover_games`/`default_discover_archive` and `MineParams`. Too small to justify re-running the step (its capture step is a minutes-long full-scale `discover`), and outside the `### discover` stdout block that gate item 13 diffs, so corrected by the supervisor in an integration commit: `roster = "roster-tiny.toml"  # path, relative to this file; default the game's built-in roster` and `max_plies = 30  # optional ply cap; omit for unlimited`. README Markdown checkers re-run clean after the edit (0 heading violations, 0 `\|---`, 0 `not implemented`, 4 `strategy=mined-`). Lesson for later phases: acceptance greps on documentation prove presence, never accuracy — a doc step's example values must be diffed against the live serde defaults. | step `30` verified PASS and left `done`/merged (`74960a7`); supervisor integration commit `54c6cf7` |
| 4 | phase-4 decomposition (D10 benchmark-evidence step) | Amendment note from the decomposer: roadmap Phase 4 Scope enumerated only the artifact/config/report deliverables and omitted any measurement vehicle for the exhaustive/holdout numbers brief D10 demands (dataset build time for the exhaustive 5478 records; per-engine fit time / rules / leaves / train + holdout soundness at depths 4/8/0 with `holdout_fraction = 0.2`, seed 0). Verified live: `tests/mining_bench.rs` benches only the generate-small corpus with `holdout_fraction: 0.0` and no exhaustive section, and no CLI path can substitute — `run_stages` (`src/cli/pipeline.rs`) unconditionally `require_file`s `RUN_FILE` before analyze while an exhaustive annotation dir holds only `annotations.jsonl` + `annotate.json`. Gate-enabling/strengthening (no DoD weakened), applied in place without a human round-trip: `tests/mining_bench.rs` added to the Phase 4 Scope list under an ADDITIVE-only constraint — existing `[bench]` lines and assertions byte-unchanged, the single `#[test]` stays the only test, appended exhaustive dataset-build timing (`annotate_exhaustive` + `load_annotations` + timed `build_dataset`, `annotations_mode: "exhaustive"`) plus per-engine fits at depths 4/8/0, holdout 0.2, seed 0, printing fit ms / rules / leaves / train + holdout soundness, new lines distinguishable from the corpus-bench lines, still non-failing on timing, estimated +4-7 s debug (warm wall 46 s vs the ≤ 80 s budget). Ripple checked: brief D10 and D5 Tests unedited (D10 becomes satisfiable); project DoD 11 unedited and stays true (additive lines, still non-failing, still `[bench]`-printing); roadmap Phase 2 DoD bench clause stays true; Phase 4 DoD frozen-files `git diff --stat` list does not name the bench; the phase-3 post-integration count (`--test mining_bench` 1) is preserved by the single-test constraint; no committed step file embeds the Phase 4 scope list. | roadmap amended |
| 4 | 32-determinism (in-flight step-file fix) | Step file action 3 invoked `target/m2/det/determinism.ps1` while action 2 wrote the script to `target/m2/determinism.ps1` (contradictory paths, one level apart). Corrected in flight before launch — no decomposer round-trip, no behavior change: action 3 now names `target/m2/determinism.ps1`. | step file `strategy-discovery-m2-32-determinism.md` corrected and committed `b16b10f` before the wave launched |
| 4 | 34-mining-induction | Step verified PASS on substance and merged (`a55885f`); three artifact defects that EVERY acceptance grep passed over, caught by supervisor review (phase-3 `30-readme` precedent). (a) FACTUAL: `run_id: bd760ff0ace48705 cells=48 games=1920 positions=14887 out=target/m2/mi/run-1` in both `.md` and `.json` — the script's `$RunIdLine -replace '^run_id=',''` stripped only the prefix and kept the whole stdout line, so the recorded `run_id` was not a run id (and leaked a scratch path into evidence); corrected to `bd760ff0ace48705`. (b) FACTUAL: the `## Commands` ```powershell fence was NOT the script that ran — its ~50-line rendering section was replaced by the comment "(rendering of the six .md sections omitted here...)", while the header's `command` field claimed "reproduced verbatim in § Commands"; the true 298-line `target/m2/mining-induction.ps1` is now pasted verbatim. (c) STYLE: rendered floats carried full binary precision (`0.964125560538117`, `0.97031863814928`) against the brief's `floats {:.3} in rendered text` constraint and the `tournament-throughput.md` precedent; both files re-emitted rounded to 3 dp so `.md` still equals `.json` cell for cell. All five data tables were re-rendered FROM the corrected `.json` (single source of truth). Consequential acceptance correction: `grep -c 'median_wall_s: ' → 1` became unsatisfiable once the verbatim script (which contains `$Md += "median_wall_s: ..."`) was pasted — the old grep was only satisfiable BY the redaction; anchored to `grep -c '^median_wall_s: ' → 1` in the step file. Measurements themselves re-verified live by the supervisor, not merely re-grepped: an independent release `discover` run reproduced wall 0.842 s (artifact median 0.826 s) and byte-matched every pinned stdout line; an independent read of that run's `discover.json` + `heuristics.json` reproduced the candidate table exactly (16/16/0, 24/25/1, 38/42/5, 40/44/5; loss 0.050/0.000/0.000/0.000; agreement 0.970/0.979/0.990/0.990; novelty 1.000/0.039/0.031/0.004); the cart exhaustive-fit rows (10/10 0.974/0.952, 26/28 0.986/0.960, 35/38 0.992/0.960) match the supervisor's own debug bench run of step 33. | step `34` `done`/merged (`a55885f`); supervisor integration commit `3e746ce` |
| 4 | 32-determinism | BRIEF/ROADMAP DEFECT — phase blocked, escalated to the human via the planner. Step 32 ran all three full-scale `discover` runs honestly and returned `status: fail` rather than write a contrived `identical: YES`; the supervisor reproduced and NARROWED the finding. Measured at base `38f2e40` (debug): run-2 vs run-1 = all 16 files byte-identical, stdout identical modulo `out=` (the repeat-run half of DoD 7 holds exactly as written). run-serial vs run-1 = 14/16 files byte-identical, stdout identical modulo `out=` (including `run_id=bd760ff0ace48705` and `evaluation=78bcde7c9113b250`), and all four archive `entry_id`s identical; the ONLY differing bytes anywhere are the `config_hash` value — `cbcaffd750bb707b` (parallel) vs `c3c8b37144634b33` (serial) — once in `discover.json`, four times in `archive/entries.jsonl`. Replacing both hash strings with a placeholder makes both files compare byte-identical (supervisor-verified by re-hashing). Root cause: `config_hash` hashes the loaded `ExperimentConfig`, and `serial` is `#[serde(default)] pub bool` on both `GenerateSection` and `DiscoverSection` (`default` affects deserialization only — the field always serializes), so DoD 7's own prescribed serial construction necessarily changes the hashed input. Not nondeterminism: execution mode changed nothing, the input document did. Every escape route is closed by the brief itself — `config_hash` is listed under Frozen, `discover` has no `--serial` flag (only `generate`/`play`/`evaluate` do), `RAYON_NUM_THREADS=1` does not select the `RayonMatchEngine::serial` path that `src/discovery/tournament.rs` chooses on `config.serial`, and an explicit `serial = false` line in the parallel copy still differs in VALUE. Defective text: brief project DoD 7 ("a third run from a copy of the config ... is identical too"), brief D10 determinism bullet, roadmap Phase 4 DoD determinism bullet ("≥ 3 `identical: YES` lines and 0 `identical: NO`"). Amendment note sent to the planner proposing that the serial comparison be restated as identity after normalizing `config_hash` (repeat-run byte-identity kept unweakened, and the artifact additionally obliged to prove the difference is confined to that one field). | planner verified every fact against live source, classified the change as GATE-WEAKENING by analogy with the phase-2 D4 label relaxation (not with the phase-1 ndarray-grep / phase-3 `--mine-*` fixes), applied and committed NOTHING, and returned `needs-human`. Phase 4 blocked at steps `32` and `35`; brief, roadmap and step files unchanged |
| 4 | 32-determinism (resolution) | Human APPROVED the amendment (declining both alternatives: no `--serial` flag on `discover`, `config_hash` stays frozen; repeat-run 16/16 byte-identity unweakened). Planner applied it in place at all three sites and committed `6148998`; decomposer re-projected the amended contract into `strategy-discovery-m2-32-determinism.md` and `strategy-discovery-m2-35-gate.md` Item 7 and committed `0cfb937`, additionally fixing a latent decomposition bug it found — the old step's verify script would have embedded literal `identical: YES`/`identical: NO` strings into the verbatim `## Commands` fence, breaking the artifact's own 4/0 count greps (the same class of collision the supervisor hit on step `34`'s `median_wall_s` grep); the revised script builds the verdict suffix by concatenation and throws `MISMATCH:` instead of writing NO verdicts. Step `32` then re-ran IN-TREE (lone ready step, warm debug build, clean tree, ledger committed) and passed. Supervisor verification went past the 18 acceptance greps to ground truth: all three run dirs re-hashed independently with `sha256sum` — every one of the 48 hashes in the artifact's `## Runs` table matches the measured value; run-2 vs run-1 is 16/16 equal; the run-serial difference set is EXACTLY {`discover.json`, `archive/entries.jsonl`}; normalizing each run's own `config_hash` value makes both files compare equal; occurrence counts are 1 and 4 in both runs; `config-hash.txt` reads `parallel=cbcaffd750bb707b` / `serial=c3c8b37144634b33` matching the pins; all three stdout captures are identical after out-dir normalization and match the pinned ten-line block byte-for-byte. The pasted `## Commands` script was diffed against the script that actually ran and IS verbatim — the step-`34` non-verbatim defect did not recur. | brief + roadmap amended `6148998`; step files revised `0cfb937`; step `32` `done` at `4f9a9d8` |
| 4 | 32-determinism | HUMAN DECISION on the prior row's escalation: APPROVED verbatim — the planner's config_hash-normalized serial comparison contract applied exactly as proposed, in place at all three sites. (1) Brief project DoD 7: run-2 clause byte-unchanged (16/16 files SHA-256-identical, stdout modulo `out=` — repeat-run identity unweakened); serial clause replaced — run-serial (`target/m2/discover-serial`, now named in the brief, making the roadmap's "as named in the brief" parenthetical accurate) vs run-1 = same 16-file set, 14/16 byte-identical outright, `discover.json` + `archive/entries.jsonl` byte-identical once the `config_hash` value is replaced by one placeholder string in both copies, stdout identical modulo `out=` (`run_id` and `evaluation` lines included); artifact obliged to state both hash values, name `config_hash` as the sole differing field, and explain the different-document / frozen-hash reason. (2) Brief D10 determinism bullet: the same contract spelled out for the `## Runs` table with the four verdict-line shapes prescribed verbatim — exactly 4 `identical: YES`, 0 `identical: NO` anywhere in the artifact — and the measured hash pins recorded (parallel `cbcaffd750bb707b`, serial `c3c8b37144634b33`; profile-independent since `config_hash` hashes only the loaded config document). (3) Roadmap Phase 4 DoD determinism bullet: `≥ 3 identical: YES` became exactly 4 with the full serial-comparison qualification. Alternatives explicitly DECLINED by the human: no `--serial` flag on `discover` (real new scope); `config_hash` stays frozen (unfreezing would be a provenance defect). Ripple re-checked after the edits: Constraints "Determinism" paragraph needs NO edit — it is scoped "for the same inputs" and the serial copy is a different input document, a fact the amended DoD 7 now states explicitly, so the two texts agree; Frozen list keeps `config_hash`; the other `config_hash` mentions (io API listing, D8 manifest field, D8 test (a), D9 field, project DoD 4) are presence/shape clauses, untouched; roadmap Phase 4 "DoD 3, 4, 6, 7, 8, 9, 11 commands pass verbatim" inherits the fix through DoD 7 itself; zero stale `is identical too` / `≥ 3` echoes remain in brief or roadmap. Step files deliberately NOT touched: `strategy-discovery-m2-32-determinism.md` and `strategy-discovery-m2-35-gate.md` (Item 7) go through the decomposer's revise operation next, per the Phase 4 notes RESUMPTION plan. | brief + roadmap amended (human-approved) |
