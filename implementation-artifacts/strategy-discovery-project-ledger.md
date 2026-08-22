# strategy-discovery — Project Ledger

## Project

- project-name: strategy-discovery
- artifacts-dir: implementation-artifacts
- develop-branch: develop
- current-milestone: 2
- base-commit: 8573d79 (develop, plan.md Phases 1–6 merged)

## Milestones

| # | Name | Status | Branch | Merge commit | Notes |
| --: | --- | --- | --- | --- | --- |
| 1 | Strategy evaluation harness, archive, and hardening | done | milestone/1-evaluation-harness (deleted) | 5ee100f | plan.md Phase 7 / Gate C; 4 phases, 24 steps; Gate C PASS; project gate 14/14 PASS |
| 2 | Heuristic mining and `discover` | pending | | | plan.md Phase 8 |
| 3 | Concept induction | pending | | | plan.md Phase 9 (soft DoD) |

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
