//! Experiment configuration: the top-level TOML document that ties the `generate`, `annotate`,
//! `analyze` and `report` stages of the discovery pipeline into one named, reproducible run
//! description for a later `pipeline` CLI command to execute end to end.
//!
//! ```toml
//! schema_version = 1
//! name = "small"
//! game = "<game-name>"
//! out = "runs/small"
//!
//! [generate]
//! sweep = "generate-small.toml"   # path to a GenerateConfig TOML, OR an inline [generate.sweep] table with the full GenerateConfig fields
//! threads = 4                     # optional
//! serial = false                  # optional, default false
//!
//! [annotate]
//! enabled = true                  # default true
//! engine_depth = 9                # optional
//!
//! [analyze]
//! analyzers = ["summary", "agreement"]   # default ["summary"]
//! strict = false                         # default false
//! [analyze.thresholds]                   # optional as a whole; defaults 0.5 / 0.2 / 0.5
//! min_canonical_coverage = 0.25
//! min_decisive_fraction = 0.2
//! min_distinct_game_fraction = 0.5
//!
//! [report]
//! out = "report.md"               # relative to `out`; default "report.md"
//! ```
//!
//! `game` names any game the CLI can resolve; nothing in this module is specific to one game.
//! The repository's own checked-in experiment files under `configs/` and the fixture
//! `tests/fixtures/experiment-small.toml` are complete worked examples.
//!
//! Every relative path in this document — the sweep path, `out`, and `[report] out` — resolves
//! against the EXPERIMENT FILE'S OWN parent directory, never the process's current directory;
//! see [`resolve_experiment`]'s `base_dir` parameter. A CLI `--out` (this module's
//! `out_override`) overrides `out` from the file entirely rather than being joined with it.
//!
//! [`load_experiment`] parses a file with no validation; [`resolve_experiment`] validates it
//! against a [`GameBundle`] and [`AnalyzerRegistry`], loads its sweep, and reduces every section
//! to the plain options types the pipeline stages already take, producing a
//! [`ResolvedExperiment`]. [`StrategyFile`] is a standalone `[[strategies]]` roster file in the
//! same shape as a sweep's own strategy list, reused by a later phase.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::discovery::analyze::{AnalyzeOptions, AnalyzerRegistry};
use crate::discovery::annotate::{AnnotateOptions, DEFAULT_SOLVER_LIMIT};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::{CorpusError, GenerateConfig, resolve};
use crate::discovery::corpus::GenerateOptions;
use crate::discovery::summary::DiversityThresholds;
use crate::io::{CorpusGame, IoError, SCHEMA_VERSION, check_schema_version};
use crate::strategy::engine::EngineGame;
use crate::strategy::roster::RosterEntry;

/// Top-level experiment document; see the module docs for its TOML shape.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExperimentConfig<A> {
    /// Schema version this config was authored against; must equal [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Experiment name; must not be empty.
    pub name: String,
    /// Game name; must match both the target game bundle's name and the sweep's `game`.
    pub game: String,
    /// Output directory for this experiment's runs, relative to the experiment file unless
    /// overridden by a CLI `--out`.
    pub out: PathBuf,
    /// The `generate` stage's configuration: which sweep to run and how.
    pub generate: GenerateSection<A>,
    /// The `annotate` stage's configuration; defaults to enabled at the bundle's default depth.
    #[serde(default)]
    pub annotate: AnnotateSection,
    /// The `analyze` stage's configuration: which analyzers to run and their thresholds.
    #[serde(default)]
    pub analyze: AnalyzeSection,
    /// The `report` stage's configuration: where to write the rendered report.
    #[serde(default)]
    pub report: ReportSection,
}

/// Where a `[generate]` section's sweep configuration comes from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum SweepSource<A> {
    /// A path to a `GenerateConfig` TOML file, relative to the experiment file's directory.
    Path(PathBuf),
    /// A `GenerateConfig` written inline as a `[generate.sweep]` table.
    Inline(GenerateConfig<A>),
}

/// TOML `[generate]` section.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GenerateSection<A> {
    /// Sweep configuration: a path to a `GenerateConfig` TOML file, or an inline
    /// `[generate.sweep]` table with the full `GenerateConfig` fields.
    pub sweep: SweepSource<A>,
    /// Run each cell's games on a private pool of this many threads; `None` uses rayon's
    /// global pool. Must be `>= 1` when set.
    #[serde(default)]
    pub threads: Option<usize>,
    /// Run each cell's games serially; takes precedence over `threads`.
    #[serde(default)]
    pub serial: bool,
}

/// TOML `[annotate]` section.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnnotateSection {
    /// Whether the annotation stage runs at all.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Engine search depth for the cross-check; `None` means the bundle's default
    /// (`full_search_depth`).
    #[serde(default)]
    pub engine_depth: Option<u32>,
}

impl Default for AnnotateSection {
    fn default() -> Self {
        AnnotateSection {
            enabled: true,
            engine_depth: None,
        }
    }
}

/// `true`, the default for [`AnnotateSection::enabled`].
fn default_true() -> bool {
    true
}

/// TOML `[analyze]` section.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnalyzeSection {
    /// Names of analyzers to run, in order; must name at least one registered, non-duplicate
    /// analyzer.
    #[serde(default = "default_analyzers")]
    pub analyzers: Vec<String>,
    /// Whether a failed analyzer check should be treated as fatal by the caller.
    #[serde(default)]
    pub strict: bool,
    /// Diversity thresholds handed to analyzers that check diversity.
    #[serde(default)]
    pub thresholds: DiversityThresholds,
}

impl Default for AnalyzeSection {
    fn default() -> Self {
        AnalyzeSection {
            analyzers: default_analyzers(),
            strict: false,
            thresholds: DiversityThresholds::default(),
        }
    }
}

/// `["summary"]`, the default for [`AnalyzeSection::analyzers`].
fn default_analyzers() -> Vec<String> {
    vec!["summary".to_string()]
}

/// TOML `[report]` section.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReportSection {
    /// Report output path, relative to the experiment's resolved `out` directory.
    #[serde(default = "default_report_out")]
    pub out: PathBuf,
}

impl Default for ReportSection {
    fn default() -> Self {
        ReportSection {
            out: default_report_out(),
        }
    }
}

/// `"report.md"`, the default for [`ReportSection::out`].
fn default_report_out() -> PathBuf {
    PathBuf::from("report.md")
}

impl<A: DeserializeOwned> ExperimentConfig<A> {
    /// Parses an [`ExperimentConfig`] from an in-memory TOML document. Parse errors are
    /// reported as [`IoError::Toml`] with `path` set to `"<string>"`. Performs no validation;
    /// see [`resolve_experiment`].
    pub fn from_toml_str(text: &str) -> Result<Self, CorpusError> {
        toml::from_str(text)
            .map_err(|e| IoError::Toml {
                path: PathBuf::from("<string>"),
                message: e.to_string(),
            })
            .map_err(CorpusError::from)
    }
}

/// Loads an [`ExperimentConfig`] from a TOML file on disk. A read failure is reported as
/// [`IoError::Io`]; a parse failure as [`IoError::Toml`], both carrying `path`. Performs no
/// validation; see [`resolve_experiment`].
pub fn load_experiment<A: DeserializeOwned>(path: &Path) -> Result<ExperimentConfig<A>, CorpusError> {
    let text = std::fs::read_to_string(path).map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text)
        .map_err(|e| IoError::Toml {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
        .map_err(CorpusError::from)
}

/// An [`ExperimentConfig`] with relative paths resolved against its `base_dir`, its sweep
/// loaded (but not yet enumerated into cells — that happens when the `generate` stage runs),
/// and every section reduced to the plain options types the pipeline stages already take.
#[derive(Debug, Clone)]
pub struct ResolvedExperiment<A> {
    /// Experiment name.
    pub name: String,
    /// Game name.
    pub game: String,
    /// Output directory, resolved against `base_dir` or overridden by `out_override`.
    pub out: PathBuf,
    /// The loaded (but unresolved) sweep configuration.
    pub sweep: GenerateConfig<A>,
    /// Options for the `generate` stage.
    pub generate: GenerateOptions,
    /// Options for the `annotate` stage; `None` when annotation is disabled.
    pub annotate: Option<AnnotateOptions>,
    /// Options for the `analyze` stage.
    pub analyze: AnalyzeOptions,
    /// Report output path, resolved against `out`.
    pub report_out: PathBuf,
}

/// Validates `config` against `bundle` and `registry`, loads its sweep, and resolves every
/// relative path against `base_dir` (the experiment file's parent directory) unless overridden
/// by `out_override` (a CLI `--out`). Every failure is reported as [`CorpusError::Config`]
/// except a missing or unparsable sweep file, which surfaces as whatever [`CorpusError`] variant
/// [`GenerateConfig::from_toml_file`] produces. See the module docs for the TOML shape validated.
pub fn resolve_experiment<G: EngineGame + CorpusGame>(
    config: ExperimentConfig<G::Action>,
    base_dir: &Path,
    bundle: &GameBundle<G>,
    registry: &AnalyzerRegistry<G>,
    out_override: Option<&Path>,
) -> Result<ResolvedExperiment<G::Action>, CorpusError> {
    if config.schema_version != SCHEMA_VERSION {
        return Err(CorpusError::Config(format!(
            "schema_version {} is not the supported {SCHEMA_VERSION}",
            config.schema_version
        )));
    }
    if config.name.is_empty() {
        return Err(CorpusError::Config("name must not be empty".to_string()));
    }
    if config.game != bundle.name {
        return Err(CorpusError::Config(format!(
            "game `{}` does not match bundle `{}`",
            config.game, bundle.name
        )));
    }
    if config.generate.threads == Some(0) {
        return Err(CorpusError::Config("[generate] threads must be >= 1, got 0".to_string()));
    }

    let GenerateSection {
        sweep: sweep_source,
        threads,
        serial,
    } = config.generate;
    let sweep = match sweep_source {
        SweepSource::Path(p) => GenerateConfig::<G::Action>::from_toml_file(&base_dir.join(&p))?,
        SweepSource::Inline(cfg) => cfg,
    };
    if sweep.game != config.game {
        return Err(CorpusError::Config(format!(
            "game `{}` does not match the sweep's game `{}`",
            config.game, sweep.game
        )));
    }
    resolve(sweep.clone(), bundle)?;

    let names = registry.names().join(", ");
    if config.analyze.analyzers.is_empty() {
        return Err(CorpusError::Config(format!(
            "[analyze] analyzers must name at least one analyzer; known analyzers: {names}"
        )));
    }
    let mut seen: Vec<&str> = Vec::with_capacity(config.analyze.analyzers.len());
    for name in &config.analyze.analyzers {
        if seen.contains(&name.as_str()) {
            return Err(CorpusError::Config(format!(
                "[analyze] analyzers lists `{name}` more than once"
            )));
        }
        seen.push(name.as_str());
        let analyzer = registry.get(name).ok_or_else(|| {
            CorpusError::Config(format!(
                "unknown analyzer `{name}` in [analyze] analyzers; known analyzers: {names}"
            ))
        })?;
        if analyzer.requires_annotations() && !config.annotate.enabled {
            return Err(CorpusError::Config(format!(
                "analyzer `{name}` requires annotations; set [annotate] enabled = true or drop it"
            )));
        }
    }

    let AnnotateSection { enabled, engine_depth } = config.annotate;
    let AnalyzeSection {
        analyzers,
        strict,
        thresholds,
    } = config.analyze;

    let out = out_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base_dir.join(&config.out));
    let report_out = out.join(&config.report.out);
    let generate = GenerateOptions { threads, serial };
    let annotate = enabled.then_some(AnnotateOptions {
        engine_depth,
        solver_limit: DEFAULT_SOLVER_LIMIT,
    });
    let analyze = AnalyzeOptions {
        analyzers,
        thresholds,
        strict,
    };

    Ok(ResolvedExperiment {
        name: config.name,
        game: config.game,
        out,
        sweep,
        generate,
        annotate,
        analyze,
        report_out,
    })
}

/// A standalone strategy roster file: `[[strategies]]` entries in the same shape as a
/// [`GenerateConfig::strategies`](crate::discovery::config::GenerateConfig)'s sweep list.
/// Reused by a later phase (e.g. a `play` command that loads a roster independent of any
/// sweep).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StrategyFile {
    /// Schema version this file was authored against; must equal [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Named strategy specs, in TOML `[[strategies]]` + `[strategies.spec]` form.
    #[serde(default = "Vec::new")]
    pub strategies: Vec<RosterEntry>,
}

impl StrategyFile {
    /// Parses a [`StrategyFile`] from an in-memory TOML document. Parse errors are reported as
    /// [`IoError::Toml`] with `path` set to `"<string>"`.
    pub fn from_toml_str(text: &str) -> Result<Self, CorpusError> {
        toml::from_str(text)
            .map_err(|e| IoError::Toml {
                path: PathBuf::from("<string>"),
                message: e.to_string(),
            })
            .map_err(CorpusError::from)
    }

    /// Parses a [`StrategyFile`] from a TOML file on disk, then checks its `schema_version`
    /// against [`SCHEMA_VERSION`]. A read failure is reported as [`IoError::Io`]; a parse
    /// failure as [`IoError::Toml`].
    pub fn from_toml_file(path: &Path) -> Result<Self, CorpusError> {
        let text = std::fs::read_to_string(path).map_err(|source| IoError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let file: Self = toml::from_str(&text)
            .map_err(|e| IoError::Toml {
                path: path.to_path_buf(),
                message: e.to_string(),
            })
            .map_err(CorpusError::from)?;
        check_schema_version(path, file.schema_version)?;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::analyze::{AnalyzeContext, Analyzer, AnalyzerOutput, builtin_registry};
    use crate::games::tictactoe::{Move, TicTacToe, game_bundle};
    use crate::strategy::minimax::{MinimaxConfig, TieBreak};
    use crate::strategy::registry::StrategySpec;
    use std::sync::Arc;

    /// A minimal `"agreement"` analyzer stub: requires annotations; its [`Analyzer::run`] is
    /// never expected to be called by these tests, which only exercise validation.
    struct StubAgreement;

    impl Analyzer<TicTacToe> for StubAgreement {
        fn name(&self) -> &str {
            "agreement"
        }
        fn description(&self) -> &str {
            "stub"
        }
        fn requires_annotations(&self) -> bool {
            true
        }
        fn run(&self, _ctx: &AnalyzeContext<'_, TicTacToe>, _options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
            Err(CorpusError::Config("stub".into()))
        }
        fn render(&self, _output: &serde_json::Value) -> Result<String, CorpusError> {
            Ok(String::new())
        }
    }

    /// [`builtin_registry`] plus [`StubAgreement`], for exercising `[analyze] analyzers` /
    /// `[annotate] enabled` validation against a registered analyzer that requires annotations.
    fn registry_with_agreement() -> AnalyzerRegistry<TicTacToe> {
        let mut registry = builtin_registry::<TicTacToe>();
        registry.register(Arc::new(StubAgreement));
        registry
    }

    /// Loads the checked-in small experiment fixture.
    fn fixture() -> ExperimentConfig<Move> {
        load_experiment::<Move>(Path::new("tests/fixtures/experiment-small.toml")).unwrap()
    }

    /// [`fixture`] with `mutate` applied.
    fn mutated(mutate: impl FnOnce(&mut ExperimentConfig<Move>)) -> ExperimentConfig<Move> {
        let mut config = fixture();
        mutate(&mut config);
        config
    }

    #[test]
    fn parses_path_sweep_and_defaults() {
        let text = r#"
schema_version = 1
name = "n"
game = "tictactoe"
out = "runs/n"

[generate]
sweep = "x.toml"
"#;
        let config = ExperimentConfig::<Move>::from_toml_str(text).unwrap();

        assert_eq!(config.generate.sweep, SweepSource::Path(PathBuf::from("x.toml")));
        assert_eq!(config.generate.threads, None);
        assert!(!config.generate.serial);
        assert_eq!(config.annotate, AnnotateSection::default());
        assert!(config.annotate.enabled);
        assert_eq!(config.analyze.analyzers, vec!["summary".to_string()]);
        assert!(!config.analyze.strict);
        assert_eq!(config.analyze.thresholds, DiversityThresholds::default());
        assert_eq!(config.report.out, PathBuf::from("report.md"));
    }

    #[test]
    fn parses_inline_sweep() {
        let text = r#"
schema_version = 1
name = "n"
game = "tictactoe"
out = "runs/n"

[generate]
threads = 2

[generate.sweep]
schema_version = 1
game = "tictactoe"
seed = 7
games_per_cell = 2
pairings = [["random", "random"]]

[[generate.sweep.strategies]]
name = "random"
[generate.sweep.strategies.spec]
kind = "random"
"#;
        let config = ExperimentConfig::<Move>::from_toml_str(text).unwrap();

        assert_eq!(config.generate.threads, Some(2));
        match config.generate.sweep {
            SweepSource::Inline(cfg) => {
                assert_eq!(cfg.seed, 7);
                assert_eq!(cfg.strategies.len(), 1);
            }
            other => panic!("expected Inline, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_fields() {
        let top_level = r#"
schema_version = 1
name = "n"
game = "tictactoe"
out = "runs/n"
bogus = 1

[generate]
sweep = "x.toml"
"#;
        match ExperimentConfig::<Move>::from_toml_str(top_level) {
            Err(CorpusError::Io(IoError::Toml { message, .. })) => assert!(message.contains("bogus")),
            other => panic!("expected Err(Io(Toml {{ .. }})), got {other:?}"),
        }

        let generate_section = r#"
schema_version = 1
name = "n"
game = "tictactoe"
out = "runs/n"

[generate]
sweep = "x.toml"
bogus = 1
"#;
        match ExperimentConfig::<Move>::from_toml_str(generate_section) {
            Err(CorpusError::Io(IoError::Toml { message, .. })) => assert!(message.contains("bogus")),
            other => panic!("expected Err(Io(Toml {{ .. }})), got {other:?}"),
        }
    }

    #[test]
    fn fixture_files_parse() {
        let config = fixture();
        assert_eq!(config.name, "experiment-small");
        assert_eq!(config.generate.sweep, SweepSource::Path(PathBuf::from("generate-small.toml")));
        assert_eq!(config.out, PathBuf::from("runs/experiment-small"));
        assert_eq!(config.analyze.analyzers, vec!["summary".to_string(), "agreement".to_string()]);
        assert_eq!(config.analyze.thresholds.min_canonical_coverage, 0.25);
        assert_eq!(config.analyze.thresholds.min_decisive_fraction, 0.2);
        assert_eq!(config.analyze.thresholds.min_distinct_game_fraction, 0.5);
        assert!(config.annotate.enabled);

        let default_config = load_experiment::<Move>(Path::new("configs/tictactoe-experiment.toml")).unwrap();
        assert_eq!(
            default_config.generate.sweep,
            SweepSource::Path(PathBuf::from("tictactoe-default.toml"))
        );
        assert_eq!(default_config.analyze.analyzers.len(), 2);
    }

    #[test]
    fn resolves_relative_paths_and_out_override() {
        let bundle = game_bundle();
        let registry = registry_with_agreement();

        let resolved = resolve_experiment::<TicTacToe>(fixture(), Path::new("tests/fixtures"), &bundle, &registry, None).unwrap();

        assert_eq!(resolved.sweep.seed, 20260821);
        assert_eq!(resolved.sweep.games_per_cell, 4);
        assert_eq!(resolved.out, Path::new("tests/fixtures").join("runs/experiment-small"));
        assert_eq!(resolved.report_out, resolved.out.join("report.md"));
        assert!(resolved.annotate.is_some());
        assert_eq!(
            resolved.analyze.analyzers,
            vec!["summary".to_string(), "agreement".to_string()]
        );
        assert_eq!(resolved.analyze.thresholds.min_canonical_coverage, 0.25);

        let overridden = resolve_experiment::<TicTacToe>(
            fixture(),
            Path::new("tests/fixtures"),
            &bundle,
            &registry,
            Some(Path::new("target/exp-out")),
        )
        .unwrap();
        assert_eq!(overridden.out, PathBuf::from("target/exp-out"));
        assert_eq!(overridden.report_out, PathBuf::from("target/exp-out").join("report.md"));

        let default_resolved = resolve_experiment::<TicTacToe>(
            load_experiment::<Move>(Path::new("configs/tictactoe-experiment.toml")).unwrap(),
            Path::new("configs"),
            &bundle,
            &registry,
            None,
        )
        .unwrap();
        assert_eq!(default_resolved.sweep.games_per_cell, 20);
    }

    #[test]
    fn validation_errors_name_field_and_value() {
        let bundle = game_bundle();
        let base_dir = Path::new("tests/fixtures");
        let registry = registry_with_agreement();

        let mut chess_inline_sweep =
            GenerateConfig::<Move>::from_toml_file(Path::new("tests/fixtures/generate-small.toml")).unwrap();
        chess_inline_sweep.game = "chess".to_string();

        let cases: Vec<(ExperimentConfig<Move>, Vec<&str>)> = vec![
            (mutated(|c| c.schema_version = 2), vec!["schema_version 2"]),
            (mutated(|c| c.name = String::new()), vec!["name must not be empty"]),
            (mutated(|c| c.game = "chess".to_string()), vec!["game `chess`"]),
            (mutated(|c| c.generate.threads = Some(0)), vec!["threads must be >= 1"]),
            (mutated(|c| c.analyze.analyzers = vec![]), vec!["at least one analyzer"]),
            (
                mutated(|c| c.analyze.analyzers = vec!["summary".to_string(), "summary".to_string()]),
                vec!["more than once"],
            ),
            (
                mutated(|c| c.analyze.analyzers = vec!["nope".to_string()]),
                vec!["unknown analyzer `nope`", "summary"],
            ),
            (
                mutated(|c| {
                    c.analyze.analyzers = vec!["agreement".to_string()];
                    c.annotate.enabled = false;
                }),
                vec!["requires annotations"],
            ),
            (
                mutated(|c| c.generate.sweep = SweepSource::Inline(chess_inline_sweep)),
                vec!["does not match the sweep's game"],
            ),
        ];

        for (config, expected) in cases {
            match resolve_experiment(config, base_dir, &bundle, &registry, None) {
                Err(CorpusError::Config(msg)) => {
                    for fragment in expected {
                        assert!(msg.contains(fragment), "expected {fragment:?} in {msg:?}");
                    }
                }
                other => panic!("expected Err(Config(_)), got {other:?}"),
            }
        }

        let missing_sweep = mutated(|c| c.generate.sweep = SweepSource::Path(PathBuf::from("missing-sweep.toml")));
        match resolve_experiment(missing_sweep, base_dir, &bundle, &registry, None) {
            Err(CorpusError::Io(IoError::Io { .. })) => {}
            other => panic!("expected Err(Io(Io {{ .. }})), got {other:?}"),
        }
        let missing_sweep = mutated(|c| c.generate.sweep = SweepSource::Path(PathBuf::from("missing-sweep.toml")));
        let err = resolve_experiment(missing_sweep, base_dir, &bundle, &registry, None).unwrap_err();
        assert!(err.to_string().contains("missing-sweep.toml"));
    }

    #[test]
    fn strategy_file_parses_strategies_shape() {
        let text = r#"
schema_version = 1

[[strategies]]
name = "perfect"
[strategies.spec]
kind = "minimax"
depth = 9

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"
"#;
        let file = StrategyFile::from_toml_str(text).unwrap();

        assert_eq!(file.strategies.len(), 2);
        assert_eq!(
            file.strategies[0].spec,
            StrategySpec::Minimax(MinimaxConfig {
                depth: 9,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            })
        );
        assert_eq!(file.strategies[1].spec, StrategySpec::Random);

        match StrategyFile::from_toml_file(Path::new("tests/fixtures/does-not-exist-strategies.toml")) {
            Err(CorpusError::Io(IoError::Io { .. })) => {}
            other => panic!("expected Err(Io(Io {{ .. }})), got {other:?}"),
        }
    }
}
