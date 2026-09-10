//! Command-line entry points — one subcommand per pipeline stage.
//!
//! Each subcommand lives in its own file (`play`, `generate`, `annotate`, `analyze`, `evaluate`,
//! `report`, `pipeline`, `discover`); [`games`] is the only non-test place in the crate allowed to name a concrete
//! game. The global `-v`/`--verbose` and `-q`/`--quiet` flags (see [`logging`]) control stderr
//! log verbosity for every subcommand.
//!
//! Process exit code: 0 success, 1 runtime failure, 2 invalid invocation or input, 3 a requested
//! check failed (`analyze --strict`, `evaluate --strict`, `discover` under `[discover] strict = true`); see [`error`] for the classification.

mod analyze;
mod annotate;
mod discover;
pub mod error;
mod evaluate;
pub mod games;
mod generate;
pub mod logging;
pub mod pipeline;
pub mod play;
pub mod report;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Top-level command line.
#[derive(Parser, Debug)]
#[command(name = "strategy-discovery", version, about)]
pub struct Cli {
    /// Increase stderr log verbosity: `-v` info, `-vv` debug, `-vvv` trace (default: warnings only).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Log errors only on stderr; conflicts with `--verbose`.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,
    /// Pipeline stage to run; omitted prints the crate name and version.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// One subcommand per pipeline stage.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Play games between named strategies and print a transcript.
    Play(play::PlayArgs),
    /// Generate a corpus from a TOML config into an output directory.
    Generate {
        /// Path to the sweep configuration TOML file.
        #[arg(long)]
        config: PathBuf,
        /// Directory to write `games.jsonl`, `positions.jsonl` and `run.json` into.
        #[arg(long)]
        out: PathBuf,
        /// Run each cell's games on a private pool of this many threads.
        #[arg(long)]
        threads: Option<usize>,
        /// Run each cell's games serially; takes precedence over `--threads`.
        #[arg(long)]
        serial: bool,
    },
    /// Annotate a corpus (`--corpus DIR`) or every reachable position of a game
    /// (`--exhaustive --game NAME --out DIR`).
    Annotate {
        /// Corpus run directory to annotate; required unless `--exhaustive` is set.
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Annotate every reachable position of `--game` instead of a generated corpus.
        #[arg(long)]
        exhaustive: bool,
        /// Game to enumerate exhaustively; required with `--exhaustive`.
        #[arg(long)]
        game: Option<String>,
        /// Directory to write `annotations.jsonl` and `annotate.json` into; required with `--exhaustive`.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Engine search depth for the cross-check; defaults to the bundle's full search depth.
        #[arg(long)]
        engine_depth: Option<u32>,
    },
    /// Run analyzers over a corpus run directory; `--strict` exits 3 when a check fails.
    Analyze(analyze::AnalyzeArgs),
    /// Evaluate strategies against the game's benchmark roster and archive the results.
    Evaluate(evaluate::EvaluateArgs),
    /// Render a Markdown report from a run directory or a single analyzer output file.
    Report(report::ReportArgs),
    /// Run a configured experiment end to end.
    Pipeline(pipeline::PipelineArgs),
    /// Run a configured experiment end to end, then evaluate and archive the mined heuristics.
    Discover(discover::DiscoverArgs),
}

/// Runs the CLI over the process arguments.
pub fn run() -> anyhow::Result<()> {
    run_from(std::env::args_os())
}

/// Runs the CLI over an explicit argument list (the entry point tests use).
pub fn run_from<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    logging::init(cli.verbose, cli.quiet);
    match cli.command {
        None => {
            println!("{} {}", crate::NAME, crate::VERSION);
            Ok(())
        }
        Some(Command::Play(args)) => play::run(args),
        Some(Command::Generate {
            config,
            out,
            threads,
            serial,
        }) => generate::run(&config, &out, threads, serial),
        Some(Command::Annotate {
            corpus,
            exhaustive,
            game,
            out,
            engine_depth,
        }) => annotate::run(corpus, exhaustive, game, out, engine_depth),
        Some(Command::Analyze(args)) => analyze::run(args),
        Some(Command::Evaluate(args)) => evaluate::run(args),
        Some(Command::Report(args)) => report::run(args),
        Some(Command::Pipeline(args)) => pipeline::run(args),
        Some(Command::Discover(args)) => discover::run(args),
    }
}
