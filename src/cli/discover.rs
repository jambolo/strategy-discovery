//! `discover` subcommand: runs a configured experiment end to end and mines it for heuristics.
//!
//! The chain is: the four experiment stages (`generate -> annotate -> analyze -> report`, run
//! via [`crate::cli::pipeline::run_stages`] and printed byte-identically to `pipeline`), then the
//! library orchestration [`crate::discovery::run_discover`], which evaluates the mined
//! candidates, archives them, and writes `evaluation.json` and `discover.json`. Before running,
//! the `dataset` and `mine` analyzers are appended to the experiment's `[analyze] analyzers` list
//! (in that order) if not already present, so mining always runs regardless of the experiment's
//! own analyzer selection.
//!
//! Output order after the four stage lines: the same `strategy=`/`evaluation=` result lines
//! `evaluate` prints (via [`crate::cli::evaluate::print_result`]), then one final `discover=`
//! summary line. The strict loss-rate check runs LAST, after `evaluation.json` and `discover.json`
//! are already on disk, so a strict failure never suppresses a write.
//!
//! Exit codes: 0 success; 1 runtime failure; 2 invalid config or a missing precondition file;
//! 3 the `--strict`-equivalent `[discover] strict = true` loss-rate check failed.

use std::path::PathBuf;

use crate::cli::error::CliError;
use crate::cli::games::{dispatch_game, game_of_toml_file};
use crate::discovery::{GameBundle, builtin_registry, load_experiment, resolve_experiment, run_discover, strict_failures};
use crate::io::CorpusGame;
use crate::io::config_hash;
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

    for name in ["dataset", "mine"] {
        if !config.analyze.analyzers.iter().any(|existing| existing == name) {
            config.analyze.analyzers.push(name.to_string());
        }
    }

    let base_dir = args.config.parent().unwrap_or_else(|| std::path::Path::new("."));
    let registry = builtin_registry::<G>();
    let resolved = resolve_experiment(config, base_dir, bundle, &registry, args.out.as_deref())?;

    crate::cli::pipeline::run_stages(bundle, &resolved, &registry, &["generate", "annotate", "analyze", "report"])?;

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
