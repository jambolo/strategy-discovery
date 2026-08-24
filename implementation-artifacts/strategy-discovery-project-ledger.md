# strategy-discovery — Project Ledger

## Project

- project-name: strategy-discovery
- artifacts-dir: implementation-artifacts
- develop-branch: develop
- current-milestone: complete (3 of 3 done)
- base-commit: 8573d79 (develop, plan.md Phases 1–6 merged)

## Milestones

| # | Name | Status | Branch | Merge commit | Notes |
| --: | --- | --- | --- | --- | --- |
| 1 | Strategy evaluation harness, archive, and hardening | done | milestone/1-evaluation-harness (deleted) | 5ee100f | plan.md Phase 7 / Gate C; 4 phases, 24 steps; Gate C PASS; project gate 14/14 PASS |
| 2 | Heuristic mining and `discover` | done | milestone/2-heuristic-mining (deleted) | ee071db | plan.md Phase 8; 4 phases, 35 steps; gate 12/12 PASS; merged and cleaned up outside the pipeline |
| 3 | Concept induction | done | milestone/3-concept-induction (deleted) | c8f17bc | plan.md Phase 9 (soft DoD); 4 phases, 30 steps; gate 13/13 PASS; lead DoD 7/7 PASS |

## Events

<!-- lead-developer appends: milestone | event (planned / phase N done / evaluated /
     merged / escalated) | detail -->

| Milestone | Event | Detail |
| --- | --- | --- |
| – | project planned | 2026-08-22; 3 milestones; test budget 90 s; deps policy proptest/tempfile/assert_cmd (m1), linfa/linfa-trees/ndarray (m2); Phase 9 DoD soft |
| 1 | branched | 2026-08-22; milestone/1-evaluation-harness from develop dcd22b1; plan-name strategy-discovery-m1 |
| 1 | planned | 2026-08-22; brief + 4-phase roadmap + ledger on milestone branch |
| 1 | phase 1 done | 2026-08-22; 3863585; serde_json float_roundtrip escalation approved + ADR 0013 |
| 1 | phase 2 done | 2026-08-22; c2d764f; no revisions; debug wall 38.95 s |
| 1 | phase 3 done | 2026-08-22; 78a160a; no revisions; 21 test binaries ok; debug wall 41.61 s; docs/adding-a-game.md 407 lines (over 200-350 target, DoD-neutral) |
| 1 | phase 4 done | 2026-08-23; 177e058; one in-flight step-22 column rename (command -> cmd); determinism 6/6 YES; release pool 1560.6 games/s (6.03x), evaluate pool 3777 games/s; Gate C PASS; gate 14/14 PASS |
| 1 | evaluated | 2026-08-23; lead re-ran m1 DoD on branch: cargo test --workspace exit 0, 21 ok / 0 failed, wall 41.55 s (<= 60); fmt/clippy/doc 0; 4 invariant proptests pass; evaluate twice byte-identical (3 files), perfect loss 0.0 all 7 opponents both seats, random vs perfect 0.875, depth-9 agreement 1.000; archive 3 entries w/ provenance+novelty, round-trip test ok; adding-a-game.md checklist present; games.rs only non-test tictactoe; both evidence files 8-field header; Cargo.toml = 3 dev-deps + approved float_roundtrip |
| 1 | merged | 2026-08-23; cleanup aa699ab (roadmap + 24 steps + 24 reports removed; brief, ledger, gate-report kept); squash 5ee100f on develop from base 48c97c8; branch deleted |
| 2 | branched | 2026-08-23; milestone/2-heuristic-mining from develop 81d268c; plan-name strategy-discovery-m2 |
| 2 | phases 1-4 done | 2026-08-23/24; steps 01-35; per-phase DoDs verified by the supervisor on the integrated tree; gate report `35-gate` 12/12 PASS |
| 2 | evaluated | 2026-08-24; m2 DoD spot-verified by the lead on develop after the merge: `cargo test --workspace` 0 failed across 26 test binaries (362 lib tests); Cargo.toml gained exactly linfa 0.8.1 / linfa-trees 0.8.1 / ndarray 0.16.1; `src/cli/discover.rs`, `src/discovery/discover.rs`, `docs/adr/0014-feature-dataset.md`, `artifacts/reproducibility/discover-determinism.md` all present |
| 2 | merged | 2026-08-24; squash ee071db on develop; branch deleted; artifact cleanup and merge performed by the user outside the pipeline (roadmap/steps/reports removed, brief + ledger kept) |
| 3 | branched | 2026-08-24; milestone/3-concept-induction from develop; plan-name strategy-discovery-m3 |
| 3 | planned | 2026-08-24; brief + 4-phase roadmap + ledger on milestone branch |
| 3 | phases 1-4 done | 2026-08-24 to 2026-09-10; steps 11-46 (30 steps); one revision (24-concepts-analyzer); no tuning of configs/tictactoe-concepts.toml; gate report `46-gate` 13/13 PASS at 2385b49 |
| 3 | evaluated | 2026-09-10; lead re-ran m3 DoD on branch: fmt/clippy/doc exit 0 with 0 doc warnings; timed `cargo test --workspace` wall 51.79 s (<= 90), 28 ok / 0 failed; Cargo.toml/lock diff vs c997c35 empty; `discover --config configs/tictactoe-concepts.toml` exit 0, 18 files, repeat run 18/18 byte-identical + stdout identical, hashes match the four gate pins; concepts.json: concept_1 (round 1) and concept_2 (round 2) promoted with definitions, provenance (run_id bd760ff0ace48705, params_hash f1596db3a853769f, seed 0, round), 22 similarity rows each vs withheld ttt.* features; mined-d12-l1 and mined-d0-l1 reference concept_2 with loss 0.000 vs perfect both seats (0/20, 0/20); report.md carries both tier-3 definitions inside every mined-* block; vocabulary.json overall supplied 0.0 / invented 0.0196 with per-candidate fractions; determinism artifact 18 hash rows, 2 identical: YES, 0 NO; rejects_candidate_* 2 passed; cli_concepts 8 passed incl. validate() self-containment |
| 3 | merged | 2026-09-10; cleanup 79f9911 (roadmap + 30 steps + 30 reports removed; brief, ledger, gate-report kept; 46-gate ledger row marked done); squash c8f17bc on develop from base c997c35; branch deleted |
| – | project complete | 2026-09-10; all 3 milestones done; project DoD = plan.md Phases 7-9 gates, each verified at its milestone evaluate row (m1 Gate C + 14/14, m2 12/12, m3 13/13 + lead 7/7) |
