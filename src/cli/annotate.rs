//! `annotate` subcommand: engine ground truth for a corpus (`--corpus DIR`) or for every
//! reachable position of a game (`--exhaustive --game NAME --out DIR`).

use crate::cli::error::CliError;
use crate::cli::games::{dispatch_game, game_of_run_dir};
use crate::discovery::annotate::DEFAULT_SOLVER_LIMIT;
use crate::discovery::{AnnotateMetadata, AnnotateMode, AnnotateOptions, annotate_corpus, annotate_exhaustive};
use std::path::PathBuf;

/// Runs `annotate --corpus DIR` or `annotate --exhaustive --game NAME --out DIR`.
pub(super) fn run(
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
        let game = game.ok_or_else(|| CliError::Usage("--exhaustive requires --game".to_string()))?;
        let out = out.ok_or_else(|| CliError::Usage("--exhaustive requires --out".to_string()))?;
        let metadata = dispatch_game!(game.as_str(), |bundle| annotate_exhaustive(bundle, &out, &options))?;
        ("exhaustive", metadata)
    } else {
        let corpus = corpus.ok_or_else(|| CliError::Usage("annotate requires --corpus (or --exhaustive)".to_string()))?;
        let game = game_of_run_dir(&corpus)?;
        let metadata = dispatch_game!(game.as_str(), |bundle| annotate_corpus(bundle, &corpus, &options))?;
        ("corpus", metadata)
    };

    print_result(mode, &metadata);
    Ok(())
}

/// Prints the one-line `annotate` result summary.
fn print_result(mode: &str, metadata: &AnnotateMetadata) {
    debug_assert!(matches!(
        (mode, metadata.mode),
        ("corpus", AnnotateMode::Corpus) | ("exhaustive", AnnotateMode::Exhaustive)
    ));
    println!(
        "mode={} annotated={} terminal={} disagreements={}",
        mode, metadata.annotated, metadata.terminal, metadata.disagreements
    );
}
