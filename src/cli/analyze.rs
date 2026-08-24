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
//!
//! The `--induce`/`--withhold-tier2`/`--induction-*`/`--label-map` flags compose an
//! [`InductionParams`] (each defaulting to [`InductionParams::default`]), validated by
//! [`induction_params`] immediately after `mine_params`, before anything else runs. Standalone
//! `analyze` reads induction parameters only from these flags.

use crate::cli::error::CliError;
use crate::cli::games::{KNOWN_GAMES, dispatch_game, game_of_run_dir};
use crate::discovery::analyze::{AnalyzeMetadata, AnalyzeOptions, AnalyzerOutput, analyze_outputs, builtin_registry};
use crate::discovery::config::CorpusError;
use crate::discovery::{CorpusSummary, DiversityThresholds, GameBundle};
use crate::io::{CorpusGame, InductionParams, MineParams};
use crate::strategy::engine::EngineGame;
use std::path::{Path, PathBuf};

/// Flags for `analyze`.
#[derive(clap::Args, Debug)]
pub struct AnalyzeArgs {
    /// Corpus run directory to analyze; required unless `--list-analyzers` is set.
    #[arg(long)]
    pub corpus: Option<PathBuf>,
    /// Game whose analyzer registry to use; defaults to the corpus's own game, or to the
    /// first known game when only listing analyzers.
    #[arg(long)]
    pub game: Option<String>,
    /// Comma-separated analyzer names to run, in order; defaults to `summary`.
    #[arg(long)]
    pub analyzers: Option<String>,
    /// Print one `name<TAB>description` line per registered analyzer and exit.
    #[arg(long)]
    pub list_analyzers: bool,
    /// Minimum fraction of known canonical positions the corpus must cover.
    #[arg(long)]
    pub min_coverage: Option<f64>,
    /// Minimum fraction of games that must end decisively.
    #[arg(long)]
    pub min_decisive: Option<f64>,
    /// Minimum fraction of games that must have a distinct action sequence.
    #[arg(long)]
    pub min_distinct: Option<f64>,
    /// Exit with status 3 when the corpus fails its diversity thresholds.
    #[arg(long)]
    pub strict: bool,
    /// Induction engine for the `dataset`/`mine` analyzers; defaults to `MineParams::default`.
    #[arg(long)]
    pub mine_engine: Option<String>,
    /// Comma-separated candidate depths for the `mine` analyzer; defaults to `MineParams::default`.
    #[arg(long)]
    pub mine_depths: Option<String>,
    /// Minimum rows per leaf for the `mine` analyzer; defaults to `MineParams::default`.
    #[arg(long)]
    pub mine_min_leaf: Option<usize>,
    /// Seed of the train/holdout shuffle for the `mine` analyzer; defaults to `MineParams::default`.
    #[arg(long)]
    pub mine_seed: Option<u64>,
    /// Fraction of rows held out for evaluation by the `mine` analyzer; defaults to `MineParams::default`.
    #[arg(long)]
    pub mine_holdout: Option<f64>,
    /// Enable concept induction for this run (sets `[induction] enabled = true`).
    #[arg(long)]
    pub induce: bool,
    /// Withhold the game-supplied tier-2 features from the dataset; requires `--induce`.
    #[arg(long)]
    pub withhold_tier2: bool,
    /// Maximum promotion rounds; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_rounds: Option<usize>,
    /// Thresholds minted per numeric atom; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_max_thresholds: Option<usize>,
    /// Level-1 predicates kept for combination; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_beam: Option<usize>,
    /// Shortlist size handed to the downstream probe; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_top_k: Option<usize>,
    /// Maximum concepts promoted per round; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_max_promoted: Option<usize>,
    /// Minimum train information-gain to shortlist a candidate; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_min_train_gain: Option<f64>,
    /// Minimum holdout information-gain to shortlist a candidate; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_min_holdout_gain: Option<f64>,
    /// Fraction of induction rows held out for evaluation; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_holdout: Option<f64>,
    /// Downstream CART probe depth limit; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_probe_depth: Option<usize>,
    /// Allowed soundness drop when the downstream probe's rule count shrinks; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_soundness_tolerance: Option<f64>,
    /// Required soundness rise when the downstream probe's rule count only ties; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_min_soundness_gain: Option<f64>,
    /// Compare against raw atoms during induction; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_compare_atoms: Option<bool>,
    /// Mint the extended mechanical tier-1 families when induction is enabled; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_extended_tier1: Option<bool>,
    /// Seed of the induction sampling; defaults to `InductionParams::default`.
    #[arg(long)]
    pub induction_seed: Option<u64>,
    /// TOML file mapping concept names to human labels, applied at render time.
    #[arg(long)]
    pub label_map: Option<PathBuf>,
}

/// Runs `analyze`.
pub(super) fn run(args: AnalyzeArgs) -> anyhow::Result<()> {
    let mine = mine_params(
        args.mine_engine.clone(),
        args.mine_depths.clone(),
        args.mine_min_leaf,
        args.mine_seed,
        args.mine_holdout,
    )?;
    let induction = induction_params(&args)?;
    if args.list_analyzers {
        let game = args.game.unwrap_or_else(|| KNOWN_GAMES[0].to_string());
        return dispatch_game!(game.as_str(), |bundle| list_analyzers_for(bundle));
    }
    let corpus = args
        .corpus
        .ok_or_else(|| CliError::Usage("analyze requires --corpus (or --list-analyzers)".to_string()))?;
    let game = match args.game {
        Some(name) => name,
        None => game_of_run_dir(&corpus)?,
    };
    let names: Vec<String> = match args.analyzers {
        Some(list) => list.split(',').map(|n| n.trim().to_string()).collect(),
        None => vec!["summary".to_string()],
    };
    let mut thresholds = DiversityThresholds::default();
    if let Some(v) = args.min_coverage {
        thresholds.min_canonical_coverage = v;
    }
    if let Some(v) = args.min_decisive {
        thresholds.min_decisive_fraction = v;
    }
    if let Some(v) = args.min_distinct {
        thresholds.min_distinct_game_fraction = v;
    }
    let options = AnalyzeOptions {
        analyzers: names,
        thresholds,
        strict: args.strict,
        mine,
        induction,
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

/// Composes an [`InductionParams`] from the `--induce`/`--withhold-tier2`/`--induction-*`/
/// `--label-map` flags, starting from [`InductionParams::default`] and overriding each field
/// given as `Some` (or set by its boolean flag). Validates the result via
/// [`InductionParams::validate`] before returning it.
fn induction_params(args: &AnalyzeArgs) -> Result<InductionParams, CorpusError> {
    let mut params = InductionParams::default();
    if args.induce {
        params.enabled = true;
    }
    if args.withhold_tier2 {
        params.withhold_tier2 = true;
    }
    if let Some(v) = args.induction_rounds {
        params.rounds = v;
    }
    if let Some(v) = args.induction_max_thresholds {
        params.max_thresholds = v;
    }
    if let Some(v) = args.induction_beam {
        params.beam = v;
    }
    if let Some(v) = args.induction_top_k {
        params.top_k = v;
    }
    if let Some(v) = args.induction_max_promoted {
        params.max_promoted = v;
    }
    if let Some(v) = args.induction_min_train_gain {
        params.min_train_gain = v;
    }
    if let Some(v) = args.induction_min_holdout_gain {
        params.min_holdout_gain = v;
    }
    if let Some(v) = args.induction_holdout {
        params.holdout_fraction = v;
    }
    if let Some(v) = args.induction_probe_depth {
        params.probe_depth = v;
    }
    if let Some(v) = args.induction_soundness_tolerance {
        params.soundness_tolerance = v;
    }
    if let Some(v) = args.induction_min_soundness_gain {
        params.min_soundness_gain = v;
    }
    if let Some(v) = args.induction_compare_atoms {
        params.compare_atoms = v;
    }
    if let Some(v) = args.induction_extended_tier1 {
        params.extended_tier1 = v;
    }
    if let Some(v) = args.induction_seed {
        params.seed = v;
    }
    if args.label_map.is_some() {
        params.label_map = args.label_map.clone();
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
