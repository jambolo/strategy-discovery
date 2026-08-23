//! `analyze` subcommand: runs the analyzer registry over a corpus run directory.
//!
//! `--list-analyzers` prints the analyzers registered for `--game` (or the first
//! [`KNOWN_GAMES`] entry when `--game` is omitted) and exits without touching a corpus.
//! Otherwise `--corpus DIR` is required; the requested `--analyzers` (default `summary`) run
//! in order via [`analyze_outputs`], each writing its own output file plus the shared
//! `analyze.json` manifest into `DIR`. [`run_analyze`] is shared with the `pipeline` command.
//!
//! The `--mine-engine`/`--mine-depths`/`--mine-min-leaf`/`--mine-seed`/`--mine-holdout` flags
//! compose a [`MineParams`] (each defaulting to [`MineParams::default`]), validated by
//! [`mine_params`] before anything else runs, so a bad value always exits 2.

use crate::cli::error::CliError;
use crate::cli::games::{KNOWN_GAMES, dispatch_game, game_of_run_dir};
use crate::discovery::analyze::{AnalyzeMetadata, AnalyzeOptions, AnalyzerOutput, analyze_outputs, builtin_registry};
use crate::discovery::config::CorpusError;
use crate::discovery::{CorpusSummary, DiversityThresholds, GameBundle};
use crate::io::{CorpusGame, MineParams};
use crate::strategy::engine::EngineGame;
use std::path::{Path, PathBuf};

/// Runs `analyze`.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    corpus: Option<PathBuf>,
    game: Option<String>,
    analyzers: Option<String>,
    list_analyzers: bool,
    min_coverage: Option<f64>,
    min_decisive: Option<f64>,
    min_distinct: Option<f64>,
    strict: bool,
    mine_engine: Option<String>,
    mine_depths: Option<String>,
    mine_min_leaf: Option<usize>,
    mine_seed: Option<u64>,
    mine_holdout: Option<f64>,
) -> anyhow::Result<()> {
    let mine = mine_params(mine_engine, mine_depths, mine_min_leaf, mine_seed, mine_holdout)?;
    if list_analyzers {
        let game = game.unwrap_or_else(|| KNOWN_GAMES[0].to_string());
        return dispatch_game!(game.as_str(), |bundle| list_analyzers_for(bundle));
    }
    let corpus = corpus.ok_or_else(|| CliError::Usage("analyze requires --corpus (or --list-analyzers)".to_string()))?;
    let game = match game {
        Some(name) => name,
        None => game_of_run_dir(&corpus)?,
    };
    let names: Vec<String> = match analyzers {
        Some(list) => list.split(',').map(|n| n.trim().to_string()).collect(),
        None => vec!["summary".to_string()],
    };
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
    let options = AnalyzeOptions {
        analyzers: names,
        thresholds,
        strict,
        mine,
    };
    dispatch_game!(game.as_str(), |bundle| run_analyze(bundle, &corpus, &options).map(|_| ()))
}

/// Composes a [`MineParams`] from the `--mine-*` flags, starting from [`MineParams::default`]
/// and overriding each field given as `Some`; a malformed `--mine-depths` entry (a fragment
/// that doesn't parse as `usize`) is a [`CorpusError::Config`] naming that fragment. Validates
/// the result via [`MineParams::validate`] before returning it.
fn mine_params(
    engine: Option<String>,
    depths: Option<String>,
    min_leaf: Option<usize>,
    seed: Option<u64>,
    holdout: Option<f64>,
) -> Result<MineParams, CorpusError> {
    let mut params = MineParams::default();
    if let Some(engine) = engine {
        params.engine = engine;
    }
    if let Some(depths) = depths {
        let mut parsed = Vec::new();
        for part in depths.split(',') {
            let part = part.trim();
            let depth: usize = part
                .parse()
                .map_err(|_| CorpusError::Config(format!("--mine-depths: `{part}` is not a depth")))?;
            parsed.push(depth);
        }
        params.depths = parsed;
    }
    if let Some(min_leaf) = min_leaf {
        params.min_leaf = min_leaf;
    }
    if let Some(seed) = seed {
        params.seed = seed;
    }
    if let Some(holdout) = holdout {
        params.holdout_fraction = holdout;
    }
    params.validate()?;
    Ok(params)
}

/// Prints one `name<TAB>description` line per analyzer registered for `G`.
fn list_analyzers_for<G: EngineGame + CorpusGame>(_bundle: &GameBundle<G>) -> anyhow::Result<()> {
    for (name, analyzer) in builtin_registry::<G>().iter() {
        println!("{name}\t{}", analyzer.description());
    }
    Ok(())
}

/// Runs every analyzer in `options` over the corpus at `corpus`, prints the result line, and
/// fails with [`CliError::CheckFailed`] when `options.strict` and a check failed. Shared with
/// the `pipeline` command.
pub(crate) fn run_analyze<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    corpus: &Path,
    options: &AnalyzeOptions,
) -> anyhow::Result<AnalyzeMetadata> {
    let registry = builtin_registry::<G>();
    let (metadata, outputs) = analyze_outputs(bundle, corpus, &registry, options)?;
    let summary = summary_of(&metadata, &outputs);
    print_result(&metadata, corpus, summary.as_ref());
    if options.strict && !metadata.checks_pass {
        return Err(CliError::CheckFailed(check_message(&metadata, &outputs, summary.as_ref())).into());
    }
    Ok(metadata)
}

/// The parsed `summary` analyzer output, if `summary` was one of the analyzers run.
fn summary_of(metadata: &AnalyzeMetadata, outputs: &[AnalyzerOutput]) -> Option<CorpusSummary> {
    let i = metadata.analyzers.iter().position(|a| a.name == "summary")?;
    serde_json::from_str::<CorpusSummary>(&outputs[i].json).ok()
}

/// Prints the `analyze` result line: the frozen `summary`-shaped line when `summary` ran,
/// otherwise a generic line naming the analyzers run and whether their checks passed.
fn print_result(metadata: &AnalyzeMetadata, corpus: &Path, summary: Option<&CorpusSummary>) {
    match summary {
        Some(summary) => {
            println!(
                "games={} distinct_games={} canonical_coverage={:.3} decisive_fraction={:.3} diversity_pass={}",
                summary.games,
                summary.distinct_games,
                summary.canonical_coverage.unwrap_or(0.0),
                summary.decisive_fraction,
                summary.diversity_pass
            );
        }
        None => {
            let names: Vec<&str> = metadata.analyzers.iter().map(|a| a.name.as_str()).collect();
            println!(
                "analyzers={} checks_pass={} out={}",
                names.join(","),
                metadata.checks_pass,
                corpus.display()
            );
        }
    }
}

/// Builds the `--strict` failure message: the diversity-failure list when `summary` ran,
/// otherwise the names of the analyzers whose own check failed.
fn check_message(metadata: &AnalyzeMetadata, outputs: &[AnalyzerOutput], summary: Option<&CorpusSummary>) -> String {
    match summary {
        Some(summary) => format!("diversity thresholds not met: {}", summary.diversity_failures.join("; ")),
        None => {
            let failing: Vec<&str> = metadata
                .analyzers
                .iter()
                .zip(outputs)
                .filter(|(_, output)| !output.checks_pass)
                .map(|(entry, _)| entry.name.as_str())
                .collect();
            format!("analyzers reported a failed check: {}", failing.join(", "))
        }
    }
}
