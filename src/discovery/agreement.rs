//! Agreement analyzer: engine-optimality rates of every played action, by strategy and by ply.
//!
//! [`AgreementAnalyzer`] joins every played position to the engine ground truth annotated for the
//! same RAW `state` and reports how often the played action was one of the annotation's optimal
//! actions, broken down by the strategy that chose it ([`AgreementReport::by_strategy`]) and by
//! ply ([`AgreementReport::by_ply`]), plus an [`AgreementReport::overall`] tally. Its output file
//! (`agreement.json`) is byte-stable across repeated runs (ADR 0009): every map in
//! [`AgreementReport`] is a `BTreeMap`.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::discovery::analyze::{AnalyzeContext, AnalyzeOptions, Analyzer, AnalyzerOutput};
use crate::discovery::config::CorpusError;
use crate::io::{CorpusGame, IoError, SCHEMA_VERSION};
use crate::strategy::engine::EngineGame;

/// File name this analyzer writes into the corpus directory.
pub const AGREEMENT_FILE: &str = "agreement.json";

/// Agreement tally over a set of positions.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct AgreementCounts {
    /// Positions tallied.
    pub positions: usize,
    /// Of `positions`, how many played an action in the annotation's optimal set.
    pub agreeing: usize,
    /// `agreeing / positions`, or `0.0` when `positions == 0`.
    pub rate: f64,
}

/// Engine-agreement report over one corpus run, written to `agreement.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AgreementReport {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// `run_id` of the corpus this report was computed from.
    pub run_id: String,
    /// Total positions tallied.
    pub positions: usize,
    /// Tallies keyed by the `chosen_by` of the position (a strategy entry name, or
    /// `opening` / `random-opening` for forced plies).
    pub by_strategy: BTreeMap<String, AgreementCounts>,
    /// Tallies keyed by ply index.
    pub by_ply: BTreeMap<usize, AgreementCounts>,
    /// Tally over every position.
    pub overall: AgreementCounts,
}

/// Sets `rate` from `agreeing` and `positions`, leaving no rate stale.
fn finalize(counts: &mut AgreementCounts) {
    counts.rate = if counts.positions == 0 {
        0.0
    } else {
        counts.agreeing as f64 / counts.positions as f64
    };
}

/// The `agreement` analyzer.
pub struct AgreementAnalyzer;

impl<G: EngineGame + CorpusGame> Analyzer<G> for AgreementAnalyzer {
    fn name(&self) -> &str {
        "agreement"
    }
    fn description(&self) -> &str {
        "engine-agreement rates of played actions, by strategy and ply (agreement.json)"
    }
    fn requires_annotations(&self) -> bool {
        true
    }
    fn run(&self, ctx: &AnalyzeContext<'_, G>, _options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
        let annotations = ctx.annotations()?;
        let by_state: HashMap<&G::State, _> = annotations.iter().map(|record| (&record.state, record)).collect();

        let mut by_strategy: BTreeMap<String, AgreementCounts> = BTreeMap::new();
        let mut by_ply: BTreeMap<usize, AgreementCounts> = BTreeMap::new();
        let mut overall = AgreementCounts::default();

        for position in ctx.positions {
            let annotation = by_state.get(&position.state).ok_or_else(|| {
                CorpusError::Precondition(format!(
                    "annotations.jsonl does not cover every position of {}; re-run `annotate --corpus {}`",
                    ctx.corpus_dir.display(),
                    ctx.corpus_dir.display()
                ))
            })?;

            let agrees = annotation.optimal_actions.contains(&position.chosen_action);

            let strategy_counts = by_strategy.entry(position.chosen_by.clone()).or_default();
            strategy_counts.positions += 1;
            if agrees {
                strategy_counts.agreeing += 1;
            }

            let ply_counts = by_ply.entry(position.ply).or_default();
            ply_counts.positions += 1;
            if agrees {
                ply_counts.agreeing += 1;
            }

            overall.positions += 1;
            if agrees {
                overall.agreeing += 1;
            }
        }

        for counts in by_strategy.values_mut() {
            finalize(counts);
        }
        for counts in by_ply.values_mut() {
            finalize(counts);
        }
        finalize(&mut overall);

        let report = AgreementReport {
            schema_version: SCHEMA_VERSION,
            run_id: ctx.run_id.clone(),
            positions: overall.positions,
            by_strategy,
            by_ply,
            overall,
        };
        AnalyzerOutput::new(AGREEMENT_FILE, &report, true)
    }
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError> {
        let report: AgreementReport = serde_json::from_value(output.clone())
            .map_err(|e| CorpusError::Io(IoError::Invalid(format!("{AGREEMENT_FILE}: {e}"))))?;
        Ok(render_agreement(&report))
    }
}

/// Renders `report` into the deterministic Markdown produced by [`AgreementAnalyzer::render`]: a
/// `## Agreement` overview table followed by `### By strategy` and `### By ply` sections, each
/// heading and each table set off by blank lines, the whole string ending in a single `\n`.
fn render_agreement(report: &AgreementReport) -> String {
    let mut blocks: Vec<String> = Vec::new();

    blocks.push("## Agreement".to_string());
    blocks.push(
        [
            "| metric | value |".to_string(),
            "| --- | --- |".to_string(),
            format!("| run_id | {} |", report.run_id),
            format!("| positions | {} |", report.positions),
            format!("| agreeing | {} |", report.overall.agreeing),
            format!("| rate | {:.3} |", report.overall.rate),
        ]
        .join("\n"),
    );

    blocks.push("### By strategy".to_string());
    let mut strategy_rows = vec![
        "| strategy | positions | agreeing | rate |".to_string(),
        "| --- | --- | --- | --- |".to_string(),
    ];
    for (strategy, counts) in &report.by_strategy {
        strategy_rows.push(format!(
            "| {strategy} | {} | {} | {:.3} |",
            counts.positions, counts.agreeing, counts.rate
        ));
    }
    blocks.push(strategy_rows.join("\n"));

    blocks.push("### By ply".to_string());
    let mut ply_rows = vec![
        "| ply | positions | agreeing | rate |".to_string(),
        "| --- | --- | --- | --- |".to_string(),
    ];
    for (ply, counts) in &report.by_ply {
        ply_rows.push(format!(
            "| {ply} | {} | {} | {:.3} |",
            counts.positions, counts.agreeing, counts.rate
        ));
    }
    blocks.push(ply_rows.join("\n"));

    format!("{}\n", blocks.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::analyze::{analyze_outputs, builtin_registry};
    use crate::discovery::annotate::annotate_corpus;
    use crate::discovery::config::GenerateConfig;
    use crate::discovery::corpus::{GenerateOptions, generate};
    use crate::games::tictactoe::{Move, TicTacToe, game_bundle};
    use std::path::PathBuf;

    const CONFIG: &str = r#"
schema_version = 1
game = "tictactoe"
seed = 7
games_per_cell = 2
pairings = [["random", "perfect"], ["perfect", "random"]]

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"

[[strategies]]
name = "perfect"
[strategies.spec]
kind = "minimax"
depth = 9
"#;

    /// Generates and annotates the tiny [`CONFIG`] corpus into a fresh temp directory named
    /// after `name`.
    fn annotated_corpus(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sd-agreement-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bundle = game_bundle();
        let config = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();
        generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
        annotate_corpus(&bundle, &dir, &crate::discovery::annotate::AnnotateOptions::default()).unwrap();
        dir
    }

    #[test]
    fn agreement_rates_split_by_strategy() {
        let dir = annotated_corpus("split-by-strategy");
        let bundle = game_bundle();
        let registry = builtin_registry::<TicTacToe>();
        let options = AnalyzeOptions {
            analyzers: vec!["agreement".to_string()],
            ..AnalyzeOptions::default()
        };

        let (_, outputs) = analyze_outputs(&bundle, &dir, &registry, &options).unwrap();
        let report: AgreementReport = serde_json::from_str(&outputs[0].json).unwrap();

        assert_eq!(report.by_strategy["perfect"].rate, 1.0);
        assert!(report.by_strategy["random"].rate < 1.0);
        assert_eq!(
            report.overall.positions,
            report.by_strategy.values().map(|c| c.positions).sum::<usize>()
        );
        assert_eq!(
            report.overall.positions,
            report.by_ply.values().map(|c| c.positions).sum::<usize>()
        );

        assert_eq!(std::fs::read_to_string(dir.join(AGREEMENT_FILE)).unwrap(), outputs[0].json);
    }

    #[test]
    fn agreement_without_annotations_is_a_precondition_error() {
        let dir = std::env::temp_dir().join(format!("sd-agreement-{}-no-annotations", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bundle = game_bundle();
        let config = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();
        generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();

        let registry = builtin_registry::<TicTacToe>();
        let options = AnalyzeOptions {
            analyzers: vec!["agreement".to_string()],
            ..AnalyzeOptions::default()
        };

        match analyze_outputs(&bundle, &dir, &registry, &options) {
            Err(CorpusError::Precondition(msg)) => assert!(msg.contains("annotations.jsonl")),
            other => panic!("expected Err(Precondition(_)), got {other:?}"),
        }
    }

    #[test]
    fn render_starts_with_the_section_heading() {
        let dir = annotated_corpus("render");
        let bundle = game_bundle();
        let registry = builtin_registry::<TicTacToe>();
        let options = AnalyzeOptions {
            analyzers: vec!["agreement".to_string()],
            ..AnalyzeOptions::default()
        };

        let (_, outputs) = analyze_outputs(&bundle, &dir, &registry, &options).unwrap();
        let value = outputs[0].value().unwrap();

        let analyzer: &dyn Analyzer<TicTacToe> = &AgreementAnalyzer;
        let rendered = analyzer.render(&value).unwrap();

        assert!(rendered.starts_with("## Agreement\n\n"));
        assert!(rendered.contains("### By strategy"));
        assert!(rendered.contains("### By ply"));
        assert!(rendered.ends_with('\n'));
        assert!(!rendered.ends_with("\n\n"));
        assert!(!rendered.contains("|---"));
    }
}
