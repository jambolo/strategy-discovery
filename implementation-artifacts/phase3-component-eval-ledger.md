# phase3-component-eval — Ledger

Single source of truth for execution state. Sections are owned by different skills —
the planner seeds Plan + Phases; the decomposer fills Steps per phase; the supervisor
updates Steps and appends Revisions.

## Plan

- plan-name: phase3-component-eval
- current-phase: 4 (all 3 phases complete)
- working-branch: feature/phase-3-component-evaluation
- starting-commit: b8aa59a8cd6c29c1e251198a81c01b39e83ec815
- default-branch: develop
- artifacts-dir: implementation-artifacts/

## Phases

| Phase | Status  | Notes |
| ----: | ------- | ----- |
| 1     | complete | Tic-tac-toe domain (Phase 2 gap-fill) — `src/games/tictactoe/`; gate report verdict PASS; 118 lib tests |
| 2     | complete | Feasibility spikes — 4 crates under `spikes/`, 11 evidence files under `artifacts/` (re-measured serially at e59c6de); gate verdict PASS (24/24); findings: game-player exposes no value/best-set (solver fallback cross-checks 0 disagreements at depth 9 and 10), serial==parallel byte-identical, polars 0.55 needs rustc 1.95 (used 0.53.0), linfa-trees traversable via public API, smartcore only via serde |
| 3     | complete | `docs/component-evaluation.md` (3 matrices, 7 candidates, all final_scores recomputed), `docs/adr/0001`-`0007`, README pointer; gate verdict PASS (22/22); cargo test 118 lib + 4 integration, fmt, clippy green; root Cargo.toml unchanged from b8aa59a |

## Steps

<!-- decomposer fills per phase: id | phase | status | files | commit -->

| id | phase | status | files | commit |
| --- | ---: | --- | --- | --- |
| 01-board-rules | 1 | done | src/games/mod.rs, src/games/tictactoe/mod.rs, src/games/tictactoe/board.rs, src/games/tictactoe/rules.rs, src/games/tictactoe/primitives.rs, src/games/tictactoe/features.rs, src/games/tictactoe/canonical.rs, src/games/tictactoe/engine.rs, implementation-artifacts/phase3-component-eval-01-board-rules-report.md | 81022314fbd933e83956a1028897947986717f1b |
| 02-primitives | 1 | done | src/games/tictactoe/primitives.rs, implementation-artifacts/phase3-component-eval-02-primitives-report.md | e94585f4b1b135995fa988f33936042cdee94f8a |
| 03-features | 1 | done | src/games/tictactoe/features.rs, implementation-artifacts/phase3-component-eval-03-features-report.md | d1abc93302c62487bcaa86c388af91cf64540ba7 |
| 04-engine | 1 | done | src/games/tictactoe/engine.rs, implementation-artifacts/phase3-component-eval-04-engine-report.md | 44de0aedc17ad0757d0113a65f447593bad7f3f5 |
| 05-bench | 1 | done | tests/ttt_bench.rs, implementation-artifacts/phase3-component-eval-05-bench-report.md | 763bf58fb26013f72a108784de0b0dabe35198b1 |
| 06-canonical | 1 | done | src/games/tictactoe/canonical.rs, implementation-artifacts/phase3-component-eval-06-canonical-report.md | 0f3ad7212fac14f28229d7139724f30038d7fa0f |
| 07-gate | 1 | done | implementation-artifacts/phase3-component-eval-07-gate-report.md | c225d216258506a8ebd7ed79d40c20374191a1ad |
| 08-annotate-spike | 2 | done | spikes/annotate-spike/Cargo.toml, spikes/annotate-spike/Cargo.lock, spikes/annotate-spike/src/main.rs, artifacts/benchmarks/annotation-per-position.md, artifacts/benchmarks/annotation-per-position.json, artifacts/reproducibility/annotation-determinism.md, implementation-artifacts/phase3-component-eval-08-annotate-spike-report.md | 115b2ea950d45f620b18aa7d5f1944aaa3fb149a |
| 09-throughput-spike | 2 | done | spikes/throughput-spike/Cargo.toml, spikes/throughput-spike/Cargo.lock, spikes/throughput-spike/src/main.rs, artifacts/benchmarks/selfplay-throughput.md, artifacts/benchmarks/selfplay-throughput.json, artifacts/reproducibility/selfplay-serial-vs-parallel.md, implementation-artifacts/phase3-component-eval-09-throughput-spike-report.md | d75613d3acb17ddcef0f6c731c4c20d92e441497 |
| 10-record-format-spike | 2 | done | spikes/record-format-spike/Cargo.toml, spikes/record-format-spike/Cargo.lock, spikes/record-format-spike/measure-compile.ps1, spikes/record-format-spike/src/{main,aggregate,evidence,jsonl,polars_support,record,selfplay,versions}.rs, artifacts/benchmarks/record-format.md, artifacts/benchmarks/record-format.json, artifacts/benchmarks/record-format-compile.json, implementation-artifacts/phase3-component-eval-10-record-format-spike-report.md | 4f98df4ecaaf42c883e839e651855ad747bc5793 |
| 11-rule-induction-spike | 2 | done | spikes/rule-induction-spike/Cargo.toml, spikes/rule-induction-spike/Cargo.lock, spikes/rule-induction-spike/survey.json, spikes/rule-induction-spike/src/{main,dataset,decision_list,header,models,solver,survey}.rs, artifacts/benchmarks/rule-induction.md, artifacts/benchmarks/rule-induction.json, implementation-artifacts/phase3-component-eval-11-rule-induction-spike-report.md | 637b6f803fa4af4b0a60554a8ebfd1415e7dc787 |
| 12-measure | 2 | done | artifacts/benchmarks/{annotation-per-position,selfplay-throughput,record-format,rule-induction}.{md,json}, artifacts/benchmarks/record-format-compile.json, artifacts/reproducibility/{annotation-determinism,selfplay-serial-vs-parallel}.md (regenerated on quiet machine, headers git_commit=e59c6de486ffca5df18f7933aec816aa04c10c06), implementation-artifacts/phase3-component-eval-12-measure-report.md | d530c0125458d7e6e02eeb3825055aca7f5321ff |
| 13-gate | 2 | done | implementation-artifacts/phase3-component-eval-13-gate-report.md | e0a6dcad11f3e58abb9ac213b8ce614e48b6d7e6 |
| 14-matrix | 3 | done | docs/component-evaluation.md, implementation-artifacts/phase3-component-eval-14-matrix-report.md | 766d8075761caa68c67bed3c84230fb43cf171dc |
| 15-adr-settled | 3 | done | docs/adr/0001-game-player-minimax-adapter.md, docs/adr/0002-self-play-experiment-runner.md, docs/adr/0003-json-toml-metadata-persistence.md, docs/adr/0004-summary-aggregator.md, README.md, implementation-artifacts/phase3-component-eval-15-adr-settled-report.md | 970bc4bbd74ecf634f00f931232db8d7aa159ccc |
| 16-adr-selected | 3 | done | docs/adr/0005-dataframe-layer.md, docs/adr/0006-rule-induction.md, docs/adr/0007-corpus-format.md, implementation-artifacts/phase3-component-eval-16-adr-selected-report.md | 27857cb97d2bf93ca22c59d9b9100885dd8d09a6 |
| 17-gate | 3 | done | implementation-artifacts/phase3-component-eval-17-gate-report.md | efd68cf859df8ef1225d9e82f76566a77bdbdb28 |

Dependency graph (phase 1): 01 → {02, 03, 04, 05} in parallel; 06 after 02; 07 (gate) after all. Step 01 creates one-line stubs for primitives/features/canonical/engine so 02/03/04 each own exactly one file.

Dependency graph (phase 2): {08, 09, 10, 11} in parallel (disjoint spike dirs + disjoint artifact files; 11 embeds its own copy of 08's solver); 12 (serial quiet-machine re-measure, rewrites all artifacts) after all four; 13 (gate) after 12.

Dependency graph (phase 3): {14, 15, 16} in parallel (disjoint files; decisions and final scores fixed identically in all three step files, so matrix == ADRs by construction); 17 (gate) after all three. Scores were fixed by the decomposer from the Phase 2 artifacts; the worker only formats and cites.

## Revisions

<!-- supervisor appends: phase | failed step | revision note | outcome -->

| phase | failed step | revision note | outcome |
| --- | --- | --- | --- |
| 1 | 03-features | acceptance passed; step `context` fixture 1 (`"XX..O...."`) claimed "X to move" with mover-relative values, but `Board::parse` (step 01 rule: X=2,O=1 → O to move) yields O to move. Root cause: acceptance/context mis-specified. Fix: corrected in flight (option b) — fixture values rewritten relative to the real mover (mine=0, theirs=1, winning={}, blocking={2}, win_available=false); step file updated, no decomposer round-trip. | merged as-is; worker's honest values verified by hand |
| 2 | 10-record-format-spike | acceptance `cargo run --release --manifest-path spikes/record-format-spike/Cargo.toml` expected exit 0; observed: polars 0.55.2 does not compile on stable rustc 1.94.1 — `polars-io 0.55.2` unconditionally enables `polars-utils/sysinfo` → `sysinfo ^0.39`, every 0.39.x has `rust-version = "1.95"` and uses `cfg_select!` (E0658). Supervisor verified via registry Cargo.toml + crates.io deps (`polars-utils 0.53.0` → `sysinfo ^0.37`, MSRV 1.88). Root cause: step context pinned a crate version incompatible with the project toolchain (acceptance unsatisfiable by honest work). Fix: corrected in flight (option b) — step file now mandates polars 0.53 and a `## Toolchain note` section recording the MSRV finding; worker's worktree soft-reset to BASE keeping its code; same worker resumed. | retry passed with polars 0.53.0 (+ dtype-i128 for u64 seeds); merged |
| 3 | 16-adr-selected | acceptance passed on content; 7 of the step's `grep -cF '- Selected default: ...'` commands fail mechanically (GNU grep parses a fixed-string pattern beginning with `-` as an option: `grep: invalid option -- ' '`). Root cause: acceptance mis-specified (missing `-e`). Fix: corrected in flight (option b) — `grep -cF -e '- ...'` in step 16 acceptance and step 17 gate checks 9-11; no content change, no decomposer round-trip. | merged as-is; supervisor re-ran all 11 checks with `-e`, all pass |
