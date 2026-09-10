//! `discover` subcommand: runs a configured experiment end to end and mines it for heuristics.
//!
//! The chain is: the four experiment stages (`generate -> annotate -> analyze -> report`, run
//! via [`crate::cli::pipeline::run_stages`] and printed byte-identically to `pipeline`), then the
//! library orchestration [`crate::discovery::run_discover`], which evaluates the mined
//! candidates, archives them, and writes `evaluation.json` and `discover.json`. Before running,
//! the required analyzers are appended to the experiment's `[analyze] analyzers` list if not
//! already present, and their relative order is validated: `dataset`, `mine` when induction is
//! disabled; `dataset`, `concepts`, `mine`, `vocabulary` when `--induce` or
//! `[induction] enabled = true` turns concept induction on. A relative-order violation is an
//! invalid config (exit 2).
//!
//! `--induce` and `--withhold-tier2` force `[induction] enabled`/`[induction] withhold_tier2`
//! to `true` in the loaded experiment config, applied AFTER `config_hash` is computed (the hash
//! stays "config as loaded").
//!
//! Output order after the four stage lines: when induction is enabled, one `concepts=` summary
//! line (read back from `concepts.json`), then the same `strategy=`/`evaluation=` result lines
//! `evaluate` prints (via [`crate::cli::evaluate::print_result`]), then one final `discover=`
//! summary line. The strict loss-rate check runs LAST, after `evaluation.json` and `discover.json`
//! are already on disk, so a strict failure never suppresses a write.
//!
//! Exit codes: 0 success; 1 runtime failure; 2 invalid config or a missing precondition file;
//! 3 the `--strict`-equivalent `[discover] strict = true` loss-rate check failed.

use std::path::PathBuf;

use crate::cli::error::CliError;
use crate::cli::games::{dispatch_game, game_of_toml_file};
use crate::discovery::config::CorpusError;
use crate::discovery::{GameBundle, builtin_registry, load_experiment, resolve_experiment, run_discover, strict_failures};
use crate::io::CorpusGame;
use crate::io::{CONCEPTS_FILE, ConceptReport, config_hash, read_json};
use crate::strategy::engine::EngineGame;

/// Flags for `discover`.
#[derive(clap::Args, Debug)]
pub struct DiscoverArgs {
    /// Experiment TOML file to run.
    #[arg(long)]
    pub config: PathBuf,
    /// Run directory; overrides the experiment's own `out`, exactly like `pipeline --out`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Force `[induction] enabled = true` in the loaded experiment config (applied after
    /// `config_hash` is computed).
    #[arg(long)]
    pub induce: bool,
    /// Force `[induction] withhold_tier2 = true` in the loaded experiment config (applied after
    /// `config_hash` is computed).
    #[arg(long)]
    pub withhold_tier2: bool,
}

/// Runs `discover`.
pub(super) fn run(args: DiscoverArgs) -> anyhow::Result<()> {
    let game = game_of_toml_file(&args.config)?;
    dispatch_game!(game.as_str(), |bundle| run_for(bundle, &args))
}

/// Runs the whole discovery chain for `bundle` over the experiment at `args.config`.
fn run_for<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, args: &DiscoverArgs) -> anyhow::Result<()> {
    let mut config = load_experiment::<G::Action>(&args.config)?;
    let hash = config_hash(&config)?;

    if args.induce {
        config.induction.enabled = true;
    }
    if args.withhold_tier2 {
        config.induction.withhold_tier2 = true;
    }

    force_analyzers(&mut config.analyze.analyzers, config.induction.enabled);
    validate_analyzer_order(&config.analyze.analyzers, config.induction.enabled)?;

    let base_dir = args.config.parent().unwrap_or_else(|| std::path::Path::new("."));
    let registry = builtin_registry::<G>();
    let resolved = resolve_experiment(config, base_dir, bundle, &registry, args.out.as_deref())?;

    crate::cli::pipeline::run_stages(bundle, &resolved, &registry, &["generate", "annotate", "analyze", "report"])?;

    if resolved.analyze.induction.enabled {
        let report: ConceptReport = read_json(&resolved.out.join(CONCEPTS_FILE))?;
        let evaluated: usize = report.rounds.iter().map(|round| round.scored).sum();
        println!(
            "concepts={} evaluated={} rounds={} withhold_tier2={}",
            report.promoted.len(),
            evaluated,
            report.rounds.len(),
            report.params.withhold_tier2
        );
    }

    let outcome = run_discover(bundle, &resolved, &hash)?;

    crate::cli::evaluate::print_result(&outcome.report, &outcome.novelties, outcome.archive_entries, &resolved.out);

    println!(
        "discover={} run_id={} heuristics={} archive_entries={} out={}",
        resolved.name,
        outcome.manifest.run_id,
        outcome.manifest.candidates.len(),
        outcome.archive_entries,
        resolved.out.display()
    );

    let failures = strict_failures(&outcome.report, resolved.discover.max_loss_rate);
    if resolved.discover.strict && !failures.is_empty() {
        return Err(CliError::CheckFailed(failures.join("; ")).into());
    }
    Ok(())
}

/// Analyzer names required in `[analyze] analyzers`, in required order, for `induction_enabled`.
fn required_analyzers(induction_enabled: bool) -> &'static [&'static str] {
    if induction_enabled {
        &["dataset", "concepts", "mine", "vocabulary"]
    } else {
        &["dataset", "mine"]
    }
}

/// Appends each analyzer required for `induction_enabled` that is not already present in
/// `analyzers`, in required order, so mining (and induction, when enabled) always runs
/// regardless of the experiment's own analyzer selection.
fn force_analyzers(analyzers: &mut Vec<String>, induction_enabled: bool) {
    for name in required_analyzers(induction_enabled) {
        if !analyzers.iter().any(|existing| existing == name) {
            analyzers.push((*name).to_string());
        }
    }
}

/// Validates that the required analyzers for `induction_enabled` appear in `analyzers` in
/// strictly increasing relative order. Returns a config error naming the required order on
/// violation.
fn validate_analyzer_order(analyzers: &[String], induction_enabled: bool) -> Result<(), CorpusError> {
    let required = required_analyzers(induction_enabled);
    let positions: Vec<usize> = required
        .iter()
        .filter_map(|name| analyzers.iter().position(|existing| existing == name))
        .collect();
    let ordered = positions.windows(2).all(|pair| pair[0] < pair[1]);
    if ordered {
        Ok(())
    } else {
        Err(CorpusError::Config(
            "[analyze] analyzers: required order is dataset < concepts < mine < vocabulary".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_analyzers_disabled_appends_dataset_mine() {
        let mut analyzers = vec!["summary".to_string()];
        force_analyzers(&mut analyzers, false);
        assert_eq!(
            analyzers,
            vec!["summary".to_string(), "dataset".to_string(), "mine".to_string()]
        );
        assert!(validate_analyzer_order(&analyzers, false).is_ok());
    }

    #[test]
    fn forced_analyzers_enabled_appends_full_chain() {
        let mut analyzers = vec!["summary".to_string()];
        force_analyzers(&mut analyzers, true);
        assert_eq!(
            analyzers,
            vec![
                "summary".to_string(),
                "dataset".to_string(),
                "concepts".to_string(),
                "mine".to_string(),
                "vocabulary".to_string(),
            ]
        );
        assert!(validate_analyzer_order(&analyzers, true).is_ok());
    }

    #[test]
    fn analyzer_order_violation_is_config_error() {
        let analyzers = vec![
            "dataset".to_string(),
            "mine".to_string(),
            "concepts".to_string(),
            "vocabulary".to_string(),
        ];
        let err = validate_analyzer_order(&analyzers, true).expect_err("out-of-order analyzers must error");
        assert!(err.to_string().contains("required order"));
    }
}
