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
- **play** — run one-off matches between named strategies and print a transcript; optionally writes a corpus run directory
- **pipeline** — run `generate -> annotate -> analyze -> report` end to end from one experiment config
- **evaluate** — score any strategy against the game's graded benchmark roster (loss rate versus the reference opponent, win/draw/loss rates per opponent, agreement with annotated best moves) and archive the result with provenance and novelty metadata

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

Every pipeline stage is a subcommand that reads and writes files, so stages can be re-run
independently:

```text
play                                        one-off matches between named strategies
generate -> annotate -> analyze -> report   the corpus pipeline, one command per stage
pipeline                                    all four stages from one experiment config
evaluate                                    score strategies against the benchmark roster and archive them
```

Each command prints its result — and nothing else — to stdout; logs and `error:` lines go
to stderr. For the same inputs, both stdout and every file a command writes are
byte-identical across runs.

### play

Plays games between named strategies and prints a transcript. `--players` takes one
strategy entry name per player, in turn order, resolved from the game's built-in roster
(`random`, `depth-1`, `depth-2`, `depth-3`, `depth-4`, `depth-6`, `perfect` for
tic-tac-toe) or from a `--strategies` roster file.

```sh
cargo run -- play --game tictactoe --players random,perfect --games 2 --seed 7
```

```text
game 0:0 seed=0x9d20c9ebd5dcd11a players=random,perfect opening_plies=0
 0. X 7
 1. O 8
 2. X 4
 3. O 1
 4. X 0
 5. O 6
 6. X 2
 7. O 3
 8. X 5
  XOX
  OXX
  OXO
outcome=Draw length=9

game 0:1 seed=0xd4c1951159fc36d5 players=random,perfect opening_plies=0
 0. X 4
 1. O 0
 2. X 8
 3. O 6
 4. X 7
 5. O 3
  O..
  OX.
  OXX
outcome=Win(O) length=6

games=2 wins=X:0,O:1 draws=1 unfinished=0
```

Flags:

- `--game <NAME>` and `--players <a,b,...>`, both required
- `--games N` (default 1), `--seed S` (default 0), `--max-plies M`
- `--opening '<json array>'` forced opening actions (tic-tac-toe: `--opening '[4,0]'`), `--random-opening-plies K` random plies played after it
- `--evaluator NAME` (defaults to the game bundle's default evaluator)
- `--strategies FILE` a TOML roster file (`schema_version` plus `[[strategies]]` entries, the same shape as a sweep config's strategy list)
- `--out DIR` also writes a corpus run directory (`run.json`, `games.jsonl`, `positions.jsonl`), byte-identical to `generate` on the equivalent sweep config and valid input for `annotate`/`analyze`/`report`
- `--serial` or `--threads N`, execution knobs only: outputs do not depend on them

### generate

Generates a corpus from a sweep config into a run directory.

```sh
cargo run -- generate --config tests/fixtures/generate-small.toml --out runs/small
```

```text
run_id=5eafa65f76637df3 cells=36 games=144 positions=1083 out=runs/small
```

Flags: `--config <TOML>` and `--out <DIR>`, both required; `--serial`; `--threads N`.
`run_id` is a hash of the resolved config, so a config with defaults omitted and one with
them spelled out share a `run_id` and produce byte-identical output. The sweep config
format is the one in `configs/tictactoe-default.toml` (TOML: strategies, pairings,
evaluators, openings, random opening plies, seed).

### annotate

Labels positions with exhaustive-solver values, optimal actions and a `game-player` engine
cross-check.

```sh
cargo run -- annotate --corpus runs/small
```

```text
mode=corpus annotated=390 terminal=0 disagreements=0
```

Flags: `--corpus <DIR>`, or `--exhaustive --game <NAME> --out <DIR>` to annotate every
reachable position of a game instead (a small-game accelerant: 5478 positions for
tic-tac-toe); `--engine-depth D` (defaults to the game's full search depth, 9 for
tic-tac-toe).

### analyze

Runs analyzers over a run directory. Analyzers live in a registry, so future miners plug
in without changing the command.

```sh
cargo run -- analyze --corpus runs/small --analyzers summary,agreement
```

```text
games=144 distinct_games=115 canonical_coverage=0.312 decisive_fraction=0.549 diversity_pass=false
```

```sh
cargo run -- analyze --list-analyzers
```

```text
agreement	engine-agreement rates of played actions, by strategy and ply (agreement.json)
summary	outcome distributions and corpus-diversity metrics (summary.json)
```

Flags:

- `--corpus <DIR>`, required unless `--list-analyzers` is given; `--game <NAME>` defaults to the corpus's own game
- `--analyzers <comma list>` names to run, in order (default `summary`); an unknown or repeated name is a usage error listing the registered names
- `--list-analyzers` prints one `name<TAB>description` line per registered analyzer and exits 0
- `--min-coverage`, `--min-decisive`, `--min-distinct` override the default diversity thresholds (0.5 / 0.2 / 0.5)
- `--strict` exits 3 when the corpus fails its diversity thresholds, listing the failures on stderr

The stdout line above is printed whenever the `summary` analyzer ran; when it did not, the
command prints `analyzers=<list> checks_pass=<bool> out=<dir>` instead.

### report

Renders Markdown from a run directory, or from a single analyzer output file.

```sh
cargo run -- report --input runs/small
```

A run-directory report opens with `# Run <run_id>` and a metadata table, then
`## Annotation` when `annotate.json` exists, then one section per entry of `analyze.json`
in manifest order (`## Summary`, `## Agreement`, ...), each rendered by its own analyzer.
Pointing `--input` at a single analyzer output file (`runs/small/summary.json`) renders
only that analyzer's section. `--out FILE` additionally writes the identical bytes to a
file.

### pipeline

Runs `generate -> annotate -> analyze -> report` from one experiment config, with file
handoffs in the run directory.

```sh
cargo run -- pipeline --experiment tests/fixtures/experiment-small.toml --out runs/experiment-small
```

```text
run_id=5eafa65f76637df3 cells=36 games=144 positions=1083 out=runs/experiment-small
mode=corpus annotated=390 terminal=0 disagreements=0
games=144 distinct_games=115 canonical_coverage=0.312 decisive_fraction=0.549 diversity_pass=true
report=runs/experiment-small/report.md bytes=3195
pipeline=experiment-small out=runs/experiment-small stages=generate,annotate,analyze,report
```

Flags: `--experiment <TOML>`, required; `--out DIR` overrides the experiment's own `out`;
`--stages <comma list>` runs a subset. Stages always run in the canonical order regardless
of the order listed, and a stage whose input files are missing fails with exit code 2,
naming the missing file and the stage to run first.

This run reports `diversity_pass=true` because `experiment-small.toml` sets its own
thresholds (0.25 / 0.2 / 0.5); a standalone `analyze` on the same corpus uses the defaults
(0.5 / 0.2 / 0.5) and reports `diversity_pass=false`.

### evaluate

Scores a strategy file against the game's built-in graded benchmark roster (or a
`--roster` file), playing every opponent from every seat, `--games` games per pairing.
With `--annotations`, each strategy's moves are also checked against the annotated
optimal actions for a move-agreement rate and a behavior signature. The headline loss
rate is measured against `--reference` (default the roster's last entry, `perfect`), and
every strategy is appended to a strategy archive with provenance and novelty metadata.

```sh
cargo run -- annotate --exhaustive --game tictactoe --out runs/exhaustive
cargo run -- evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations runs/exhaustive --out runs/eval
```

```text
strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000
strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535
strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051
evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=runs/eval
```

There are two line shapes: one `strategy=` line per strategy, with counts summed over
all pairings and seats (`games`, `wins`, `draws`, `losses`, `unfinished`), the headline
`loss_rate_vs_reference`, `agreement` (`none` when `--annotations` was not given), and
`novelty` (distance to the nearest archived entry, `1.000` for the first entry ever
archived); then one `evaluation=` line naming the run (`evaluation_id`, game, roster id,
reference, strategy count, and `archive_entries`, the archive's total entry count after
this run — so re-running the same command with the same archive appends nothing and
prints the same bytes).

Flags:

- `--game <NAME>` and `--strategies <FILE>`, both required; `--out <DIR>`, required
- `--archive <DIR>` archive location (default `<out>/archive`), to share one archive
  across evaluation runs
- `--annotations <DIR>` a corpus run directory from `annotate --corpus`, or an
  `annotate --exhaustive --out DIR` directory, to compute move agreement and a behavior
  signature
- `--roster <FILE>` a TOML roster file (`name`, `version`, `[[entries]]` with `name` and
  `[entries.spec]`) in place of the game's built-in benchmark roster
- `--reference <NAME>` the opponent the headline loss rate is measured against (default
  the roster's last entry, `perfect` for the tic-tac-toe benchmark roster)
- `--games N` games per pairing (default 20), `--seed S` (default 0), `--max-plies M`
- `--evaluator NAME` (defaults to the game bundle's default evaluator)
- `--strict` exit 3 if any strategy's loss rate against the reference exceeds
  `--max-loss-rate` (default 0.0)
- `--serial` or `--threads N`, execution knobs only: outputs do not depend on them

This writes `<out>/evaluation.json` (the full report) and appends one entry per strategy
to the archive at `--archive` (`archive.json`, an index, and `entries.jsonl`, one entry
per line with provenance and novelty metadata); re-running the same command against the
same archive appends nothing.

With `--strict`, if any strategy's loss rate against the reference exceeds
`--max-loss-rate`, the command exits 3 after everything above is written:

```text
error: check failed: reference loss rate exceeded: {name} {rate} > {max}
```

A `heuristic-rules` strategy entry is a usage error (exit 2, `strategy `heuristic-rules`
is not implemented: ...`) — its rule interpreter lands with heuristic mining — and is
never counted as a loss.

Harness contract: [docs/adr/0011-strategy-evaluation-harness.md](docs/adr/0011-strategy-evaluation-harness.md). Archive and novelty: [docs/adr/0012-strategy-archive-and-novelty.md](docs/adr/0012-strategy-archive-and-novelty.md). Adding a game: [docs/adding-a-game.md](docs/adding-a-game.md).

### Experiment config

Every relative path in an experiment file resolves against that file's own directory, never
the process's working directory; `--out` overrides `out` entirely. Complete worked examples
are `configs/tictactoe-experiment.toml` and `tests/fixtures/experiment-small.toml`.

```toml
schema_version = 1
name = "ttt-small"
game = "tictactoe"
out = "runs/ttt-small"          # run directory (created)

[generate]
sweep = "generate-small.toml"   # path to a sweep config, or an inline [generate.sweep] table
threads = 4                     # optional
serial = false                  # optional, default false

[annotate]
enabled = true                  # default true
engine_depth = 9                # optional

[analyze]
analyzers = ["summary", "agreement"]   # default ["summary"]
strict = false                         # default false

[analyze.thresholds]                   # optional, all-or-nothing; defaults 0.5 / 0.2 / 0.5
min_canonical_coverage = 0.25
min_decisive_fraction = 0.2
min_distinct_game_fraction = 0.5

[report]
out = "report.md"               # relative to `out`; default "report.md"
```

### Logging and verbosity

Logs are structured `tracing` events written to stderr only — no log line ever reaches
stdout or a persisted file, so a stage's stdout is byte-identical with and without them.

- `-v` info, `-vv` debug, `-vvv` trace; the default is warnings only
- `-q` errors only; conflicts with `-v`
- `RUST_LOG`, when set, overrides the flags with an `EnvFilter` directive (`RUST_LOG=off` silences everything)

Both flags are global: they may be given before or after the subcommand.

### Exit codes

| exit | class | when |
| --- | --- | --- |
| 0 | success | the stage completed; for `analyze --strict` and `pipeline`, all checks passed |
| 1 | failure | runtime error: I/O write failure, engine or strategy failure at play time, solver limit, internal invariant |
| 2 | usage | invalid invocation or input: bad flags, a missing or malformed config, an unknown game, strategy, evaluator or analyzer, a missing pipeline input, a `heuristic-rules` strategy (its interpreter is not implemented yet) |
| 3 | check | a requested check failed: `analyze --strict`, or `pipeline` with `strict = true`, on a corpus that misses its diversity thresholds, or `evaluate --strict` on a strategy whose loss rate against the reference exceeds `--max-loss-rate` |

Every usage error names the offending flag or field, its value, and the accepted values or
the fix:

```text
error: invalid configuration: unknown analyzer `bogus`; known analyzers: agreement, summary
error: precondition failed: runs/small/run.json not found; run the `generate` stage first
error: check failed: diversity thresholds not met: canonical_coverage 0.01 < 0.50; decisive_fraction 0.00 < 0.20; distinct_game_fraction 0.06 < 0.50
```

### Run directory

| file | written by | contents |
| --- | --- | --- |
| `run.json` | generate, play `--out` | run metadata: `run_id` (hash of the resolved config), the resolved config, cells, totals |
| `games.jsonl` | generate, play `--out` | one `GameRecord` per game: cell, seed, strategy specs, action list, outcome, final state |
| `positions.jsonl` | generate, play `--out` | one `PositionRecord` per ply: state, canonical state and transform, legal actions, chosen action, who chose it |
| `annotations.jsonl` | annotate | one `AnnotationRecord` per distinct state: game-theoretic value, optimal actions, engine action, agreement |
| `annotate.json` | annotate | annotation metadata: mode, engine depth, counts, disagreements |
| `summary.json` | analyze, `summary` analyzer | outcome distributions (by pairing, length, ply, first-move orbit) and the diversity metrics |
| `agreement.json` | analyze, `agreement` analyzer | engine-agreement rates of the actions actually played, overall and by strategy and by ply |
| `analyze.json` | analyze | the analyzer manifest: which analyzers ran, in order, and the file each wrote; the thresholds used; `checks_pass` |
| `report.md` | report `--out`, pipeline | the rendered Markdown report |
| `evaluation.json` | evaluate | the evaluation report: roster, config, and per-strategy tournament tallies, headline metrics, agreement and behavior signature |
| `archive/archive.json` | evaluate | strategy archive index: one `{entry_id, name, kind, sequence}` per archived strategy |
| `archive/entries.jsonl` | evaluate | one archive entry per line: spec, provenance, the full evaluation, novelty |

Record schema and run-directory layout: [docs/adr/0009-corpus-record-schema.md](docs/adr/0009-corpus-record-schema.md).
CLI contract, analyzer registry, experiment schema and exit codes: [docs/adr/0010-cli-contract-and-analyzer-registry.md](docs/adr/0010-cli-contract-and-analyzer-registry.md).
Evaluation harness, archive and novelty: [docs/adr/0011-strategy-evaluation-harness.md](docs/adr/0011-strategy-evaluation-harness.md), [docs/adr/0012-strategy-archive-and-novelty.md](docs/adr/0012-strategy-archive-and-novelty.md). Adding a game: [docs/adding-a-game.md](docs/adding-a-game.md).

## License

[MIT](LICENSE)
