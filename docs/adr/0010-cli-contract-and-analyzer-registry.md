# ADR 0010: CLI contract and analyzer registry

## Status

Accepted — 2026-08-22 (Phase 6)

## Context

- `docs/plan.md` Phase 6 requires one CLI command per pipeline stage (`generate -> annotate ->
  analyze -> report`) composed via file handoffs, a pluggable analyzer registry so future
  miners slot in without restructuring, typed experiment configuration, structured logging and
  a documented error/exit-code taxonomy. This ADR records that contract as implemented.

- ADR 0003 fixed TOML-in/JSON-out and `schema_version` on every persisted document; ADR 0007
  selected JSONL as the corpus record format; ADR 0009 fixed the record schema, the run-directory
  layout and the byte-identity rule. This ADR extends byte-identity to the new analyzer,
  manifest and report outputs and to CLI stdout, and layers the command surface on top of the
  ADR 0009 run directory.

- Before Phase 6 the CLI repeated its game dispatch in four places and had no `report`, no
  `pipeline` and no analyzer registry; adding a game or an analyzer touched multiple files.

## Decision

- Command set: `play`, `generate`, `annotate`, `analyze`, `report`, `pipeline`. `discover` is
  deliberately not added (reserved for a later phase). `play` has no interactive mode. No
  subcommand prints `strategy-discovery 0.1.0` and exits 0.

- stdout/stderr split: stdout carries only the stage's result; logs and `error: {message}` lines
  go to stderr. Frozen stdout lines: `generate` prints
  `run_id={} cells={} games={} positions={} out={}`; `annotate` prints
  `mode={corpus|exhaustive} annotated={} terminal={} disagreements={}`; `analyze` prints
  `games={} distinct_games={} canonical_coverage={:.3} decisive_fraction={:.3} diversity_pass={}`
  when the `summary` analyzer ran, and `analyzers={} checks_pass={} out={}` when it did not;
  `play` ends with `games={N} wins=X:{n},O:{m} draws={d} unfinished={u}`; `pipeline` prints each
  stage's line, then `report={path} bytes={n}`, then `pipeline={name} out={dir} stages={list}`.

- Single-point game dispatch: `src/cli/games.rs` exports `KNOWN_GAMES: &[&str]`, the
  `dispatch_game!(name, |bundle| expr)` macro, `game_of_toml_file` and `game_of_run_dir`. It is
  the only non-test file outside `src/games/` that names a concrete game; adding a game is one
  macro arm plus one `KNOWN_GAMES` entry. `src/main.rs` is bootstrap plus exit-code mapping only
  (11 lines).

- `src/cli/` module layout: `mod.rs` (clap `Cli`/`Command`, `run`, `run_from`), `games.rs`,
  `logging.rs`, `error.rs`, `play.rs`, `generate.rs`, `annotate.rs`, `analyze.rs`, `report.rs`,
  `pipeline.rs`.

- Analyzer registry (`src/discovery/analyze.rs`, game-agnostic): trait `Analyzer<G>: Send +
  Sync` with `name()`, `description()`, `requires_annotations()`,
  `run(&AnalyzeContext<G>, &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError>` and
  `render(&serde_json::Value) -> Result<String, CorpusError>`. `AnalyzerRegistry<G>` is a
  `BTreeMap<String, Arc<dyn Analyzer<G>>>` with `register` (replaces same name), `get`, `names()`
  (sorted); `builtin_registry::<G>()` provides `summary` and `agreement`. `AnalyzerOutput { file,
  json, checks_pass }` carries the exact file bytes (`serde_json::to_string_pretty` plus a
  trailing newline), not a `serde_json::Value`, because a `Value` round-trip would reorder keys
  and break the pinned `summary.json` hash. `render` output starts with `## {Title}` and uses
  `###` for subsections, so `report` concatenates sections without knowing any analyzer.

- Analyzer manifest: `analyze.json` (`ANALYZE_FILE` in `src/io/schema.rs`) =
  `AnalyzeMetadata { schema_version, game, run_id, analyzers: [{ name, file }], thresholds,
  strict, checks_pass }`, written on every `analyze` run, listing the analyzers in run order with
  the file each wrote. `report --input <dir>` renders one section per manifest entry, in
  manifest order. Built-ins: `summary` -> `summary.json` (unchanged bytes versus the previous
  phase); `agreement` -> `agreement.json`, which joins each `PositionRecord` to the
  `AnnotationRecord` with an equal raw `state` (a canonical join would silently inflate
  agreement) and reports `positions`/`agreeing`/`rate` overall, by `chosen_by` strategy key and
  by ply.

- Experiment config (`src/discovery/experiment.rs`): `ExperimentConfig<A>` with
  `schema_version`, `name`, `game`, `out`, and `[generate]` (required; `sweep` is either a path
  string or an inline table, an `#[serde(untagged)] SweepSource<A>`), `[annotate]`, `[analyze]`
  (plus an all-or-nothing `[analyze.thresholds]`) and `[report]` sections. `load_experiment`
  parses; `resolve_experiment(config, base_dir, bundle, registry, out_override)` validates and
  reduces to `ResolvedExperiment`. Relative-path rule: the sweep path, `out` and `[report] out`
  all resolve against the experiment file's own parent directory, never the process's working
  directory; a CLI `--out` replaces `out` outright rather than being joined with it. `pipeline`
  runs the stages in the fixed order `generate -> annotate -> analyze -> report` through the
  same library functions the standalone commands use, regardless of the order `--stages` lists
  them. `StrategyFile { schema_version, strategies }` is the standalone `[[strategies]]` roster
  file `play --strategies` loads.

- Exit codes — verbatim; `analyze --strict` previously exited 2 and now exits 3, the one
  externally breaking change of this ADR:

  | exit | class | when |
  | --- | --- | --- |
  | 0 | success | the stage completed; for `analyze --strict` and `pipeline`, all checks passed |
  | 1 | failure | runtime error: I/O write failure, engine or strategy failure at play time, solver limit, internal invariant |
  | 2 | usage | invalid invocation or input: clap parse errors, `CorpusError::Config`, `CorpusError::Precondition`, `IoError::{Toml, SchemaVersion, Missing, Invalid}`, a not-found input path, `StrategyError::Unimplemented` |
  | 3 | check | a requested check failed: `analyze --strict`, or `pipeline` with `strict = true`, on a corpus that misses its diversity thresholds |

  `cli::error::exit_code(&anyhow::Error)` classifies by downcasting the error chain; `main.rs`
  prints `error: {err:#}` to stderr and returns `ExitCode::from(exit_code(&err))`.
  `CorpusError::Precondition(String)` (`"precondition failed: {0}"`) was added for "required
  input missing; run X first". Every `Config`/`Precondition` message names the offending field
  or flag, its value, and the accepted values or the fix; unknown game/strategy/evaluator/
  analyzer messages list the known names.

- Logging (`src/cli/logging.rs`): a `tracing_subscriber::fmt` subscriber on stderr with
  `with_ansi(false)`, `with_target(false)` and no timestamps; level from `-v` (info/debug/trace
  by count) or `-q` (errors only), overridden by `RUST_LOG` when set; `tracing-subscriber`
  gained the `env-filter` feature (the only dependency change of the phase). `init` uses
  `try_init` and ignores an already-initialized error so `run_from` can be called repeatedly
  in-process. Both flags are `global = true`.

- Byte-identity extension (on top of ADR 0009): for the same inputs, `play` stdout and its
  `--out` files, every analyzer output file, `analyze.json`, `report` stdout and its `--out`
  file, and every `pipeline` stage file are byte-identical across runs. stderr (logs) is exempt.
  All maps are `BTreeMap`; report floats print at fixed precision (`{:.3}`) so the bytes are
  stable across platforms. Evidence: `artifacts/reproducibility/cli-outputs-determinism.md`.

## Consequences

- A new game touches only `src/cli/games.rs` plus its own `src/games/**`; no other pipeline,
  corpus, annotation or evaluation code names a concrete game.

- A new analyzer is a `register` call plus a manifest entry; `report` picks it up with no code
  change because it renders one section per `analyze.json` entry in manifest order.

- Scripts that treated `analyze --strict`'s exit 2 as "diversity failed" must move to 3; exit 2
  now means only "you invoked it wrong."

- The `analyze.json` manifest makes a run directory self-describing, so `report` needs no
  analyzer list of its own and can render any registry's output without a code change.

- The stdout/stderr split means a stage's stdout can be hashed or piped safely regardless of
  verbosity, since logs never share the stream.

- The experiment config's relative-path rule (paths resolve against the experiment file's own
  directory) means an experiment file is portable across working directories and CI checkouts
  without embedding absolute paths.

- Evidence: `tests/cli_*.rs` integration tests; `artifacts/reproducibility/cli-outputs-determinism.md`.
