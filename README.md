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

Component selections and Gate A evidence: [docs/component-evaluation.md](docs/component-evaluation.md); decision records in [docs/adr/](docs/adr/).

## Status

Early development. The current milestone is the platform MVP: game framework, engine integration, corpus generation, annotation, persistence, and the strategy-evaluation harness. Heuristic mining and concept induction follow post-MVP.

## Building

```sh
cargo build
cargo test
```

Requires stable Rust (edition 2024).

## Usage

Once the CLI lands, each pipeline stage is a subcommand:

```sh
cargo run -- generate --config <file>
cargo run -- annotate ...
cargo run -- analyze ...
cargo run -- report --input <path>
```

## License

[MIT](LICENSE)
