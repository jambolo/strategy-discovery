# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

A Rust framework for **automated strategy discovery** in board games: generate corpora of games, annotate them with engine ground truth, mine them for human-readable heuristics, invent new concepts to express those heuristics, and validate them by benchmark match play. Tic-tac-toe is the only implemented game; the framework is built so a second game touches only game-side code.

All planned milestones are complete (evaluation harness, heuristic mining, concept induction). There is no live plan document. The design record is the ADR series in `docs/adr/` and the onboarding guide `docs/adding-a-game.md`; `README.md` is the user-facing CLI reference, including every flag, the exit codes, and the run-directory file table.

## Commands

```sh
cargo test --workspace                        # all tests; debug wall budget <= 90 s (~52 s warm today)
cargo test --test ttt_bench -- --nocapture    # micro-benchmarks are non-failing; they print `[bench]` lines
cargo fmt --all --check                       # CI-enforced on PRs; rustfmt max_width = 132
cargo clippy --workspace --all-targets --all-features -- -D warnings   # CI-enforced on PRs
cargo doc --no-deps --workspace               # must build with zero warnings (deployed to GitHub Pages)
cargo run -- <subcommand>                     # one subcommand per pipeline stage; see `--help`
```

Rust edition 2024, stable toolchain. CI runs build + test on Ubuntu and Windows; fmt/clippy run on pull requests only, so run them locally before pushing.

`pipeline` takes `--experiment`; `discover` takes `--config`. `-v`/`-q` affect stderr only; stdout and files are byte-stable.

## Git workflow

Gitflow-style branching: `master` (releases), `develop` (integration), `feature/**` and `release/**` working branches. Branch features off `develop` and PR back into it. Pushing a `Cargo.toml` version change to `master` triggers CD, which tags `v<version>` and merges `master` back into `develop`. CI builds/tests pushes to `master`, `develop`, and `release/**`; docs deploy to GitHub Pages from `master`; coverage runs on `develop`.

## Design rules

1. **Pipeline = composable stages with file handoffs**: `generate -> annotate -> analyze -> report`. Each stage is a CLI command reading and writing a run directory; analyzers are a pluggable registry. Stage independence: `analyze` run standalone over a `discover` run dir must reproduce its outputs byte-identically.
2. **Match simulation is embarrassingly parallel** (rayon): game state must be cheap to clone; no shared mutable state across games. A parallel batch with the same seeds must equal a serial run.
3. **Seeded reproducibility, not pure determinism**: every run is reproducible from its seed, and seeded stochasticity (tie-breaks, epsilon, mixed pairings) is used on purpose for corpus diversity. Repeat runs of any command with the same inputs must be byte-identical; tests pin this.
4. **Open, tiered feature vocabulary**: tier 1 = primitives derived mechanically from `GamePrimitives` through the symmetry group (center/corners/edges are orbits, never hand-coded); tier 2 = hand-supplied features (threats, forks), a bootstrap that can be withheld; tier 3 = concepts invented at runtime by `induce` (`concept_N`, auto-generated definition text, provenance, stackable across rounds). Features are serializable `FeatureExpr`s; nothing in core assumes a fixed set.
5. **The framework owns rule forms; games supply vocabulary.** Emitted heuristics are self-contained: they carry definitions of every non-primitive feature they reference, enforced by `HeuristicStrategy::validate()`.
6. **`game-player` (jambolo/game-player, git-pinned) is a component, not the discovery engine.** Its roles: game generation, per-position annotation, benchmark opposition.
7. **Adding a game touches only game-side code** plus one `dispatch_game!` arm and one `KNOWN_GAMES` entry in `src/cli/games.rs`. Follow `docs/adding-a-game.md`, including its second-game dry-run checklist.
8. **Game-neutral wording**: outside `src/games/` and `src/cli/games.rs`, non-test code must not mention `tictactoe`, and `src/core`, `src/discovery`, `src/io`, `src/cli` must not use game-concept words (`threat`, `fork`, `corner`, `center`, `edge`). Test modules are exempt.

### Dependencies and decision records

- Root dependency changes require an ADR. `polars` was evaluated and rejected from the default build (ADR 0005); `serde_json`'s `float_roundtrip` feature is required, not optional (ADR 0013).
- ADRs `docs/adr/0001`–`0018` are the design record. Add `docs/adr/NNNN-<slug>.md` for any new component or contract change: title `# ADR NNNN: <name>`, sections `Status`, `Context`, `Decision`, `Consequences`.
- Benchmark and reproducibility evidence files and the original phase plan were removed at project completion; the measurements that justified decisions are quoted inside the ADRs.

### Testing conventions

- Deterministic rule tests (all 8 win lines, draws, illegal moves), symmetry (5,478 legal positions, 765 canonical), derived orbits.
- Tactical fixtures: immediate win and block; perfect-vs-perfect draws across many seeds; a hand-written `heuristic-rules` strategy takes wins and blocks threats.
- Property tests (`proptest`, `tests/invariants.rs`): terminal states have no legal moves, canonicalization is idempotent and outcome-preserving, serde round-trips. Keep `tests/invariants.proptest-regressions` committed.
- Determinism tests: repeat runs byte-identical, parallel equals serial, standalone `analyze` reproduces `discover` outputs.
- Micro-benchmarks `tests/*_bench.rs` are non-failing and print `[bench]` lines under `-- --nocapture`.
- CLI tests spawn the built binary (`assert_cmd`) against tiny fixtures in `tests/fixtures/`. Keep tests tiny; full-scale runs use `configs/*.toml` and are not tests.
- Budget: debug `cargo test --workspace` wall time ≤ 90 s.
