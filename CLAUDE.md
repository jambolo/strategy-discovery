# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

A Rust framework for **automated strategy discovery** in board games: generate corpora of games, annotate them with engine ground truth, mine them for human-readable heuristics, and validate those heuristics by benchmark match play. Tic-tac-toe is the first (and only MVP) game; the framework is designed so new games slot in without pipeline changes.

**[docs/plan.md](docs/plan.md) is the authoritative plan.** It defines 9 phases, acceptance checks per phase, and the component-evaluation process. Read it before starting any implementation work; every platform decision must answer "can the Phase 8–9 miners consume this?" The codebase is currently at Phase 1 (baseline setup) — most of the source tree described below does not exist yet and is the intended target structure.

Prefer open-source AI/ML crates over hand-rolled implementations wherever possible (e.g. `linfa`/`smartcore` for rule induction, `polars` for the dataframe layer — selected via the Phase 3 weighted-scoring process, not added speculatively).

## Commands

```sh
cargo check                 # fast compile check
cargo build                 # build
cargo test                  # all tests
cargo test <name>           # single test or module (e.g. cargo test ttt)
cargo fmt --all --check     # format check (CI-enforced on PRs)
cargo clippy --workspace --all-targets --all-features -- -D warnings   # lint (CI-enforced; warnings are errors)
cargo run -- <subcommand>   # CLI stages: play, generate, annotate, analyze, report, evaluate, pipeline, discover
```

Rust edition 2024, stable toolchain. CI runs build + test on Ubuntu and Windows; fmt/clippy run on pull requests only, but run them locally before pushing.

## Git workflow

Gitflow-style branching: `master` (releases), `develop` (integration), `feature/**` and `release/**` working branches. Branch features off `develop` and PR back into it. Pushing a `Cargo.toml` version change to `master` triggers CD, which tags `v<version>` and merges `master` back into `develop`. CI builds/tests pushes to `master`, `develop`, and `release/**`; docs deploy to GitHub Pages from `master`; coverage runs on `develop`.

## Architecture

### Intended module layout (from the plan)

- `src/core/` — framework traits (`GameRules`, `GamePrimitives`, `FeatureExtractor`, `Canonicalize`, `StateEvaluator`, `Strategy`, `StrategyGenerator`, `MatchEngine`), the feature-algebra types, and the heuristic-rule DSL. **Nothing game-specific lives here.**
- `src/games/tictactoe/` — game domain: rules, board, primitives declaration, tier-2 features, symmetry canonicalization.
- `src/strategy/` — `game-player` engine adapters, `MinimaxStrategy` (configurable depth, seeded tie-breaks, epsilon-random), and the strategy registry/factory (named kinds: `minimax`, `random`, reserved `heuristic-rules`).
- `src/discovery/` — corpus generation runner, annotation stage, feature dataset and heuristic miner, strategy archive.
- `src/io/` — versioned record schema, JSONL (optionally Parquet) writers, replay parser.
- `src/cli/` — one subcommand per pipeline stage.
- `src/main.rs` — CLI bootstrap and dispatch only.

### Cross-cutting design rules

1. **Pipeline = composable stages with file handoffs**: `generate -> annotate -> analyze -> report`. Each stage is a separate CLI command reading/writing files; analyzers are a pluggable registry so future miners slot in without restructuring.
2. **Match simulation is embarrassingly parallel** (rayon): game state must be cheap to clone; no shared mutable state across games. A parallel batch with the same seeds must produce results identical to a serial run.
3. **Seeded reproducibility, not pure determinism**: every run is reproducible given a seed, but seeded stochasticity (tie-breaks, epsilon, mixed opponent pairings) is deliberately used to create corpus diversity. A corpus of identical perfect-play draws is a failure.
4. **Open, tiered feature vocabulary** — the framework's key reusability lever: tier 1 = primitives derived mechanically from `GamePrimitives` (e.g. tic-tac-toe's center/corners/edges fall out of the symmetry group, never hand-coded); tier 2 = hand-supplied strategic features (threats, forks) as a bootstrap, not a ceiling; tier 3 = concepts invented at runtime (Phase 9). Features are serializable, composable expressions; nothing in core assumes a fixed feature set.
5. **The framework owns rule *forms*; games supply *vocabulary*.** The strategy DSL (ordered decision lists over features) is a first-class core type. Emitted heuristics must be self-contained: they carry definitions of every non-primitive feature they reference.
6. **`game-player` (jambolo/game-player, git-pinned) is a component, not the discovery engine.** Its three roles: game generation, per-position annotation, benchmark opposition.
7. **Adding a game must touch only game-side code**: `GameRules`, `GamePrimitives`, engine adapters, optionally `Canonicalize` and tier-2 features — never the pipeline, corpus, annotation, or evaluation layers.

### Component selection discipline

Contested choices (the analysis/mining stack) get weighted scoring recorded in `docs/component-evaluation.md`; settled plumbing gets short ADR notes in `docs/adr/`. Benchmark and reproducibility evidence goes in `artifacts/benchmarks/` and `artifacts/reproducibility/`. Don't add analysis crates (`polars`, `linfa`, `smartcore`) before the Phase 3 evaluation selects them.

### Testing conventions

- Deterministic unit tests for game rules (all 8 win lines, draws, illegal moves), symmetry (~5,478 legal positions, 765 canonical), and derived orbits.
- Tactical regression fixtures for engine correctness (immediate win/block; perfect-vs-perfect always draws across multiple tie-break seeds).
- Property-based tests for framework invariants (terminal states have no legal moves; canonicalization is idempotent and outcome-preserving).
- Micro-benchmarks are non-failing and logged, run from Phase 2 onward.
