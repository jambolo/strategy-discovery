//! `report` subcommand: renders a Markdown report from a run directory or a single analyzer
//! output file.
//!
//! Whole-run mode (`--input` a directory) assembles a `# Run` header and overview table, an
//! `## Annotation` block when `annotate.json` is present, then one block per analyzer listed in
//! `analyze.json`'s manifest, in that order (`render_run_dir`, reused by the `pipeline`
//! command). Single-analyzer mode (`--input` a file, e.g. `summary.json`) renders just that
//! file's analyzer block, with the game resolved from the `run.json` next to it.
//!
//! Byte-identity (ADR 0009): every analyzer this crate ships already renders deterministic
//! Markdown ending in exactly one `\n`; this command only concatenates those blocks and prints
//! or writes the result verbatim, so two `report` runs over the same inputs, and `--out` versus
//! stdout, produce identical bytes.

use std::path::{Path, PathBuf};

use crate::cli::games::{dispatch_game, game_of_run_dir};
use crate::discovery::{
    AnalyzeMetadata, AnalyzerRegistry, AnnotateMetadata, CorpusError, GameBundle, RunMetadata, builtin_registry,
};
use crate::io::schema::ANALYZE_FILE;
use crate::io::{ANNOTATE_FILE, CorpusGame, IoError, RUN_FILE, read_json};
use crate::strategy::engine::EngineGame;

/// Flags for `report`.
#[derive(clap::Args, Debug)]
pub struct ReportArgs {
    /// Corpus run directory, or a single analyzer output file (e.g. `summary.json`).
    #[arg(long)]
    pub input: PathBuf,
    /// Also write the rendered Markdown to this file, byte-identical to stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// Runs `report`.
pub(super) fn run(args: ReportArgs) -> anyhow::Result<()> {
    let markdown = render(&args.input)?;
    print!("{markdown}");
    if let Some(path) = &args.out {
        std::fs::write(path, markdown.as_bytes()).map_err(|source| IoError::Io {
            path: path.clone(),
            source,
        })?;
    }
    Ok(())
}

/// Renders `input` to Markdown: a run directory in whole-run mode, a single analyzer output
/// file in single-analyzer mode, or [`IoError::Missing`] when `input` is neither.
fn render(input: &Path) -> Result<String, CorpusError> {
    if input.is_dir() {
        let game = game_of_run_dir(input)?;
        dispatch_game!(game.as_str(), |bundle| render_run_dir(bundle, input, &builtin_registry()))
    } else if input.is_file() {
        let dir = input.parent().unwrap_or_else(|| Path::new("."));
        let game = game_of_run_dir(dir)?;
        dispatch_game!(game.as_str(), |bundle| render_single_analyzer(bundle, input))
    } else {
        Err(CorpusError::Io(IoError::Missing {
            path: input.to_path_buf(),
        }))
    }
}

/// Assembles the whole-run Markdown report for the corpus run directory `dir`: a `# Run` header
/// and overview table, an `## Annotation` block when `annotate.json` is present, then one block
/// per analyzer listed in `dir`'s `analyze.json` manifest, in manifest order. `pub(crate)` so the
/// `pipeline` command can reuse it.
pub(crate) fn render_run_dir<G: EngineGame + CorpusGame>(
    _bundle: &GameBundle<G>,
    dir: &Path,
    registry: &AnalyzerRegistry<G>,
) -> Result<String, CorpusError> {
    let mut blocks: Vec<String> = Vec::new();

    let run_path = dir.join(RUN_FILE);
    if !run_path.is_file() {
        return Err(CorpusError::Io(IoError::Missing { path: run_path }));
    }
    let run: RunMetadata<G::Action, G::Player> = read_json(&run_path)?;
    blocks.push(format!("# Run {}", run.run_id));
    blocks.push(run_overview_table(&run));

    let annotate_path = dir.join(ANNOTATE_FILE);
    if annotate_path.is_file() {
        let annotate: AnnotateMetadata = read_json(&annotate_path)?;
        blocks.push("## Annotation".to_string());
        blocks.push(annotation_table(&annotate));
    }

    let analyze_path = dir.join(ANALYZE_FILE);
    if !analyze_path.is_file() {
        return Err(CorpusError::Precondition(format!(
            "{} not found; run `analyze --corpus {}` first",
            analyze_path.display(),
            dir.display()
        )));
    }
    let manifest: AnalyzeMetadata = read_json(&analyze_path)?;
    for entry in &manifest.analyzers {
        let analyzer = registry.get(&entry.name).ok_or_else(|| {
            CorpusError::Config(format!(
                "unknown analyzer `{}` in {}; known analyzers: {}",
                entry.name,
                ANALYZE_FILE,
                registry.names().join(", ")
            ))
        })?;
        let output_path = dir.join(&entry.file);
        if !output_path.is_file() {
            return Err(CorpusError::Io(IoError::Missing { path: output_path }));
        }
        let value: serde_json::Value = read_json(&output_path)?;
        blocks.push(analyzer.render(&value)?);
    }

    Ok(blocks
        .iter()
        .map(|b| b.trim_end_matches('\n'))
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n")
}

/// The `# Run` header's overview table: `| field | value |` rows for the run's identity, config
/// knobs and totals, in the fixed order [`render_run_dir`] documents.
fn run_overview_table<A, P>(run: &RunMetadata<A, P>) -> String {
    let strategies: Vec<&str> = run.config.strategies.iter().map(|s| s.name.as_str()).collect();
    let openings: Vec<&str> = run.config.openings.iter().map(|o| o.name.as_str()).collect();
    let random_opening_plies: Vec<String> = run.config.random_opening_plies.iter().map(|p| p.to_string()).collect();
    [
        "| field | value |".to_string(),
        "| --- | --- |".to_string(),
        format!("| game | {} |", run.game),
        format!("| crate_name | {} |", run.crate_name),
        format!("| crate_version | {} |", run.crate_version),
        format!("| seed | {} |", run.config.seed),
        format!("| games_per_cell | {} |", run.config.games_per_cell),
        format!("| cells | {} |", run.cells.len()),
        format!("| games | {} |", run.games),
        format!("| positions | {} |", run.positions),
        format!("| strategies | {} |", strategies.join(", ")),
        format!("| pairings | {} |", run.config.pairings.len()),
        format!("| evaluators | {} |", run.config.evaluators.join(", ")),
        format!("| openings | {} |", openings.join(", ")),
        format!("| random_opening_plies | {} |", random_opening_plies.join(", ")),
    ]
    .join("\n")
}

/// The `## Annotation` block's `| field | value |` table.
fn annotation_table(annotate: &AnnotateMetadata) -> String {
    [
        "| field | value |".to_string(),
        "| --- | --- |".to_string(),
        format!("| mode | {:?} |", annotate.mode),
        format!("| engine_depth | {} |", annotate.engine_depth),
        format!("| evaluator | {} |", annotate.evaluator),
        format!("| annotated | {} |", annotate.annotated),
        format!("| terminal | {} |", annotate.terminal),
        format!("| disagreements | {} |", annotate.disagreements),
        format!("| solver_states | {} |", annotate.solver_states),
    ]
    .join("\n")
}

/// Renders a single analyzer output file (`input`, e.g. `summary.json`), chosen by its file
/// stem, into just that analyzer's Markdown block (no `# Run` header), normalized to end with
/// exactly one `\n`.
fn render_single_analyzer<G: EngineGame + CorpusGame>(_bundle: &GameBundle<G>, input: &Path) -> Result<String, CorpusError> {
    let registry: AnalyzerRegistry<G> = builtin_registry();
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let analyzer = registry.get(stem).ok_or_else(|| {
        let file_name = input.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        CorpusError::Config(format!(
            "`{file_name}` is not a registered analyzer's output file; known analyzers: {}",
            registry.names().join(", ")
        ))
    })?;
    let value: serde_json::Value = read_json(input)?;
    let rendered = analyzer.render(&value)?;
    Ok(format!("{}\n", rendered.trim_end_matches('\n')))
}
