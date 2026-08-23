//! Analyzer registry: pluggable corpus analysis stages run by the `analyze` pipeline command.
//!
//! An [`Analyzer`] reads an already-generated corpus through an [`AnalyzeContext`] (which
//! exposes its games, positions and, lazily, its `annotations.jsonl`) and produces one output
//! document plus a pass/fail verdict. [`AnalyzerRegistry`] is the game-agnostic set of analyzers
//! available for a game; [`builtin_registry`] wires up the analyzers this crate ships:
//! [`crate::discovery::agreement::AgreementAnalyzer`] (`"agreement"`),
//! [`crate::discovery::dataset::DatasetAnalyzer`] (`"dataset"`),
//! [`crate::discovery::mine::MineAnalyzer`] (`"mine"`), and
//! [`crate::discovery::summary::SummaryAnalyzer`] (`"summary"`).
//! [`analyze_outputs`] (and its thin wrapper [`analyze`]) validate the requested analyzer names,
//! run each requested analyzer in the given order, write its output file into the corpus
//! directory, and write `analyze.json` (an [`AnalyzeMetadata`] manifest) alongside them.
//!
//! Byte-identity (ADR 0009): repeated runs over the same corpus and options write byte-identical
//! output files. `serde_json` in this crate is built WITHOUT `preserve_order`, so a
//! `serde_json::Value` object always serializes with its keys sorted — round-tripping a typed
//! document through `Value` therefore reorders its fields and changes its bytes. For this reason
//! an [`Analyzer::run`] hands back the exact bytes it wants written ([`AnalyzerOutput::json`],
//! built by [`AnalyzerOutput::new`] directly from the typed document, exactly as
//! [`crate::io::write_json_pretty`] would write it), never a re-serialized `Value`; `Value` is
//! used only by [`Analyzer::render`], which reads a written output file back for rendering.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::core::traits::GameDomain;
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::discovery::summary::DiversityThresholds;
use crate::io::schema::ANALYZE_FILE;
use crate::io::{
    ANNOTATIONS_FILE, AnnotationRecord, CorpusGame, GAMES_FILE, GameRecord, IoError, MineParams, POSITIONS_FILE, PositionRecord,
    RUN_FILE, SCHEMA_VERSION, check_schema_version, read_json, read_jsonl, write_json_pretty,
};
use crate::strategy::engine::EngineGame;

/// A [`GameRecord`] specialized to `G`'s persisted types.
pub type GameRec<G> =
    GameRecord<<G as GameDomain>::State, <G as GameDomain>::Action, <G as GameDomain>::Player, <G as GameDomain>::Outcome>;
/// A [`PositionRecord`] specialized to `G`'s persisted types.
pub type PosRec<G> =
    PositionRecord<<G as GameDomain>::State, <G as GameDomain>::Action, <G as GameDomain>::Player, <G as GameDomain>::Outcome>;
/// An [`AnnotationRecord`] specialized to `G`'s persisted types.
pub type AnnRec<G> = AnnotationRecord<<G as GameDomain>::State, <G as GameDomain>::Action, <G as GameDomain>::Player>;

/// Knobs for an `analyze` pass: which analyzers to run, the diversity thresholds handed to
/// analyzers that check diversity, and whether a failed check should be treated as fatal by the
/// caller (the CLI maps this to an exit status; nothing in this module exits the process).
#[derive(Debug, Clone, PartialEq)]
pub struct AnalyzeOptions {
    /// Names of analyzers to run, in the order they run and are listed in `analyze.json`.
    pub analyzers: Vec<String>,
    /// Diversity thresholds handed to analyzers that check diversity (currently `summary`).
    pub thresholds: DiversityThresholds,
    /// Whether a failed analyzer check should be treated as fatal by the caller.
    pub strict: bool,
    /// Miner parameters consumed by the `dataset`/`mine` analyzers.
    pub mine: MineParams,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        AnalyzeOptions {
            analyzers: vec!["summary".to_string()],
            thresholds: DiversityThresholds::default(),
            strict: false,
            mine: MineParams::default(),
        }
    }
}

/// The fields of `run.json` this stage needs; other fields are ignored.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RunHeader {
    /// Schema version `run.json` was written under.
    pub schema_version: u32,
    /// Game name the run was generated for.
    pub game: String,
    /// Identifier of the run.
    pub run_id: String,
}

/// A corpus run's header plus its games and positions, loaded and schema-checked.
pub struct LoadedCorpus<G: EngineGame + CorpusGame> {
    /// Parsed `run.json` header.
    pub header: RunHeader,
    /// Every game in the corpus, in file order.
    pub games: Vec<GameRec<G>>,
    /// Every position in the corpus, in file order.
    pub positions: Vec<PosRec<G>>,
}

/// Reads and schema-checks `run.json`, `games.jsonl` and `positions.jsonl` from `corpus_dir`,
/// rejecting a header whose `game` does not match `bundle.name`.
pub fn load_corpus<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, corpus_dir: &Path) -> Result<LoadedCorpus<G>, CorpusError> {
    let run_path = corpus_dir.join(RUN_FILE);
    if !run_path.is_file() {
        return Err(CorpusError::Io(IoError::Missing { path: run_path }));
    }
    let header: RunHeader = read_json(&run_path)?;
    check_schema_version(&run_path, header.schema_version)?;
    if header.game != bundle.name {
        return Err(CorpusError::Config(format!(
            "corpus game `{}` does not match bundle `{}`",
            header.game, bundle.name
        )));
    }

    let games_path = corpus_dir.join(GAMES_FILE);
    if !games_path.is_file() {
        return Err(CorpusError::Io(IoError::Missing { path: games_path }));
    }
    let games: Vec<GameRec<G>> = read_jsonl(&games_path)?;
    for game in &games {
        check_schema_version(&games_path, game.schema_version)?;
    }

    let positions_path = corpus_dir.join(POSITIONS_FILE);
    if !positions_path.is_file() {
        return Err(CorpusError::Io(IoError::Missing { path: positions_path }));
    }
    let positions: Vec<PosRec<G>> = read_jsonl(&positions_path)?;
    for position in &positions {
        check_schema_version(&positions_path, position.schema_version)?;
    }

    Ok(LoadedCorpus {
        header,
        games,
        positions,
    })
}

/// Everything an [`Analyzer`] needs to run: the loaded corpus (games, positions, and — lazily —
/// annotations) plus the game bundle and corpus directory it came from.
pub struct AnalyzeContext<'a, G: EngineGame + CorpusGame> {
    /// Game bundle the corpus was generated for.
    pub bundle: &'a GameBundle<G>,
    /// Corpus run directory being analyzed.
    pub corpus_dir: &'a Path,
    /// `run_id` of the corpus, from its `run.json`.
    pub run_id: String,
    /// Every game in the corpus.
    pub games: &'a [GameRec<G>],
    /// Every position in the corpus.
    pub positions: &'a [PosRec<G>],
    /// `annotations.jsonl` of the corpus, read and schema-checked on first use by
    /// [`Self::annotations`].
    annotations: std::cell::OnceCell<Vec<AnnRec<G>>>,
}

impl<'a, G: EngineGame + CorpusGame> AnalyzeContext<'a, G> {
    /// Wraps an already-loaded corpus for analyzer use.
    pub fn new(
        bundle: &'a GameBundle<G>,
        corpus_dir: &'a Path,
        run_id: String,
        games: &'a [GameRec<G>],
        positions: &'a [PosRec<G>],
    ) -> Self {
        AnalyzeContext {
            bundle,
            corpus_dir,
            run_id,
            games,
            positions,
            annotations: std::cell::OnceCell::new(),
        }
    }

    /// `annotations.jsonl` of the corpus, read and schema-checked on first use.
    pub fn annotations(&self) -> Result<&[AnnRec<G>], CorpusError> {
        if let Some(records) = self.annotations.get() {
            return Ok(records);
        }
        let path = self.corpus_dir.join(ANNOTATIONS_FILE);
        if !path.is_file() {
            return Err(CorpusError::Precondition(format!(
                "{} not found; run `annotate --corpus {}` first",
                path.display(),
                self.corpus_dir.display()
            )));
        }
        let records: Vec<AnnRec<G>> = read_jsonl(&path)?;
        for record in &records {
            check_schema_version(&path, record.schema_version)?;
        }
        let _ = self.annotations.set(records);
        Ok(self.annotations.get().expect("set above"))
    }
}

/// One analyzer's result: the file it wants written, the exact bytes to write, and whether its
/// own check passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerOutput {
    /// Output file name, relative to the corpus directory (e.g. `"summary.json"`).
    pub file: String,
    /// The exact bytes to write to `file`: pretty JSON plus a trailing LF, identical to what
    /// [`crate::io::write_json_pretty`] would write for the same document.
    pub json: String,
    /// This analyzer's own pass/fail verdict; `true` for analyzers without a check.
    pub checks_pass: bool,
}

impl AnalyzerOutput {
    /// Serializes `value` exactly as `write_json_pretty` writes it: pretty JSON plus a trailing LF.
    pub fn new<T: Serialize>(file: &str, value: &T, checks_pass: bool) -> Result<Self, CorpusError> {
        let mut json = serde_json::to_string_pretty(value).map_err(|source| IoError::Json {
            path: PathBuf::from(file),
            line: 0,
            source,
        })?;
        json.push('\n');
        Ok(AnalyzerOutput {
            file: file.to_string(),
            json,
            checks_pass,
        })
    }
    /// Parses [`Self::json`] into the `serde_json::Value` form that [`Analyzer::render`] takes.
    pub fn value(&self) -> Result<serde_json::Value, CorpusError> {
        serde_json::from_str(&self.json).map_err(|source| {
            CorpusError::Io(IoError::Json {
                path: PathBuf::from(&self.file),
                line: 0,
                source,
            })
        })
    }
}

/// One pluggable analysis stage: reads an [`AnalyzeContext`] and produces one output document.
pub trait Analyzer<G: EngineGame + CorpusGame>: Send + Sync {
    /// Short, stable name; keys this analyzer in [`AnalyzerRegistry`] and its `analyze.json`
    /// entry, and selects it via `--analyzers`.
    fn name(&self) -> &str;
    /// One-line human-readable description of what this analyzer computes.
    fn description(&self) -> &str;
    /// Whether this analyzer needs [`AnalyzeContext::annotations`].
    fn requires_annotations(&self) -> bool;
    /// Runs the analyzer, producing its output document and verdict.
    fn run(&self, ctx: &AnalyzeContext<'_, G>, options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError>;
    /// Renders a previously written output file (read back as a `Value`) into a deterministic
    /// Markdown section starting with `## {Title}` (`Title` = [`Analyzer::name`] with its first
    /// letter upper-cased).
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError>;
}

/// Analyzers available for a game, keyed by name.
pub struct AnalyzerRegistry<G: EngineGame + CorpusGame> {
    analyzers: BTreeMap<String, Arc<dyn Analyzer<G>>>,
}

impl<G: EngineGame + CorpusGame> AnalyzerRegistry<G> {
    /// An empty registry.
    pub fn new() -> Self {
        AnalyzerRegistry {
            analyzers: BTreeMap::new(),
        }
    }

    /// Registers `analyzer`, keyed by its own [`Analyzer::name`]; replaces any analyzer already
    /// registered under that name.
    pub fn register(&mut self, analyzer: Arc<dyn Analyzer<G>>) {
        self.analyzers.insert(analyzer.name().to_string(), analyzer);
    }

    /// Looks up an analyzer by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Analyzer<G>>> {
        self.analyzers.get(name)
    }

    /// Registered analyzer names, sorted.
    pub fn names(&self) -> Vec<&str> {
        self.analyzers.keys().map(String::as_str).collect()
    }

    /// Iterates over `(name, analyzer)` pairs in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Arc<dyn Analyzer<G>>)> {
        self.analyzers.iter().map(|(name, analyzer)| (name.as_str(), analyzer))
    }

    /// Number of registered analyzers.
    pub fn len(&self) -> usize {
        self.analyzers.len()
    }

    /// Whether no analyzers are registered.
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty()
    }
}

impl<G: EngineGame + CorpusGame> Default for AnalyzerRegistry<G> {
    fn default() -> Self {
        Self::new()
    }
}

/// The analyzers this crate ships, registered under their default names: `agreement`
/// (`crate::discovery::agreement`), `dataset` (`crate::discovery::dataset`), `mine`
/// (`crate::discovery::mine`), and
/// [`SummaryAnalyzer`](crate::discovery::summary::SummaryAnalyzer) as `"summary"`.
pub fn builtin_registry<G: EngineGame + CorpusGame>() -> AnalyzerRegistry<G> {
    let mut registry = AnalyzerRegistry::new();
    registry.register(Arc::new(crate::discovery::agreement::AgreementAnalyzer));
    registry.register(Arc::new(crate::discovery::dataset::DatasetAnalyzer));
    registry.register(Arc::new(crate::discovery::mine::MineAnalyzer));
    registry.register(Arc::new(crate::discovery::summary::SummaryAnalyzer));
    registry
}

/// One analyzer's entry in [`AnalyzeMetadata::analyzers`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnalyzerEntry {
    /// Analyzer name.
    pub name: String,
    /// Output file it wrote, relative to the corpus directory.
    pub file: String,
}

/// Manifest of one `analyze` pass, written to `analyze.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnalyzeMetadata {
    /// Schema version this manifest was written under.
    pub schema_version: u32,
    /// Game name this pass analyzed.
    pub game: String,
    /// `run_id` of the corpus this pass read.
    pub run_id: String,
    /// Analyzers run, in the order they ran.
    pub analyzers: Vec<AnalyzerEntry>,
    /// Diversity thresholds this pass was run with.
    pub thresholds: DiversityThresholds,
    /// Whether a failed analyzer check should be treated as fatal by the caller.
    pub strict: bool,
    /// Logical AND of every analyzer's [`AnalyzerOutput::checks_pass`].
    pub checks_pass: bool,
}

/// Checks `analyzers` against `registry` before any corpus is loaded or file written: rejects an
/// empty list, a name listed more than once, and a name not in `registry`.
fn validate_analyzers<G: EngineGame + CorpusGame>(registry: &AnalyzerRegistry<G>, analyzers: &[String]) -> Result<(), CorpusError> {
    let names = registry.names().join(", ");
    if analyzers.is_empty() {
        return Err(CorpusError::Config(format!(
            "no analyzers requested; known analyzers: {names}"
        )));
    }
    let mut seen: Vec<&str> = Vec::with_capacity(analyzers.len());
    for name in analyzers {
        if seen.contains(&name.as_str()) {
            return Err(CorpusError::Config(format!(
                "analyzer `{name}` is listed more than once; list each analyzer once"
            )));
        }
        seen.push(name.as_str());
        if registry.get(name).is_none() {
            return Err(CorpusError::Config(format!(
                "unknown analyzer `{name}`; known analyzers: {names}"
            )));
        }
    }
    Ok(())
}

/// Validates `options.analyzers` against `registry`, loads the corpus, runs each requested
/// analyzer in order, writes each analyzer's output file into `corpus_dir`, and writes
/// `analyze.json`. Returns the manifest plus every analyzer's raw output (in run order).
pub fn analyze_outputs<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    corpus_dir: &Path,
    registry: &AnalyzerRegistry<G>,
    options: &AnalyzeOptions,
) -> Result<(AnalyzeMetadata, Vec<AnalyzerOutput>), CorpusError> {
    validate_analyzers(registry, &options.analyzers)?;

    let loaded = load_corpus(bundle, corpus_dir)?;
    let ctx = AnalyzeContext::new(
        bundle,
        corpus_dir,
        loaded.header.run_id.clone(),
        &loaded.games,
        &loaded.positions,
    );

    let mut analyzers = Vec::with_capacity(options.analyzers.len());
    let mut outputs = Vec::with_capacity(options.analyzers.len());
    let mut checks_pass = true;
    for name in &options.analyzers {
        let analyzer = registry.get(name).expect("validated above");
        let started = std::time::Instant::now();
        let output = analyzer.run(&ctx, options)?;

        let path = corpus_dir.join(&output.file);
        std::fs::write(&path, output.json.as_bytes()).map_err(|source| IoError::Io {
            path: path.clone(),
            source,
        })?;
        tracing::info!(
            analyzer = name.as_str(),
            file = %output.file,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "analyzer finished"
        );

        analyzers.push(AnalyzerEntry {
            name: name.clone(),
            file: output.file.clone(),
        });
        checks_pass &= output.checks_pass;
        outputs.push(output);
    }

    let metadata = AnalyzeMetadata {
        schema_version: SCHEMA_VERSION,
        game: bundle.name.clone(),
        run_id: loaded.header.run_id.clone(),
        analyzers,
        thresholds: options.thresholds.clone(),
        strict: options.strict,
        checks_pass,
    };
    write_json_pretty(&corpus_dir.join(ANALYZE_FILE), &metadata)?;

    Ok((metadata, outputs))
}

/// [`analyze_outputs`], discarding the individual analyzer outputs.
pub fn analyze<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    corpus_dir: &Path,
    registry: &AnalyzerRegistry<G>,
    options: &AnalyzeOptions,
) -> Result<AnalyzeMetadata, CorpusError> {
    analyze_outputs(bundle, corpus_dir, registry, options).map(|(metadata, _)| metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::config::GenerateConfig;
    use crate::discovery::corpus::{GenerateOptions, generate};
    use crate::games::tictactoe::{Move, TicTacToe, game_bundle};
    use crate::io::SUMMARY_FILE;

    const CONFIG: &str = r#"
schema_version = 1
game = "tictactoe"
seed = 7
games_per_cell = 2
pairings = [["random", "random"], ["random", "depth-1"], ["depth-1", "random"]]

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"

[[strategies]]
name = "depth-1"
[strategies.spec]
kind = "minimax"
depth = 1
"#;

    /// Generates the tiny [`CONFIG`] corpus into a fresh temp directory named after `name`.
    fn small_corpus(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sd-analyze-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bundle = game_bundle();
        let config = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();
        generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
        dir
    }

    /// A minimal analyzer for registry/validation tests: writes `{"ok": true}` to
    /// `"{name}.json"` and renders a bare `## {name}` heading.
    struct Stub {
        name: &'static str,
        description: &'static str,
    }

    impl Analyzer<TicTacToe> for Stub {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            self.description
        }
        fn requires_annotations(&self) -> bool {
            false
        }
        fn run(&self, _ctx: &AnalyzeContext<'_, TicTacToe>, _options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
            AnalyzerOutput::new(&format!("{}.json", self.name), &serde_json::json!({"ok": true}), true)
        }
        fn render(&self, _output: &serde_json::Value) -> Result<String, CorpusError> {
            Ok(format!("## {}\n\n", self.name))
        }
    }

    #[test]
    fn registry_register_replace_and_names() {
        let mut registry: AnalyzerRegistry<TicTacToe> = AnalyzerRegistry::new();
        registry.register(Arc::new(Stub {
            name: "b",
            description: "second",
        }));
        registry.register(Arc::new(Stub {
            name: "a",
            description: "first",
        }));
        assert_eq!(registry.names(), vec!["a", "b"]);

        registry.register(Arc::new(Stub {
            name: "a",
            description: "replaced",
        }));
        assert_eq!(registry.len(), 2);
        assert_eq!(registry.get("a").unwrap().description(), "replaced");

        assert!(registry.get("zzz").is_none());
    }

    #[test]
    fn builtin_registry_names() {
        assert_eq!(
            builtin_registry::<TicTacToe>().names(),
            vec!["agreement", "dataset", "mine", "summary"]
        );
    }

    #[test]
    fn analyze_options_default_includes_mine() {
        assert_eq!(AnalyzeOptions::default().mine, MineParams::default());
        assert_eq!(AnalyzeOptions::default().analyzers, vec!["summary".to_string()]);
    }

    #[test]
    fn analyze_rejects_empty_unknown_and_duplicate_analyzers() {
        let dir = small_corpus("rejects");
        let bundle = game_bundle();
        let registry = builtin_registry::<TicTacToe>();

        let empty = AnalyzeOptions {
            analyzers: vec![],
            ..AnalyzeOptions::default()
        };
        match analyze(&bundle, &dir, &registry, &empty) {
            Err(CorpusError::Config(msg)) => assert!(msg.contains("no analyzers")),
            other => panic!("expected Err(Config(_)), got {other:?}"),
        }

        let unknown = AnalyzeOptions {
            analyzers: vec!["nope".to_string()],
            ..AnalyzeOptions::default()
        };
        match analyze(&bundle, &dir, &registry, &unknown) {
            Err(CorpusError::Config(msg)) => {
                assert!(msg.contains("unknown analyzer `nope`"));
                assert!(msg.contains("summary"));
            }
            other => panic!("expected Err(Config(_)), got {other:?}"),
        }

        let duplicate = AnalyzeOptions {
            analyzers: vec!["summary".to_string(), "summary".to_string()],
            ..AnalyzeOptions::default()
        };
        match analyze(&bundle, &dir, &registry, &duplicate) {
            Err(CorpusError::Config(msg)) => assert!(msg.contains("more than once")),
            other => panic!("expected Err(Config(_)), got {other:?}"),
        }

        assert!(!dir.join(ANALYZE_FILE).is_file());
    }

    #[test]
    fn analyze_writes_summary_and_manifest() {
        let dir = small_corpus("writes-summary-and-manifest");
        let bundle = game_bundle();
        let registry = builtin_registry::<TicTacToe>();

        let (metadata, outputs) = analyze_outputs(&bundle, &dir, &registry, &AnalyzeOptions::default()).unwrap();

        assert_eq!(
            metadata.analyzers,
            vec![AnalyzerEntry {
                name: "summary".to_string(),
                file: "summary.json".to_string(),
            }]
        );
        assert!(dir.join(SUMMARY_FILE).is_file());
        assert!(dir.join(ANALYZE_FILE).is_file());

        let read_back: AnalyzeMetadata = read_json(&dir.join(ANALYZE_FILE)).unwrap();
        assert_eq!(read_back, metadata);

        assert_eq!(std::fs::read_to_string(dir.join(SUMMARY_FILE)).unwrap(), outputs[0].json);
    }

    #[test]
    fn annotations_missing_is_a_precondition_error() {
        let dir = small_corpus("annotations-missing");
        let bundle = game_bundle();
        let loaded = load_corpus::<TicTacToe>(&bundle, &dir).unwrap();
        let ctx = AnalyzeContext::new(&bundle, &dir, loaded.header.run_id.clone(), &loaded.games, &loaded.positions);

        match ctx.annotations() {
            Err(CorpusError::Precondition(msg)) => {
                assert!(msg.contains("annotations.jsonl"));
                assert!(msg.contains("annotate --corpus"));
            }
            other => panic!("expected Err(Precondition(_)), got {other:?}"),
        }
    }

    #[test]
    fn analyzer_output_json_matches_write_json_pretty() {
        let path = std::env::temp_dir().join(format!("sd-analyze-output-json-{}.json", std::process::id()));
        write_json_pretty(&path, &DiversityThresholds::default()).unwrap();
        let expected = std::fs::read_to_string(&path).unwrap();

        let output = AnalyzerOutput::new("t.json", &DiversityThresholds::default(), true).unwrap();
        assert_eq!(output.json, expected);
    }
}
