//! Command-line entry points — one subcommand per pipeline stage.
//!
//! This module is the composition root: it is the only non-test place in the crate allowed to
//! name a concrete game. Everything else (`core`, `discovery`, `io`) stays generic over
//! `EngineGame + CorpusGame`; here a `--game` name (or a config/manifest's `game` field) is
//! resolved to a concrete [`crate::discovery::GameBundle`] before calling into the pipeline.

use crate::discovery::annotate::DEFAULT_SOLVER_LIMIT;
use crate::discovery::config::GenerateConfig;
use crate::discovery::{
    AnnotateMetadata, AnnotateMode, AnnotateOptions, CorpusSummary, DiversityThresholds, GenerateOptions, analyze_corpus,
    annotate_corpus, annotate_exhaustive, generate,
};
use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

/// Top-level command line.
#[derive(Parser, Debug)]
#[command(name = "strategy-discovery", version, about)]
pub struct Cli {
    /// Pipeline stage to run; omitted prints the crate name and version.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// One subcommand per pipeline stage.
#[derive(Subcommand, Debug)]
pub enum Command {
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
    /// Summarize a corpus into `summary.json`; `--strict` exits 2 when diversity fails.
    Analyze {
        /// Corpus run directory to summarize.
        #[arg(long)]
        corpus: PathBuf,
        /// Minimum fraction of known canonical positions the corpus must cover.
        #[arg(long)]
        min_coverage: Option<f64>,
        /// Minimum fraction of games that must end decisively.
        #[arg(long)]
        min_decisive: Option<f64>,
        /// Minimum fraction of games that must have a distinct action sequence.
        #[arg(long)]
        min_distinct: Option<f64>,
        /// Exit with status 2 when the corpus fails its diversity thresholds.
        #[arg(long)]
        strict: bool,
    },
}

/// The one field of a config or manifest needed to pick a game bundle.
#[derive(serde::Deserialize)]
struct GameName {
    game: String,
}

/// Error message for a `--game`/config `game` value that names no known game bundle.
fn unknown_game(name: &str) -> anyhow::Error {
    anyhow::anyhow!("unknown game `{name}`; known games: tictactoe")
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
    match cli.command {
        None => {
            println!("{} {}", crate::NAME, crate::VERSION);
            Ok(())
        }
        Some(Command::Generate {
            config,
            out,
            threads,
            serial,
        }) => run_generate(&config, &out, threads, serial),
        Some(Command::Annotate {
            corpus,
            exhaustive,
            game,
            out,
            engine_depth,
        }) => run_annotate(corpus, exhaustive, game, out, engine_depth),
        Some(Command::Analyze {
            corpus,
            min_coverage,
            min_decisive,
            min_distinct,
            strict,
        }) => run_analyze(&corpus, min_coverage, min_decisive, min_distinct, strict),
    }
}

/// Runs the `generate` subcommand.
fn run_generate(config_path: &Path, out: &Path, threads: Option<usize>, serial: bool) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(config_path).with_context(|| format!("reading config file `{}`", config_path.display()))?;
    let header: GameName = toml::from_str(&text).with_context(|| format!("parsing config file `{}`", config_path.display()))?;

    let options = GenerateOptions { threads, serial };

    match header.game.as_str() {
        "tictactoe" => {
            let config = GenerateConfig::<crate::games::tictactoe::Move>::from_toml_str(&text)?;
            let metadata = generate(&crate::games::tictactoe::game_bundle(), config, &options, out)?;
            println!(
                "run_id={} cells={} games={} positions={} out={}",
                metadata.run_id,
                metadata.cells.len(),
                metadata.games,
                metadata.positions,
                out.display()
            );
            Ok(())
        }
        other => Err(unknown_game(other)),
    }
}

/// Runs the `annotate` subcommand.
fn run_annotate(
    corpus: Option<PathBuf>,
    exhaustive: bool,
    game: Option<String>,
    out: Option<PathBuf>,
    engine_depth: Option<u32>,
) -> anyhow::Result<()> {
    let options = AnnotateOptions {
        engine_depth,
        solver_limit: DEFAULT_SOLVER_LIMIT,
    };

    let (mode, metadata) = if exhaustive {
        let game = game.ok_or_else(|| anyhow::anyhow!("--exhaustive requires --game"))?;
        let out = out.ok_or_else(|| anyhow::anyhow!("--exhaustive requires --out"))?;
        let metadata = match game.as_str() {
            "tictactoe" => annotate_exhaustive(&crate::games::tictactoe::game_bundle(), &out, &options)?,
            other => return Err(unknown_game(other)),
        };
        ("exhaustive", metadata)
    } else {
        let corpus = corpus.ok_or_else(|| anyhow::anyhow!("annotate requires --corpus (or --exhaustive)"))?;
        let run_path = corpus.join("run.json");
        let header: GameName =
            crate::io::read_json(&run_path).with_context(|| format!("reading run manifest `{}`", run_path.display()))?;
        let metadata = match header.game.as_str() {
            "tictactoe" => annotate_corpus(&crate::games::tictactoe::game_bundle(), &corpus, &options)?,
            other => return Err(unknown_game(other)),
        };
        ("corpus", metadata)
    };

    print_annotate_result(mode, &metadata);
    Ok(())
}

/// Prints the one-line `annotate` result summary.
fn print_annotate_result(mode: &str, metadata: &AnnotateMetadata) {
    debug_assert!(matches!(
        (mode, metadata.mode),
        ("corpus", AnnotateMode::Corpus) | ("exhaustive", AnnotateMode::Exhaustive)
    ));
    println!(
        "mode={} annotated={} terminal={} disagreements={}",
        mode, metadata.annotated, metadata.terminal, metadata.disagreements
    );
}

/// Runs the `analyze` subcommand.
fn run_analyze(
    corpus: &Path,
    min_coverage: Option<f64>,
    min_decisive: Option<f64>,
    min_distinct: Option<f64>,
    strict: bool,
) -> anyhow::Result<()> {
    let run_path = corpus.join("run.json");
    let header: GameName =
        crate::io::read_json(&run_path).with_context(|| format!("reading run manifest `{}`", run_path.display()))?;

    let mut thresholds = DiversityThresholds::default();
    if let Some(v) = min_coverage {
        thresholds.min_canonical_coverage = v;
    }
    if let Some(v) = min_decisive {
        thresholds.min_decisive_fraction = v;
    }
    if let Some(v) = min_distinct {
        thresholds.min_distinct_game_fraction = v;
    }

    let summary: CorpusSummary = match header.game.as_str() {
        "tictactoe" => analyze_corpus(&crate::games::tictactoe::game_bundle(), corpus, &thresholds)?,
        other => return Err(unknown_game(other)),
    };

    println!(
        "games={} distinct_games={} canonical_coverage={:.3} decisive_fraction={:.3} diversity_pass={}",
        summary.games,
        summary.distinct_games,
        summary.canonical_coverage.unwrap_or(0.0),
        summary.decisive_fraction,
        summary.diversity_pass
    );

    if strict && !summary.diversity_pass {
        std::process::exit(2);
    }

    Ok(())
}
