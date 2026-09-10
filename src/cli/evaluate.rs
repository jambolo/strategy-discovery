//! `evaluate` subcommand: evaluates every strategy of a `--strategies` file against a game's
//! benchmark roster (or a `--roster` override), optionally measuring agreement with annotated
//! optimal actions (`--annotations DIR`), writes `evaluation.json` into `--out`, appends one
//! entry per strategy to the strategy archive (`--archive DIR`, default `<out>/archive`), prints
//! one frozen result line per strategy plus one summary line, and finally applies the
//! `--strict` loss-rate check (exit 3 after everything has been written).
//!
//! Write order: every strategy is evaluated before anything is written, so a runtime error
//! writes nothing.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::cli::error::CliError;
use crate::cli::games::dispatch_game;
use crate::discovery::{
    Appended, Archive, CorpusError, EvaluateConfig, GameBundle, NewEntry, StrategyFile, default_reference, evaluate_strategies,
    load_annotations, strict_failures,
};
use crate::io::{CorpusGame, EVALUATION_FILE, EvaluationReport, IoError, Provenance, write_json_pretty};
use crate::strategy::engine::EngineGame;
use crate::strategy::roster::Roster;

/// Flags for `evaluate`.
#[derive(clap::Args, Debug)]
pub struct EvaluateArgs {
    /// Game whose benchmark roster to evaluate against.
    #[arg(long)]
    pub game: String,
    /// TOML strategy file (`schema_version` plus `[[strategies]]` entries); every entry is evaluated in file order.
    #[arg(long)]
    pub strategies: PathBuf,
    /// Directory to write `evaluation.json` into (created).
    #[arg(long)]
    pub out: PathBuf,
    /// Strategy archive directory; defaults to `<out>/archive`.
    #[arg(long)]
    pub archive: Option<PathBuf>,
    /// Directory holding `annotations.jsonl` and `annotate.json`; enables agreement and behavior signatures.
    #[arg(long)]
    pub annotations: Option<PathBuf>,
    /// TOML roster file overriding the game's built-in benchmark roster.
    #[arg(long)]
    pub roster: Option<PathBuf>,
    /// Roster entry the headline loss rate is measured against; defaults to the last roster entry.
    #[arg(long)]
    pub reference: Option<String>,
    /// Games per (opponent, seat) pairing.
    #[arg(long, default_value_t = 20)]
    pub games: usize,
    /// Master seed for the tournaments and the agreement pass.
    #[arg(long, default_value_t = 0)]
    pub seed: u64,
    /// Abort a game after this many plies (tallied as unfinished, never as a loss).
    #[arg(long)]
    pub max_plies: Option<usize>,
    /// Evaluator name; defaults to the game bundle's default evaluator.
    #[arg(long)]
    pub evaluator: Option<String>,
    /// Exit with status 3 when any strategy's loss rate against the reference exceeds `--max-loss-rate`.
    #[arg(long)]
    pub strict: bool,
    /// Loss-rate threshold for `--strict`.
    #[arg(long, default_value_t = 0.0)]
    pub max_loss_rate: f64,
    /// Play every game serially; takes precedence over `--threads`.
    #[arg(long)]
    pub serial: bool,
    /// Play games on a private pool of this many threads.
    #[arg(long)]
    pub threads: Option<usize>,
}

/// Runs `evaluate`.
pub(super) fn run(args: EvaluateArgs) -> anyhow::Result<()> {
    let game = args.game.clone();
    dispatch_game!(game.as_str(), |bundle| evaluate_for(bundle, &args))
}

/// Evaluates the strategies of `args.strategies` for `bundle`, writes the outputs, prints the
/// result lines, then applies the strict check.
fn evaluate_for<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, args: &EvaluateArgs) -> anyhow::Result<()> {
    if args.threads == Some(0) {
        return Err(CorpusError::Config("--threads must be >= 1, got 0".to_string()).into());
    }

    let strategies = StrategyFile::from_toml_file(&args.strategies)?.strategies;
    if strategies.is_empty() {
        return Err(CorpusError::Config(format!(
            "strategies file {} has no [[strategies]] entries",
            args.strategies.display()
        ))
        .into());
    }

    let roster_override = match &args.roster {
        Some(path) => Some(load_roster(path)?),
        None => None,
    };
    let roster = roster_override.as_ref().unwrap_or(&bundle.roster);

    let reference = match &args.reference {
        Some(name) => name.clone(),
        None => default_reference(roster)?,
    };

    let annotations = match &args.annotations {
        Some(dir) => Some(load_annotations(bundle, dir)?),
        None => None,
    };

    let config = EvaluateConfig {
        evaluator: args.evaluator.clone().unwrap_or_else(|| bundle.default_evaluator.clone()),
        games_per_pairing: args.games,
        seed: args.seed,
        max_plies: args.max_plies,
        reference,
        threads: args.threads,
        serial: args.serial,
    };

    let report = evaluate_strategies(bundle, &strategies, roster_override.as_ref(), annotations.as_ref(), &config)
        .with_context(|| format!("evaluating strategies from {}", args.strategies.display()))?;

    std::fs::create_dir_all(&args.out).map_err(|source| IoError::Io {
        path: args.out.clone(),
        source,
    })?;
    write_json_pretty(&args.out.join(EVALUATION_FILE), &report)?;

    let archive_dir = args.archive.clone().unwrap_or_else(|| args.out.join("archive"));
    let mut archive = Archive::open(&archive_dir, &bundle.name)?;
    let mut novelties: Vec<f64> = Vec::with_capacity(report.strategies.len());
    for evaluation in &report.strategies {
        let appended = archive.append(NewEntry {
            name: evaluation.name.clone(),
            spec: evaluation.spec.clone(),
            provenance: provenance(&report, &config),
            evaluation: evaluation.clone(),
        })?;
        let id = match appended {
            Appended::New(id) | Appended::Duplicate(id) => id,
        };
        let entry = archive
            .get(&id)
            .ok_or_else(|| CorpusError::Config(format!("archive entry `{id}` vanished after append")))?;
        novelties.push(entry.novelty.distance);
    }

    print_result(&report, &novelties, archive.len(), &args.out);

    let failures = strict_failures(&report, args.max_loss_rate);
    if args.strict && !failures.is_empty() {
        return Err(CliError::CheckFailed(failures.join("; ")).into());
    }
    tracing::info!(strategies = report.strategies.len(), "evaluated");
    Ok(())
}

/// Reads and validates a roster TOML file (`name`, `version`, `[[entries]]`).
fn load_roster(path: &Path) -> Result<Roster, CorpusError> {
    let text = std::fs::read_to_string(path).map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let roster: Roster = toml::from_str(&text).map_err(|e| IoError::Toml {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    roster
        .validate()
        .map_err(|e| CorpusError::Config(format!("roster file {}: {e}", path.display())))?;
    Ok(roster)
}

/// Provenance shared by every archive entry of one `evaluate` run.
fn provenance(report: &EvaluationReport, config: &EvaluateConfig) -> Provenance {
    Provenance {
        game: report.game.clone(),
        source: "evaluate".to_string(),
        evaluation_id: report.evaluation_id.clone(),
        roster_id: report.roster.id(),
        seed: config.seed,
        games_per_pairing: config.games_per_pairing,
        evaluator: config.evaluator.clone(),
        corpus_run_id: None,
        annotations_run_id: report.config.annotations.as_ref().and_then(|a| a.run_id.clone()),
        annotations_mode: report.config.annotations.as_ref().map(|a| a.mode.clone()),
        discovery: None,
    }
}

/// Prints the frozen result lines: one `strategy=` line per evaluated strategy, then the
/// `evaluation=` summary line. Shared by `evaluate` and `discover`.
pub(crate) fn print_result(report: &EvaluationReport, novelties: &[f64], archive_entries: usize, out: &Path) {
    for (evaluation, novelty) in report.strategies.iter().zip(novelties) {
        let agreement = match evaluation.headline.agreement_rate {
            Some(rate) => format!("{rate:.3}"),
            None => "none".to_string(),
        };
        println!(
            "strategy={} kind={} games={} wins={} draws={} losses={} unfinished={} loss_rate_vs_reference={:.3} agreement={} novelty={:.3}",
            evaluation.name,
            evaluation.kind,
            evaluation.tournament.totals.games,
            evaluation.tournament.totals.wins,
            evaluation.tournament.totals.draws,
            evaluation.tournament.totals.losses,
            evaluation.tournament.totals.unfinished,
            evaluation.headline.loss_rate_vs_reference,
            agreement,
            novelty
        );
    }
    println!(
        "evaluation={} game={} roster={} reference={} strategies={} archive_entries={} out={}",
        report.evaluation_id,
        report.game,
        report.roster.id(),
        report.config.reference,
        report.strategies.len(),
        archive_entries,
        out.display()
    );
}
