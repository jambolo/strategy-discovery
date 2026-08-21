# phase1-baseline — Brief

## Goal

Complete Phase 1 of `docs/plan.md` ("Project baseline and dependency setup") for the
`strategy-discovery` Rust crate: correct the Cargo manifest and add all baseline
dependencies, restructure `src/` into the framework module skeleton, document the
architecture contracts, and add compile-only smoke tests. Acceptance: `cargo check` and
`cargo test` succeed.

## Context

Facts read at commit `b0a02ac874ecf47e4c44229779ea95d0a8fed219` (branch
`feature/phase1-baseline`). Line numbers and file contents may shift as steps land —
re-verify against live files before editing.

### Current repo state

- `Cargo.toml` — 6 lines: package `strategy-discovery`, version `0.1.0`,
  edition `2024`, empty `[dependencies]`. No workspace, single binary crate.
- `src/main.rs` — hello-world only. No `src/lib.rs`. No other source files.
- `tests/` — does not exist.
- `docs/plan.md` — authoritative 9-phase project plan. Phase 1 is lines 27–42.
- `CLAUDE.md` — repo instructions; already describes the target module layout.
- `README.md`, `LICENSE`, `.gitignore`, `.github/workflows/{ci,cd}.yml` exist.
- Toolchain on this machine: rustc 1.97.1, cargo 1.97.1 (stable).

### CI contract (`.github/workflows/ci.yml`)

- Push to `master`/`develop`/`release/**` and all PRs: `cargo build --workspace
  --all-targets` and `cargo test --workspace` on ubuntu-latest + windows-latest.
- PRs additionally: `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Therefore every commit must be fmt-clean and clippy-clean at `-D warnings` with
  `--all-features`, and must not use unix-only paths/APIs.

### game-player dependency (verified 2026-08-19)

- Repo: `https://github.com/jambolo/game-player.git`
- Pin (plan Option A, commit SHA; equals tag `v0.4.0`, current master):
  `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`
- Crate name at that rev: `game-player` (package section: name `game-player`,
  version `0.4.0`, edition `2024`). Rust identifier: `game_player`.
- Its deps: `indextree 4.7` (mandatory); `rand 0.10.2` and `serde_json 1.0` optional,
  behind non-default features. Default features = none, so no forced version coupling.
- Exact manifest line to add:

```toml
game-player = { git = "https://github.com/jambolo/game-player.git", rev = "0b8ce6f9ec66a239273a83c74a2c9515f8c4e795" }
```

- Building with this dependency requires network access to github.com for the first
  fetch. If `cargo check` cannot fetch, report `status: fail` with the error — do not
  substitute a path or crates.io dependency.

### Baseline dependency set (plan.md Phase 1 step 1)

`game-player` (git, pinned as above) plus crates.io: `clap` (with `derive` feature),
`serde` (with `derive` feature), `serde_json`, `toml`, `anyhow`, `thiserror`, `tracing`,
`tracing-subscriber`, `rand`, `rand_chacha`, `rayon`.

- Add crates.io deps with `cargo add` so the resolver picks current compatible versions;
  do not hand-pick old versions. `rand` and `rand_chacha` must be a matching pair
  (rand 0.10.x line); verify by compiling a seeded-RNG usage
  (`ChaCha8Rng::seed_from_u64(0)` behind a test or the smoke test).
- Explicitly NOT baseline (do not add): `polars`, `linfa`, `smartcore` (Phase 3
  selection), `bincode` (dropped from baseline).
- Commit the updated `Cargo.lock` together with `Cargo.toml`.

### Target module skeleton (plan.md Phase 1 step 2; content stays skeletal — traits are Phase 2)

Convert to lib + thin binary:

- `src/lib.rs` (new) — crate docs + `pub mod core; pub mod games; pub mod strategy;
  pub mod discovery; pub mod io; pub mod cli;`
- `src/core/mod.rs` — module doc: framework traits, feature algebra, strategy DSL
  (placeholders only; nothing game-specific ever lives here).
- `src/games/mod.rs` + `src/games/tictactoe/mod.rs` — module docs; tictactoe is empty
  placeholder.
- `src/strategy/mod.rs` — module doc: engine adapters, MinimaxStrategy, registry (Phase 4).
- `src/discovery/mod.rs` — module doc: corpus generation, annotation, strategy archive.
- `src/io/mod.rs` — module doc: versioned record schema, JSONL writers, replay parser.
- `src/cli/mod.rs` — clap `Parser` struct + `Subcommand` enum with the five pipeline
  stages `play`, `generate`, `annotate`, `analyze`, `report`; each dispatches to a stub
  that returns an error/message "not yet implemented".
- `src/main.rs` — reduce to CLI bootstrap and dispatch only: parse via
  `strategy_discovery::cli`, init `tracing-subscriber`, dispatch.
- Module-wiring convention so smoke tests have something concrete and clippy-safe to
  assert: each of the six top modules exposes
  `pub const MODULE_PATH: &str = "<name>";` (e.g. `"core"`). Remove in a later phase
  when real APIs exist.
- A module named `core` is legal (`crate::core`); refer to std core as `::core` if ever
  needed. Do not rename it.

### Smoke tests (plan.md Phase 1 step 4)

- `tests/smoke.rs` — integration test importing all six modules from the
  `strategy_discovery` lib and asserting each `MODULE_PATH` value with `assert_eq!`
  (avoids `clippy::assertions_on_constants`).
- One additional smoke assertion that the dependency set links: construct
  `rand_chacha::ChaCha8Rng::seed_from_u64(0)`, draw a value; reference one
  `game_player` public item (any re-export/type; check its docs/source at the pinned
  rev for a cheap public symbol) so the git dep is proven to compile and link.

### Architecture docs (plan.md Phase 1 step 3)

- Add an "Architecture" section to `README.md` describing the six modules and their
  boundaries (core = game-agnostic framework; games = per-game vocabulary; strategy;
  discovery; io; cli) and the two cross-cutting design rules verbatim in substance:
  1. Pipeline = composable stages with file handoffs: `generate -> annotate ->
     analyze -> report`; each stage a separate CLI command reading/writing files.
  2. Match simulation is embarrassingly parallel (rayon): game state cheap to clone,
     no shared mutable state across games; parallel batch with same seeds must equal
     serial run.
- Also state: `game-player` is a pinned component (generation, annotation, benchmark
  opposition), not the discovery engine; adding a game touches only game-side code.
- Do not duplicate `docs/plan.md`; link to it.

## Constraints

- Rust edition 2024, stable toolchain. Single crate (no workspace split).
- Every commit passes: `cargo check`, `cargo test`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- All work commits to local branch `feature/phase1-baseline`. Never push; never touch
  `develop`/`master`.
- Only jobs of Phase 1: no framework traits, no game logic, no feature system, no
  strategy implementations (Phases 2+).
- Windows compatibility (CI matrix): no platform-specific code or paths.
- Plan artifacts live in `implementation-artifacts/` at repo root.

## Assumptions

- Network access to github.com is available for the git dependency fetch.
- `jambolo/game-player` at the pinned rev compiles on stable rustc ≥1.97 (edition 2024).
- The `docs/framework-plan.md` deliverable mentioned in plan.md's file list is not a
  Phase 1 step and is deferred.

## Out of scope

- Phase 2+ content: traits, feature algebra, strategy DSL, tic-tac-toe rules, engine
  adapters, corpus/annotation code, config loading, analyzers.
- Analysis crates (`polars`, `linfa`, `smartcore`), `bincode`, Parquet.
- CI workflow changes, version bumps, releases, pushing to remote.
- Interactive play, benchmarks (Phase 2 starts micro-benchmarks).

## Definition of Done (project)

All verified at `feature/phase1-baseline` HEAD, from the repo root:

1. `cargo check` exits 0.
2. `cargo test` exits 0 and runs the module smoke tests (≥1 test named/marked smoke).
3. `cargo fmt --all --check` exits 0.
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0.
5. `Cargo.toml` contains exactly the baseline dependency set above, with `game-player`
   git-pinned to rev `0b8ce6f9ec66a239273a83c74a2c9515f8c4e795`; `Cargo.lock` committed;
   no `polars`/`linfa`/`smartcore`/`bincode`.
6. `src/` contains `lib.rs`, `main.rs`, and the six modules (`core`, `games` with
   `tictactoe`, `strategy`, `discovery`, `io`, `cli`); `main.rs` is bootstrap/dispatch
   only.
7. `cargo run -- generate` (and each of the five subcommands) runs and reports
   not-yet-implemented rather than panicking.
8. `README.md` has the Architecture section with module boundaries and both
   cross-cutting design rules.
