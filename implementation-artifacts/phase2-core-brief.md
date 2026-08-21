# phase2-core — Brief

## Goal

Implement Phase 2 of `docs/plan.md`: framework core abstractions (traits, tiered feature system, strategy DSL) plus the tic-tac-toe domain model, with deterministic tests and micro-benchmarks. Read `docs/plan.md` lines 44-81 (at commit 056a0a9131609d25556c5c8b4287781e19e7bb0e; line numbers shift if that file is edited — re-verify against live source) for the authoritative phase spec.

## Context

### Repo state at planning time (commit 056a0a9131609d25556c5c8b4287781e19e7bb0e)

- Rust binary crate `strategy-discovery`, edition 2024, stable toolchain. `src/` contains ONLY `src/main.rs` (hello-world). The module tree described in project docs does NOT exist yet — Phase 2 must create it.
- Dependencies already in `Cargo.toml`: `anyhow`, `clap` (derive), `game-player` (git = `https://github.com/jambolo/game-player.git`, branch = `master`), `rand`, `rand_chacha`, `rayon`, `serde` (derive), `serde_json`, `thiserror`, `toml`, `tracing`, `tracing-subscriber`. No analysis crates (`polars`, `linfa`, `smartcore`) — those are Phase 3 selections and MUST NOT be added.
- CI: build + test on Ubuntu and Windows; `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` enforced on PRs.
- `docs/plan.md` is the authoritative project plan. This effort implements its "Phase 2 - Core abstractions and tic-tac-toe model".

### Target module layout (create in this effort)

- `src/lib.rs` — new library root exposing modules; `src/main.rs` reduced to CLI bootstrap stub calling into the lib.
- `src/core/` — framework traits, feature algebra, strategy DSL. NOTHING game-specific here.
- `src/games/` + `src/games/tictactoe/` — tic-tac-toe domain.
- `src/strategy/`, `src/discovery/`, `src/io/`, `src/cli/` — empty placeholder modules (`mod.rs` with doc comment) so the full intended tree compiles; their real content is Phases 4-6.
- `tests/` — integration tests.

### Framework traits to define (src/core)

Per plan.md Phase 2 step 1:

- `GameDomain` — associated types for state / action / outcome.
- `GameRules` — initial state, legal actions, apply action, terminal detection.
- `GamePrimitives` — structural facts a game declares with no strategic insight: position topology, win-condition lines, symmetry group. Framework derives further primitives mechanically (symmetry orbits).
- `FeatureExtractor` — state -> named feature values over the tiered vocabulary.
- `Canonicalize` — optional trait: state -> canonical form under symmetry.
- `StateEvaluator` — game-specific static evaluation contract for search engines.
- `Strategy` — choose action given state.
- `StrategyGenerator` — produces candidate strategies (contract only; no implementation in Phase 2).
- `MatchEngine` — parallel play loop contract over strategy providers (contract only; implementation is Phase 4).

Design rules: game state cheap to clone (`Clone`, no shared mutable state); everything needed for rayon parallelism later. Traits must not assume any fixed feature set or any specific game.

### Feature system (src/core)

Open, tiered vocabulary — Phase 9 concept induction depends on this shape:

- Tier 1: rule-derived primitives obtained mechanically from `GamePrimitives` — e.g. symmetry orbits over cells. Tic-tac-toe's {center}, {corners}, {edges} MUST fall out of orbit derivation from the symmetry group, never be hand-coded.
- Tier 2: hand-supplied per-game strategic features (threats, forks).
- Tier 3: invented concepts — named features minted at runtime as expressions over lower tiers. Phase 2 only has to represent, evaluate, and persist them.
- Features are composable expressions in a small feature algebra: serializable (serde), evaluable against states, pretty-printable definitions.

### Strategy DSL (src/core)

First-class core type, no miner emits it until Phase 8:

- Ordered decision list: rules `condition-over-features -> action selector`, with thresholds and priorities.
- Serializable (serde), pretty-prints to human-readable text, executable by a generic rule-interpreter `Strategy` (interpreter itself may be a stub returning unimplemented until Phase 8, but the types must be complete).
- Self-containment: an emitted heuristic carries definitions of every non-primitive feature it references.
- Reserve `heuristic-rules` as a named strategy kind in any registry-facing naming.

### Tic-tac-toe domain facts (src/games/tictactoe)

- Board: 3x3, cells indexable 0-8 (row-major). Cheap to clone.
- Win lines: exactly 8 — rows {0,1,2},{3,4,5},{6,7,8}; cols {0,3,6},{1,4,7},{2,5,8}; diags {0,4,8},{2,4,6}.
- Symmetry group: dihedral D4, 8 elements (identity, rotations 90/180/270, horizontal/vertical/both-diagonal reflections).
- Cell orbits under D4: {4} center, {0,2,6,8} corners, {1,3,5,7} edges — must be DERIVED from the group, not hand-coded.
- Known enumeration counts (test oracles): 5,478 reachable legal positions; 765 positions up to symmetry.
- Tier-2 features: line occupancy counts, immediate threats (two-in-a-line with third cell empty), forks (move creating two simultaneous threats).
- Canonicalization: minimum (or any fixed total order) over the 8 symmetric images of a state.

### game-player conversion layer

Phase 2 step 5: conversion from framework state/action to `game_player::State` and compatible action types. `game-player` is a git dependency already in Cargo.toml; its source is NOT on docs.rs. To learn its API: read the checked-out source under `%USERPROFILE%\.cargo\git\checkouts\game-player-*\` (run `cargo fetch` first if absent) or run `cargo doc -p game-player`. Scope is conversion/adapter types only — `MinimaxStrategy` and `ResponseGenerator`/`StaticEvaluator` implementations are Phase 4, NOT this effort.

### Tests required (plan.md Phase 2 step 6)

Deterministic unit tests covering:

- All 8 win lines detected; draw board handled; illegal moves rejected (occupied cell, out of range, move after terminal); whose-turn transitions correct.
- Symmetry-orbit derivation recovers {center}, {corners}, {edges} from the group.
- Enumeration: reachable legal positions == 5,478; canonical forms == 765.
- Canonicalization sanity (idempotent on at least fixture states; full property tests are Phase 7).

### Micro-benchmarks (plan.md Phase 2 step 7)

Non-failing, logged — NOT pass/fail assertions. Measure state clone cost and move-generation throughput. Implementation constraint: no new benchmark crates (no criterion); use `std::time::Instant` + `std::hint::black_box` in `#[test]` functions (or an ignored-by-default test run explicitly) that print/`tracing`-log timings and always pass. Runnable from Phase 2 onward.

## Constraints

- Toolchain: stable Rust, edition 2024. Windows dev machine; CI also runs Ubuntu — code must be platform-neutral.
- Must pass: `cargo check`, `cargo test`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` (warnings are errors).
- NO new dependencies without explicit instruction in a step file. Specifically forbidden: `polars`, `linfa`, `smartcore`, `criterion`, `proptest`, `bincode`. Everything needed is already in Cargo.toml.
- `src/core` must contain zero game-specific code (no "3x3", no "X/O", no tic-tac-toe cell counts).
- Serialization: serde derive with `serde_json` round-trip tests for feature-algebra and DSL types.
- Single crate (no workspace split). Library + thin binary: `src/lib.rs` owns modules, `src/main.rs` is bootstrap only.
- Commit style: plain descriptive messages, matching existing history (no conventional-commit prefixes observed).

## Assumptions

- Phase 1 is accepted as complete (deps present) even though the module tree was not created; creating it is folded into this effort.
- `game-player` builds from its `master` branch as pinned in Cargo.toml; API details are read from the fetched source at implementation time, not assumed.
- Enumeration counts 5,478 / 765 are the acceptance oracles; if an implementation disagrees, the implementation is wrong.
- Micro-benchmark output format is free-form logged text; no artifact files required in Phase 2.

## Out of scope

- `MinimaxStrategy`, strategy registry/factory, match runner, tactical regression fixtures (Phase 4).
- Corpus generation, annotation, persistence schema, JSONL writers (Phase 5).
- CLI subcommands (Phase 6) — `src/cli` stays a placeholder.
- Analysis-stack crates and component evaluation (Phase 3).
- Rule mining, concept induction (Phases 8-9).
- Property-based testing framework (Phase 7).
- Modifying `docs/plan.md`, CI workflows, or `Cargo.toml` dependencies.

## Definition of Done (project)

All checked on branch `feature/phase-2-core-abstractions`:

1. `cargo check` passes.
2. `cargo test` passes; includes tests for: all 8 win lines, draw, illegal-move rejection, turn transitions, orbit derivation ({4}/{0,2,6,8}/{1,3,5,7} derived not hand-coded), enumeration counts 5,478 legal / 765 canonical, serde round-trips for feature and DSL types.
3. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` pass.
4. `src/core` compiles with no reference to tic-tac-toe; `src/games/tictactoe` implements `GameRules`, `GamePrimitives`, tier-2 `FeatureExtractor`, `Canonicalize`; conversion layer to `game_player` types compiles against the pinned dependency.
5. Micro-benchmark tests exist, run non-failing, and log clone-cost and move-generation timings.
