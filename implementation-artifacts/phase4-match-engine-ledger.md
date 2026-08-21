# phase4-match-engine — Ledger

Single source of truth for execution state. Sections are owned by different skills —
the planner seeds Plan + Phases; the decomposer fills Steps per phase; the supervisor
updates Steps and appends Revisions.

## Plan

- plan-name: phase4-match-engine
- current-phase: complete
- working-branch: feature/phase-4-match-engine
- starting-commit: a0c285ca9cd4b6ba1b284c6f6b9dfc91923fc228
- default-branch: develop
- artifacts-dir: implementation-artifacts/

## Phases

| Phase | Status  | Notes |
| ----: | ------- | ----- |
| 1     | done    | Engine glue, strategies, evaluator, match engine — 172 lib tests, gate PASS (678c039) |
| 2     | done    | Registry, roster, acceptance suites, ADR 0008 — steps 09-16; 191 lib tests, `minimax_tactics` 7/7 (1.4 s), `match_engine` 9/9, `match_bench` release serial≈150 / parallel≈1040 games/s, gate PASS (b92a2e7); `cargo test` debug wall ≈27 s |
| 3     | done    | Step 17-gate verdict PASS (dfa905b); supervisor re-ran fmt/clippy/`cargo test` (191 lib, all suites 0 failed) + release bench (serial≈146 / parallel≈1028 games/s); debug wall 27.08 s, release 10.78 s. Plan complete. |

## Steps

<!-- decomposer fills per phase: id | phase | status | files | commit -->

| id | phase | status | files | commit |
| --- | --- | --- | --- | --- |
| 01-engine-glue | 1 | done | src/core/mod.rs, src/strategy/mod.rs, src/strategy/engine.rs, src/strategy/minimax.rs, src/strategy/random.rs, src/strategy/scripted.rs, src/discovery/mod.rs, src/discovery/match_engine.rs, src/games/tictactoe/engine.rs, implementation-artifacts/phase4-match-engine-01-engine-glue-report.md | 4ea72e1 |
| 02-ttt-eval | 1 | done | src/games/tictactoe/eval.rs, src/games/tictactoe/mod.rs, implementation-artifacts/phase4-match-engine-02-ttt-eval-report.md | 0d719f8 |
| 03-random | 1 | done | src/strategy/random.rs, implementation-artifacts/phase4-match-engine-03-random-report.md | cd473da |
| 04-scripted | 1 | done | src/strategy/scripted.rs, implementation-artifacts/phase4-match-engine-04-scripted-report.md | b35644e |
| 05-minimax | 1 | done | src/strategy/minimax.rs, implementation-artifacts/phase4-match-engine-05-minimax-report.md | edcf4fe |
| 06-match-engine | 1 | done | src/discovery/match_engine.rs, src/discovery/mod.rs, implementation-artifacts/phase4-match-engine-06-match-engine-report.md | 76de7b3 |
| 07-strategy-reexports | 1 | done | src/strategy/mod.rs, implementation-artifacts/phase4-match-engine-07-strategy-reexports-report.md | 453b18b |
| 08-gate | 1 | done | implementation-artifacts/phase4-match-engine-08-gate-report.md | 678c039 |
| 09-registry | 2 | done | src/strategy/registry.rs, src/strategy/mod.rs, implementation-artifacts/phase4-match-engine-09-registry-report.md | 2011b1b |
| 10-roster | 2 | done | src/strategy/roster.rs, src/strategy/mod.rs, implementation-artifacts/phase4-match-engine-10-roster-report.md | b81edc5 |
| 11-ttt-roster | 2 | done | src/games/tictactoe/roster.rs, src/games/tictactoe/mod.rs, implementation-artifacts/phase4-match-engine-11-ttt-roster-report.md | b507873 |
| 12-tactics | 2 | done | tests/minimax_tactics.rs, implementation-artifacts/phase4-match-engine-12-tactics-report.md | c345027 |
| 13-match-tests | 2 | done | tests/match_engine.rs, implementation-artifacts/phase4-match-engine-13-match-tests-report.md | e3c48aa |
| 14-bench | 2 | done | tests/match_bench.rs, implementation-artifacts/phase4-match-engine-14-bench-report.md | 41df675 |
| 15-adr | 2 | done | docs/adr/0008-minimax-tie-breaking.md, implementation-artifacts/phase4-match-engine-15-adr-report.md | d3bc0ce |
| 16-gate | 2 | done | implementation-artifacts/phase4-match-engine-16-gate-report.md | b92a2e7 |
| 17-gate | 3 | done | implementation-artifacts/phase4-match-engine-gate-report.md, implementation-artifacts/phase4-match-engine-17-gate-report.md | dfa905b |

Phase 1 dependency graph: 01 → {02, 03, 04, 06} in parallel; 05 after {01, 02}; 07 after {03, 04, 05}; 08 after {02, 06, 07}. Steps 03/04/05/06 overwrite placeholder files that 01 creates; `src/strategy/mod.rs` is touched by 01 then 07 only, `src/discovery/mod.rs` by 01 then 06 only.

Phase 2 dependency graph: {09, 12, 14, 15} start in parallel; 10 after 09; 11 after 10; 13 after 11; 16 after {11, 12, 13, 14, 15}. `src/strategy/mod.rs` is touched by 09 then 10 only (serialized by `depends_on`); `src/games/tictactoe/mod.rs` by 11 only.

Phase 3 dependency graph: 17-gate after 16-gate — single verification-only step; scope = the project gate report (`phase4-match-engine-gate-report.md`, brief DoD item 7) plus its own step report. Decomposer pre-verified at 437e041: ADR 0008 contains `upstream` (line 53), `core::kinds` has `SCRIPTED`/`EVOLUTIONARY`/`LLM`, all nine DoD-2 files open with `//!`, `pub mod` wiring present, `impl EngineGame for TicTacToe` at `src/games/tictactoe/engine.rs:72`.

Decomposer note for Phase 2 (resolved at decomposition): the exhaustive oracle was run against all seven fixtures — `win-o` `XX.OO..X.` depth-9 optimal set is `{2, 5}` (not `{5}`; depth-1 set is `{5}`); `block-o` `XX.O.....` depth-9 set is all six legal moves (depth-2 set `{2}`); other rows as in the brief. Step 12-tactics carries the corrected table. Exhaustive depth-9 engine-in-tie-set and tie-set-subset-of-oracle checks: 0 offenders over 4520 non-terminal positions, 1.4 s debug. Dry runs: perfect-vs-perfect 32 games seed 7 → 31 distinct draws; depth-1 vs depth-9 → 64/83 decisive of 100; epsilon 0.25 self-play → 109/200 decisive. `toml 1.1.4` round-trips the internally tagged `StrategySpec` and a nested `Roster`.

Supervisor note for Phase 2 (step 14-bench): release numbers on the dev box — `[bench] games/s serial=147 parallel=1008 speedup=6.84x (200 games, depth 9, seeded-uniform)`, `[bench] games/s engine-tie-break serial=410 (200 games, depth 9)`. Debug-build runtime of `cargo test --test match_bench` is ~19-20 s (the step context's ~1 s estimate was wrong: PV-replay `root_values` multiplies per-ply search cost). The file doc comment repeated the wrong estimate; corrected in supervisor commit 46df1b0. Non-failing bench, no revision needed. Step 12-tactics: both exhaustive depth-9 checks 0 offenders over 4520 non-terminal positions, binary 1.4 s debug; fixture table needed no correction.

Supervisor note for Phase 3 (step 17-gate): worker report records `base: a0c285c` (plan starting commit) where the actual launch HEAD was eb914ea; git identity used instead. No revision needed.

Original planner note for Phase 2 (tactical fixtures): the brief's `block-o` fixture `XX.O.....` has optimal set `{2}` only at depth 2; at depth 9 every O move loses (after O2, X4 forks), so the depth-9 tie set is all six legal moves. Phase-1 tests avoid asserting `{2}` at depth 9; the Phase 2 oracle re-verification must correct that fixture row.

## Revisions

<!-- supervisor appends: phase | failed step | revision note | outcome -->
