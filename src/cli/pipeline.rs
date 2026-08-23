//! `pipeline` subcommand: run a configured experiment's `generate -> annotate -> analyze ->
//! report` stages, in that canonical order, with file handoffs between them.
//!
//! `--stages` selects a subset of `generate,annotate,analyze,report` (default: all four); the
//! stages always run in canonical order regardless of the order listed. Each stage's stdout line
//! is identical, character for character, to the line the matching standalone command prints
//! (`generate`, `annotate`, and `analyze` via `crate::cli::analyze::run_analyze`); `report`
//! writes the rendered Markdown to the experiment's `[report] out` path (via
//! `crate::cli::report::render_run_dir`) instead of echoing it. Before a stage runs, its
//! required input files are checked and a missing one is reported as `CorpusError::Precondition`
//! naming both the missing file and the stage that produces it. `annotate` is skipped entirely
//! (and left out of the final `stages=` line) when the experiment disables annotation. When
//! `[annotate] mode = "exhaustive"`, the annotate stage annotates every reachable position of
//! the game instead of the generated corpus, and has no `generate`-produced preconditions. The
//! stage-running body lives in `run_stages`, shared with the `discover` subcommand, which
//! runs the same four stages before continuing with evaluation.

use std::path::{Path, PathBuf};

use crate::cli::games::{dispatch_game, game_of_toml_file};
use crate::discovery::{
    AnalyzerRegistry, AnnotateMode, CorpusError, GameBundle, ResolvedExperiment, annotate_corpus, annotate_exhaustive,
    builtin_registry, generate, load_experiment, mode_name, resolve_experiment,
};
use crate::io::schema::ANALYZE_FILE;
use crate::io::{ANNOTATIONS_FILE, CorpusGame, IoError, POSITIONS_FILE, RUN_FILE};
use crate::strategy::engine::EngineGame;

/// Canonical stage order; `--stages` selects a subset of this but never reorders it.
const STAGE_ORDER: [&str; 4] = ["generate", "annotate", "analyze", "report"];

/// Flags for `pipeline`.
#[derive(clap::Args, Debug)]
pub struct PipelineArgs {
    /// Experiment TOML file to run.
    #[arg(long)]
    pub experiment: PathBuf,
    /// Run directory; overrides the experiment's own `out`.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Comma-separated subset of `generate,annotate,analyze,report`; stages always run in
    /// that canonical order regardless of the order listed.
    #[arg(long)]
    pub stages: Option<String>,
}

/// Runs `pipeline`.
pub(super) fn run(args: PipelineArgs) -> anyhow::Result<()> {
    let stages = selected_stages(args.stages.as_deref())?;
    let game = game_of_toml_file(&args.experiment)?;
    dispatch_game!(game.as_str(), |bundle| run_for(bundle, &args, &stages))
}

/// Parses `--stages` into the canonically ordered subset of [`STAGE_ORDER`] to run; `None`
/// (the flag omitted) selects all four. An empty token, an unknown stage name, or a repeated
/// name is `CorpusError::Config`, naming the offending token.
fn selected_stages(stages: Option<&str>) -> Result<Vec<&'static str>, CorpusError> {
    let Some(value) = stages else {
        return Ok(STAGE_ORDER.to_vec());
    };
    let requested: Vec<&str> = value.split(',').map(str::trim).collect();
    let mut seen: Vec<&str> = Vec::with_capacity(requested.len());
    for token in &requested {
        if !STAGE_ORDER.contains(token) || seen.contains(token) {
            return Err(CorpusError::Config(format!(
                "--stages `{token}` is not valid; known stages: generate, annotate, analyze, report"
            )));
        }
        seen.push(token);
    }
    Ok(STAGE_ORDER
        .iter()
        .copied()
        .filter(|stage| requested.contains(stage))
        .collect())
}

/// Runs the selected `stages` of the experiment at `args.experiment`, over one concrete game.
fn run_for<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, args: &PipelineArgs, stages: &[&str]) -> anyhow::Result<()> {
    let config = load_experiment::<G::Action>(&args.experiment)?;
    let base_dir = args.experiment.parent().unwrap_or_else(|| Path::new("."));
    let registry = builtin_registry::<G>();
    let resolved = resolve_experiment(config, base_dir, bundle, &registry, args.out.as_deref())?;

    let ran = run_stages(bundle, &resolved, &registry, stages)?;

    println!(
        "pipeline={} out={} stages={}",
        resolved.name,
        resolved.out.display(),
        ran.join(",")
    );
    Ok(())
}

/// Runs the selected `stages` of an already-resolved experiment, over one concrete game, without
/// printing the final `pipeline=` trailer line. Shared by the `pipeline` and `discover`
/// subcommands so both run the four stages with byte-identical stdout.
pub(crate) fn run_stages<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    resolved: &ResolvedExperiment<G::Action>,
    registry: &AnalyzerRegistry<G>,
    stages: &[&str],
) -> anyhow::Result<Vec<&'static str>> {
    let mut ran: Vec<&str> = Vec::with_capacity(stages.len());

    if stages.contains(&"generate") {
        let metadata = generate(bundle, resolved.sweep.clone(), &resolved.generate, &resolved.out)?;
        println!(
            "run_id={} cells={} games={} positions={} out={}",
            metadata.run_id,
            metadata.cells.len(),
            metadata.games,
            metadata.positions,
            resolved.out.display()
        );
        ran.push("generate");
    }

    if stages.contains(&"annotate")
        && let Some(options) = &resolved.annotate
    {
        let metadata = match resolved.annotate_mode {
            AnnotateMode::Corpus => {
                require_file(&resolved.out.join(RUN_FILE), "generate")?;
                require_file(&resolved.out.join(POSITIONS_FILE), "generate")?;
                annotate_corpus(bundle, &resolved.out, options)?
            }
            AnnotateMode::Exhaustive => annotate_exhaustive(bundle, &resolved.out, options)?,
        };
        println!(
            "mode={} annotated={} terminal={} disagreements={}",
            mode_name(metadata.mode),
            metadata.annotated,
            metadata.terminal,
            metadata.disagreements
        );
        ran.push("annotate");
    }

    if stages.contains(&"analyze") {
        require_file(&resolved.out.join(RUN_FILE), "generate")?;
        let needs_annotations = resolved
            .analyze
            .analyzers
            .iter()
            .filter_map(|name| registry.get(name))
            .any(|analyzer| analyzer.requires_annotations());
        if needs_annotations {
            require_file(&resolved.out.join(ANNOTATIONS_FILE), "annotate")?;
        }
        crate::cli::analyze::run_analyze(bundle, &resolved.out, &resolved.analyze)?;
        ran.push("analyze");
    }

    if stages.contains(&"report") {
        require_file(&resolved.out.join(ANALYZE_FILE), "analyze")?;
        let markdown = crate::cli::report::render_run_dir(bundle, &resolved.out, registry)?;
        if let Some(parent) = resolved.report_out.parent() {
            std::fs::create_dir_all(parent).map_err(|source| IoError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&resolved.report_out, markdown.as_bytes()).map_err(|source| IoError::Io {
            path: resolved.report_out.clone(),
            source,
        })?;
        println!("report={} bytes={}", resolved.report_out.display(), markdown.len());
        ran.push("report");
    }

    Ok(ran)
}

/// Fails with `CorpusError::Precondition` naming `path` and the stage (`produced_by`) that
/// produces it, unless `path` is an existing file.
fn require_file(path: &Path, produced_by: &str) -> Result<(), CorpusError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(CorpusError::Precondition(format!(
            "{} not found; run the `{produced_by}` stage first",
            path.display()
        )))
    }
}
