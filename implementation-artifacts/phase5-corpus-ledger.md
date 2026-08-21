# phase5-corpus — Ledger

Single source of truth for execution state. Sections are owned by different skills —
the planner seeds Plan + Phases; the decomposer fills Steps per phase; the supervisor
updates Steps and appends Revisions.

## Plan

- plan-name: phase5-corpus
- current-phase: complete (all 3 phases done; project gate PASS at ed15e33)
- working-branch: feature/phase-5-corpus
- starting-commit: e4e0a7ea7975301531851c5ded86257ff72d5373
- default-branch: develop
- artifacts-dir: implementation-artifacts/

## Phases

| Phase | Status  | Notes |
| ----: | ------- | ----- |
| 1     | done    | Gate PASS (implementation-artifacts/phase5-corpus-09-gate-report.md): 240 lib tests, fmt/clippy/doc clean. Foundations: io schema/jsonl/hash/replay, ConstantEvaluator, openings in match engine, exhaustive solver, GameBundle, GenerateConfig + cell enumeration, ttt game_bundle |
| 2     | done    | Gate PASS (implementation-artifacts/phase5-corpus-19-gate-report.md): 253 lib + corpus 10 + annotate 3 + cli_pipeline 6 + corpus_bench 1 tests, debug wall 31-32 s, fmt/clippy/doc clean. Stages generate/annotate/analyze + JsonlWriter + CLI (run_from) + configs/tictactoe-default.toml + 3 fixtures. Release bench 980 games: serial ~750 games/s, parallel ~3.5-3.7k games/s. Diversity small fixture seeds 1/2/3: coverage 0.318/0.298/0.301, decisive 0.611/0.604/0.590, distinct 0.813/0.826/0.854 |
| 3     | done    | Project gate PASS (implementation-artifacts/phase5-corpus-gate-report.md, step 26-gate ed15e33): steps 20-26 done, zero failures, zero revisions. Supervisor re-ran the full DoD: 253 lib + annotate 3 + cli_pipeline 6 + corpus 10 + corpus_bench 1 + match_bench 2 + match_engine 9 + minimax_tactics 7 + smoke 2 + ttt_bench 2, debug wall 30.6 s, fmt/clippy/doc clean, `git diff e4e0a7e` on protected paths empty. Gate B evidence (release, 9800 games): serial 736.0 / threads-4 2381.9 / parallel 4651.8 games/s, 75004 positions, positions.jsonl 28003228 B, annotate 0.143 s (2882 states, 0 disagreements), analyze 0.169 s, exhaustive 0.053 s; SHA-256 identity YES for same-seed ×2, serial/threads-4/default, annotate ×2, exhaustive ×2, analyze ×2; diversity seeds 1-3 PASS vs 0.5/0.2/0.5 (coverage 0.601/0.612/0.607, decisive 0.522/0.520/0.511, distinct 0.591/0.588/0.626), draws control FAIL exit 2; docs/component-evaluation.md § Gate B verdict PASS; ADR 0009; README Usage. Scale finding recorded (not a DoD break): distinct_game_fraction 0.329 at 9800 games, thresholds and default config unchanged |

## Steps

<!-- decomposer fills per phase: id | phase | status | files | commit -->

| id | phase | status | files | commit |
| --- | --- | --- | --- | --- |
| 01-io-base | 1 | done | src/io/mod.rs, src/io/hash.rs, src/io/jsonl.rs, implementation-artifacts/phase5-corpus-01-io-base-report.md | d69506b |
| 02-solver | 1 | done | src/discovery/mod.rs, src/discovery/solver.rs, src/discovery/bundle.rs, src/discovery/config.rs, implementation-artifacts/phase5-corpus-02-solver-report.md | 777161e |
| 03-constant-evaluator | 1 | done | src/strategy/engine.rs, src/strategy/mod.rs, implementation-artifacts/phase5-corpus-03-constant-evaluator-report.md | 316edfe |
| 04-openings | 1 | done | src/discovery/match_engine.rs, implementation-artifacts/phase5-corpus-04-openings-report.md | 86bf211 |
| 05-io-schema-replay | 1 | done | src/io/schema.rs, src/io/replay.rs, src/io/mod.rs, implementation-artifacts/phase5-corpus-05-io-schema-replay-report.md | 925eeb0 |
| 06-config-types | 1 | done | src/discovery/config.rs, implementation-artifacts/phase5-corpus-06-config-types-report.md | 6440fa9 |
| 07-bundle | 1 | done | src/discovery/bundle.rs, src/games/tictactoe/corpus.rs, src/games/tictactoe/mod.rs, implementation-artifacts/phase5-corpus-07-bundle-report.md | 4245bab |
| 08-config-resolve | 1 | done | src/discovery/config.rs, implementation-artifacts/phase5-corpus-08-config-resolve-report.md | fd305e6 |
| 09-gate | 1 | done | implementation-artifacts/phase5-corpus-09-gate-report.md | 1d0548c |
| 10-scaffold | 2 | done | src/discovery/mod.rs, src/discovery/corpus.rs, src/discovery/annotate.rs, src/discovery/summary.rs, src/discovery/config.rs, configs/tictactoe-default.toml, tests/fixtures/generate-small.toml, tests/fixtures/generate-small-explicit.toml, tests/fixtures/generate-draws.toml, implementation-artifacts/phase5-corpus-10-scaffold-report.md | 722252a |
| 11-corpus | 2 | done | src/io/jsonl.rs, src/io/mod.rs, src/discovery/corpus.rs, implementation-artifacts/phase5-corpus-11-corpus-report.md | cf0b9f4 |
| 12-annotate | 2 | done | src/discovery/annotate.rs, implementation-artifacts/phase5-corpus-12-annotate-report.md | bd4c20f |
| 13-summary | 2 | done | src/discovery/summary.rs, implementation-artifacts/phase5-corpus-13-summary-report.md | 011f73f |
| 14-cli | 2 | done | src/cli/mod.rs, src/discovery/mod.rs, tests/smoke.rs, implementation-artifacts/phase5-corpus-14-cli-report.md | 8912586 |
| 15-tests-corpus | 2 | done | tests/corpus.rs, implementation-artifacts/phase5-corpus-15-tests-corpus-report.md | 8fbd28d |
| 16-tests-annotate | 2 | done | tests/annotate.rs, implementation-artifacts/phase5-corpus-16-tests-annotate-report.md | d136827 |
| 17-tests-cli | 2 | done | tests/cli_pipeline.rs, implementation-artifacts/phase5-corpus-17-tests-cli-report.md | 1d2c2b9 |
| 18-bench | 2 | done | tests/corpus_bench.rs, implementation-artifacts/phase5-corpus-18-bench-report.md | f9ebc38 |
| 19-gate | 2 | done | implementation-artifacts/phase5-corpus-19-gate-report.md | bdb3e7b |
| 20-bench | 3 | done | artifacts/benchmarks/corpus-generation.md, artifacts/benchmarks/corpus-generation.json, implementation-artifacts/phase5-corpus-20-bench-report.md | 21abdc5 |
| 21-determinism | 3 | done | artifacts/reproducibility/corpus-determinism.md, implementation-artifacts/phase5-corpus-21-determinism-report.md | 1e78f74 |
| 22-diversity | 3 | done | artifacts/reproducibility/corpus-diversity.md, implementation-artifacts/phase5-corpus-22-diversity-report.md | bc5339b |
| 23-adr | 3 | done | docs/adr/0009-corpus-record-schema.md, implementation-artifacts/phase5-corpus-23-adr-report.md | d880247 |
| 24-readme | 3 | done | README.md, implementation-artifacts/phase5-corpus-24-readme-report.md | 435cff0 |
| 25-gate-b | 3 | done | docs/component-evaluation.md, implementation-artifacts/phase5-corpus-25-gate-b-report.md | 6c9a01b |
| 26-gate | 3 | done | implementation-artifacts/phase5-corpus-gate-report.md, implementation-artifacts/phase5-corpus-26-gate-report.md | ed15e33 |

Phase 3 dependency graph: 20-bench first and ALONE (lone in-tree step: the timing evidence needs a quiet
machine and the warm release build, so every other Phase 3 step depends on it — for that reason only).
Then {21-determinism, 22-diversity, 23-adr, 24-readme} in parallel worktrees (21/22/24 each build
release in a fresh worktree, ~1-2 min; 23 runs no cargo). Then 25-gate-b (cites numbers from the three
artifacts and the ADR path). Then 26-gate (project gate, verification only; may run in-tree as the
lone final step). Scopes: `artifacts/benchmarks/**` by 20; `artifacts/reproducibility/corpus-determinism.md`
by 21; `corpus-diversity.md` by 22; `docs/adr/0009-*` by 23; `README.md` by 24;
`docs/component-evaluation.md` by 25; the project gate report by 26 only. Couplings: 24 links
`docs/adr/0009-corpus-record-schema.md` by its brief-fixed path without depending on 23 (26 verifies the
link target exists) and rewords the README's "Gate A evidence" line to "Gate A, Gate B" ahead of 25
landing that section.

Decomposer notes for Phase 3 (measured at `7628977`, release, in-tree; the measurement scripts are
embedded verbatim in steps 20-22 and were dry-run end to end, as were all seven acceptance chains):

- 9800-game run (`configs/tictactoe-default.toml` with `games_per_cell = 200`, seed 20260821):
  `run_id=dea7db96340e923d cells=49 games=9800 positions=75004`; serial ≈ 13.5 s (≈ 725 games/s),
  `--threads 4` ≈ 4.1 s (≈ 2400 games/s), global pool ≈ 2.1 s (≈ 4600 games/s); annotate 0.14 s
  (`annotated=2882 terminal=0 disagreements=0`), analyze 0.17 s, exhaustive 0.06 s; games.jsonl
  6161570 B, positions.jsonl 28003228 B, run.json 13645 B, annotations.jsonl 829583 B (exhaustive
  1570396 B). SHA-256 identity held for same seed ×2, serial / threads-4 / default, annotate ×2,
  exhaustive ×2, analyze ×2 — the hashes are pinned in step 21's acceptance.
- FINDING (not a DoD break; recorded verbatim in the artifacts and as the Gate B scale note): at 9800
  games `analyze` reports `diversity_pass=false` — `distinct_game_fraction` 3225 / 9800 = 0.329 < 0.5
  while coverage 0.935 (715 / 765) and decisive 0.510 pass. The brief scopes the Gate B diversity claim
  to `games_per_cell = 20`, which passes at seeds 1 / 2 / 3 (distinct 0.5908 / 0.5878 / 0.6255,
  coverage 0.6013 / 0.6118 / 0.6065, decisive 0.5224 / 0.5204 / 0.5112; run_ids a6307a3549cd8da3 /
  dfb3fa21f15dc3a0 / 4f30afd1b5711779) and at the checked-in seed 20260821 (run_id 74bba44805e514a7:
  distinct 0.618, coverage 0.599, decisive 0.514, `--strict` exit 0). Draws fixture: run_id
  933a94e742497e81, `--strict` exit 2, three failures. `--min-distinct 0.3` passes at 9800 games. The
  default thresholds and `configs/tictactoe-default.toml` are untouched.
- Evidence scripts write their configs and run outputs under the gitignored `target/gate-b/`; the
  committed artifacts embed the scripts verbatim (no absolute paths — `$Root` resolves from
  `$PSScriptRoot`). Hash tool `Get-FileHash -Algorithm SHA256`; timing via
  `System.Diagnostics.Stopwatch`, median of three runs.
- Working-tree text files are CRLF (`core.autocrlf = true`); acceptance greps avoid `$` anchors and
  last-line checks use `tail -n 1 | tr -d '\r'`. `pwsh` (PowerShell 7) is on PATH and is the JSON
  validator (`ConvertFrom-Json`); no `python`, `jq` or `bc`.

Supervisor notes for Phase 3:

- Step 21's action 6 said the SHA-256 table has "(22 rows)"; the script prints 28 (the two annotate
  copies list their 3 copied inputs plus 3 outputs). The worker transcribed all 28 and flagged the
  miscount in its report; no re-run, acceptance unaffected.
- Step 23's ADR claimed `seeds_follow_the_documented_formulas` pins all three seed formulas; that test
  covers the `cell_seed` → `game_seed` chain only, `opening_seed` is pinned by
  `opening_seed_reference_values` in `src/discovery/match_engine.rs`. Corrected in a supervisor
  integration commit right after the 23-adr merge (`docs/adr/0009-corpus-record-schema.md`, one
  sentence).
- Wave-1 workers (21, 22, 24) each rebuilt release from scratch in their fresh worktrees (~1.5 min);
  measured wave wall ≈ 7.5 min for four concurrent workers.

Phase 2 dependency graph: 10 first (alone). Then {11, 12, 13} in parallel. Then 14 (needs all three
stages) in parallel with {15, 16, 18}; 17 after 14; 19 after everything. File ownership in Phase 2:
`src/discovery/mod.rs` is touched by 10 (`pub mod` wiring) then 14 (re-exports) only;
`src/discovery/corpus.rs` by 10 (stub) then 11; `annotate.rs` by 10 then 12; `summary.rs` by 10 then
13; `src/discovery/config.rs`, `configs/**` and `tests/fixtures/**` by 10 only; `src/io/jsonl.rs` and
`src/io/mod.rs` by 11 only; every `tests/*.rs` file by exactly one step. Steps 15/16/18 must address
the library through full module paths (`strategy_discovery::discovery::corpus::generate`), never the
crate-root re-exports, because those land in step 14 which may not be merged into their worktree.
Stages 11/12/13 are mutually independent by construction: `annotate.rs` and `summary.rs` each read
`run.json` through their own private `RunHeader` deserializer instead of importing
`corpus::RunMetadata`.

Decomposer notes for Phase 2 (measured on this machine at `99e1636` with throwaway probes, not
committed):

- Debug generation cost of `tests/fixtures/generate-small.toml` (36 cells x 4 games = 144 games,
  strategies random / depth-2 / perfect-with-`tie_break = "engine"`): ~0.5 s per run. The same sweep
  with `perfect` on the default seeded-uniform tie-break costs ~1.2 s, and without the openings
  ~4.2 s (a depth-9 search from the empty board dominates). The fixture therefore keeps
  `tie_break = "engine"` for `perfect`, exactly matching the `SMALL` const already committed in
  `src/discovery/config.rs`'s test module, so the fixture text is known to parse and resolve.
- Measured diversity of `generate-small.toml` at seeds 1 / 2 / 3: `distinct_game_fraction`
  0.812 / 0.826 / 0.854, `canonical_coverage` 0.318 / 0.298 / 0.301, `decisive_fraction`
  0.611 / 0.604 / 0.590, ~1050 position records per run. Fixture thresholds in
  `diversity_thresholds_pass_across_seeds` are therefore 0.25 / 0.2 / 0.5 (the brief's values, kept
  unchanged; coverage has ~19 % headroom).
- Measured diversity of `configs/tictactoe-default.toml` (49 cells, `games_per_cell = 20`, 980 games)
  at seeds 1 / 2 / 3 in release: `distinct_game_fraction` 0.591 / 0.588 / 0.626, `canonical_coverage`
  0.601 / 0.612 / 0.607, `decisive_fraction` 0.522 / 0.520 / 0.511 — all pass the default
  0.5 / 0.2 / 0.5 thresholds, so the Phase 3 Gate B diversity claim holds with the plain defaulted
  config and no enrichment is needed. Release serial throughput: 980 games in 1.3 s (~745 games/s).
- `tests/corpus_bench.rs` must size itself with `cfg!(debug_assertions)`: `games_per_cell = 2`
  (98 games, ~1.7 s) in debug, `20` in release. `games_per_cell = 20` in debug is far over budget.
- Exhaustive annotation of tic-tac-toe in debug: `reachable_states` 22 ms, solving all 5478 states
  34 ms, 4520 depth-9 engine searches 402 ms — under 0.5 s per pass, so running it twice for the
  byte-identity check is cheap.
- Estimated Phase 2 addition to the debug `cargo test` wall: ~20 s on top of the existing ~25 s.
- `tests/fixtures/generate-small-explicit.toml` is a new file the brief did not name; it is
  `generate-small.toml` with the nine pairings spelled out, and it exists so
  `defaults_spelled_out_produce_same_run_id` can compare two real config FILES.

Phase 1 dependency graph: {01, 02, 03, 04} start in parallel; 05 after 01; 06 after {01, 02, 04, 05}; 07 after {03, 06}; 08 after {05, 07}; 09 after all. Step 02 creates doc-only placeholders `src/discovery/bundle.rs` / `src/discovery/config.rs` and the `pub mod` wiring in `src/discovery/mod.rs`; 06 and 08 overwrite/extend `config.rs`, 07 overwrites `bundle.rs` (serialized by `depends_on`). `src/io/mod.rs` is touched by 01 then 05 only; `src/discovery/mod.rs` by 02 only; `src/games/tictactoe/mod.rs` by 07 only; `src/strategy/mod.rs` by 03 only. `src/discovery/mod.rs` gets only `pub mod` wiring in Phase 1 — re-exports of the new discovery items are left for the Phase 2 `discovery/mod.rs` step.

Decomposer notes for Phase 1 (verified at `1f4a905` with a throwaway test, not committed):

- `toml 1.1.4` parses `GenerateConfig<Move>` from the brief's spelling: top-level scalars/arrays (`evaluators`, `random_opening_plies`, `pairings = [["a","b"]]`) MUST precede the `[[strategies]]` (`name` + nested `[strategies.spec]` with `kind`) and `[[openings]]` (`actions = [4]`) tables; a top-level key placed after a table silently joins that table. `deny_unknown_fields` and unknown `kind` surface as `toml` errors whose message contains the offending token.
- Serde pitfall: a bare `#[serde(default)]` on a field whose type mentions the type parameter `A` (e.g. `Vec<NamedOpening<A>>`, `Vec<A>`) makes serde add `A: Default` to the `Deserialize` impl, which `Move` lacks. Steps 04/06 mandate `#[serde(default = "Vec::new")]` on those fields; `Opening<A>` gets a manual `Default`.
- The `CorpusGame` supertrait with associated-type bounds (`GameDomain<State: Serialize + DeserializeOwned, ...>`) plus the where-clause blanket impl compiles on rustc 1.94.1 and `TicTacToe: CorpusGame` holds.
- Reference values baked into steps: `opening_seed(0) = 0x4396D60DBD8537AF`, `opening_seed(42) = 0xC549D6F38899C014`, `opening_seed(0xBDD732262FEB6E95) = 0x5A0ECCCE1EDF2C68`; `cell_seed(0,0) = 0xBA8894FA3BE59747`, `cell_seed(0,1) = 0x2366A2894CCB62DB`, `cell_seed(20260821,0) = 0xF7AB10D4F37DAD2A`, `cell_seed(20260821,1) = 0x45120386E5756643`; `fnv1a64(b"abc") = 0xe71fa2190541574b`; `config_hash(json!({"a":1})) = "9c3e82dd6fcae8b1"`.
- `check_schema_version` lives in `src/io/schema.rs` (re-exported as `crate::io::check_schema_version`), not `jsonl.rs`; `CorpusError` lives in `src/discovery/config.rs` as briefed, which forces the 01/02 → 06 → 07 → 08 chain (bundle needs `CorpusError`, `resolve` needs the bundle).

Planner notes for the decomposer:

- Baseline at `e4e0a7e` (= develop `7e1026e` + the committed plan documents; source identical to `c2d1386`): `cargo test` = 191 lib + match_bench 2 (17.5 s debug) + match_engine 9 + minimax_tactics 7 + smoke 2 + ttt_bench 2, all green; debug wall ≈ 25 s. fmt check exit 0 with the repo `rustfmt.toml` (`max_width = 132`, committed `7e1026e`). `core.autocrlf = true` locally — never byte-compare checked-out fixtures.
- `tests/smoke.rs::cli_bootstrap_runs` calls `cli::run()`; it must switch to `cli::run_from(["strategy-discovery"])` in the same step that makes `run()` parse argv, or `cargo test` breaks mid-phase.
- Verify the `GenerateConfig<Move>` TOML spelling (`[[strategies]]` + `[strategies.spec]` with `kind`, `pairings = [["a","b"]]`, `[[openings]]` with `actions = [4]`) against `toml 1.1.4` before fixing fixture text in step files.
- Fixture boards must obey `Board::parse` (`#X == #O` ⇒ X to move; `#X == #O + 1` ⇒ O to move). Solver facts to assert: empty → value 0, 9 optimal; `XX.OO....` → 1, `[2]`; `XX.O.....` → -1, all six empties; `X...O...X` → 0, `[1,3,5,7]`; 5478 states, 958 terminal, labels 2936 / 1068 / 1474.

## Revisions

<!-- supervisor appends: phase | failed step | revision note | outcome -->
