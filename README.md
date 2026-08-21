# Strategy Discovery Framework

[![CI](https://github.com/jambolo/strategy-discovery/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/jambolo/strategy-discovery/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/jambolo/strategy-discovery/branch/develop/graph/badge.svg)](https://codecov.io/gh/jambolo/strategy-discovery)

A Rust framework for automated strategy discovery in board games: generate diverse corpora of games, annotate them with engine ground truth, mine them for human-readable heuristics, and validate those heuristics by benchmark match play.

Tic-tac-toe is the first concrete game. Game logic lives in replaceable modules, so more complex games can be added later without changing the pipeline or orchestration layer.

## How it works

The pipeline is a set of composable stages with file handoffs:

```text
generate -> annotate -> analyze -> report
```

- **generate** — batch self-play corpus generation with parameter sweeps (search depth, epsilon-random rates, seeded tie-breaks) designed for diversity as well as reproducibility
- **annotate** — label positions with best moves and values from the [game-player](https://github.com/jambolo/game-player) minimax engine
- **analyze** — run analyzers over a corpus; miners emit heuristics as ordered, human-readable decision lists over game features
- **report** — render analyzer output in human-readable form

Discovered strategies are validated by match play against a graded opponent population (random through perfect) and by agreement with engine-annotated best moves.

See [docs/plan.md](docs/plan.md) for the full project plan and architecture.

Component selections and evaluation-gate evidence (Gate A, Gate B): [docs/component-evaluation.md](docs/component-evaluation.md); decision records in [docs/adr/](docs/adr/).

## Status

Early development. The current milestone is the platform MVP: game framework, engine integration, corpus generation, annotation, persistence, and the strategy-evaluation harness. Heuristic mining and concept induction follow post-MVP.

## Building

```sh
cargo build
cargo test
```

Requires stable Rust (edition 2024).

## Usage

Each pipeline stage is a subcommand that reads and writes files, so stages can be re-run independently:

```sh
# 1. generate a corpus from a sweep config (7 roster strategies -> 49 pairings x 20 games = 980 games)
cargo run --release -- generate --config configs/tictactoe-default.toml --out runs/ttt-default

# 2. annotate every distinct position of that corpus with exhaustive-solver values and a game-player cross-check
cargo run --release -- annotate --corpus runs/ttt-default

# 3. summarize the corpus; --strict exits 2 when the diversity thresholds fail
cargo run --release -- analyze --corpus runs/ttt-default --strict

# tic-tac-toe only: annotate all 5478 reachable positions (a small-game accelerant)
cargo run --release -- annotate --exhaustive --game tictactoe --out runs/ttt-exhaustive
```

`generate` accepts `--serial` or `--threads N` (execution knobs only: outputs are byte-identical for the same config and seed). `analyze` accepts `--min-coverage`, `--min-decisive` and `--min-distinct` to override the default diversity thresholds (0.5 / 0.2 / 0.5). `annotate` accepts `--engine-depth` (default: the game's full search depth, 9 for tic-tac-toe). Each stage prints one summary line to stdout. `report` arrives with Phase 6.

A run directory contains:

| file | written by | contents |
| --- | --- | --- |
| `run.json` | generate | run metadata: `run_id` (hash of the resolved config), the resolved config, cells, totals |
| `games.jsonl` | generate | one `GameRecord` per game: cell, seed, strategy specs, action list, outcome, final state |
| `positions.jsonl` | generate | one `PositionRecord` per ply: state, canonical state and transform, legal actions, chosen action, who chose it |
| `annotations.jsonl` | annotate | one `AnnotationRecord` per distinct state: game-theoretic value, optimal actions, engine action, agreement |
| `annotate.json` | annotate | annotation metadata: mode, engine depth, counts, disagreements |
| `summary.json` | analyze | outcome distributions (by pairing, length, ply, first-move orbit) and the diversity metrics |

The sweep config format is the one in `configs/tictactoe-default.toml` (TOML: strategies, pairings, evaluators, openings, random opening plies, seed). Record schema and run-directory layout: [docs/adr/0009-corpus-record-schema.md](docs/adr/0009-corpus-record-schema.md).

## License

[MIT](LICENSE)
