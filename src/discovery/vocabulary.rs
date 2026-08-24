//! The vocabulary analyzer: reports the given-versus-discovered split of the features mined
//! heuristics actually reference, per candidate and overall, over `heuristics.json`.

use std::collections::BTreeSet;

use crate::core::features::Tier;
use crate::discovery::analyze::{AnalyzeContext, AnalyzeOptions, Analyzer, AnalyzerOutput};
use crate::discovery::config::CorpusError;
use crate::io::{
    CandidateUsage, CorpusGame, HEURISTICS_FILE, IoError, MiningReport, SCHEMA_VERSION, UsageBreakdown, VOCABULARY_FILE,
    VocabularyReport, read_json,
};
use crate::strategy::engine::EngineGame;

/// The `vocabulary` analyzer: reports given-versus-discovered feature usage of mined heuristics.
pub struct VocabularyAnalyzer;

/// Maps a [`Tier`] to its `vocabulary.json` breakdown key.
fn tier_key(tier: Tier) -> &'static str {
    match tier {
        Tier::Primitive => "primitive",
        Tier::Supplied => "supplied",
        Tier::Invented => "invented",
    }
}

/// Builds a [`UsageBreakdown`] over `names`, resolving each name's tier via `tier_of`. Every
/// breakdown carries exactly the three tier keys, zeroed when absent.
fn breakdown(names: &BTreeSet<String>, tier_of: impl Fn(&str) -> Option<Tier>) -> Result<UsageBreakdown, CorpusError> {
    let mut counts = std::collections::BTreeMap::new();
    counts.insert("primitive".to_string(), 0usize);
    counts.insert("supplied".to_string(), 0usize);
    counts.insert("invented".to_string(), 0usize);

    for name in names {
        let tier = tier_of(name)
            .ok_or_else(|| CorpusError::Precondition(format!("candidate does not define referenced feature `{name}`")))?;
        *counts.get_mut(tier_key(tier)).expect("seeded with all three keys") += 1;
    }

    let referenced = names.len();
    let mut fractions = std::collections::BTreeMap::new();
    for key in ["primitive", "supplied", "invented"] {
        let count = counts[key];
        let fraction = if referenced == 0 {
            0.0
        } else {
            count as f64 / referenced as f64
        };
        fractions.insert(key.to_string(), fraction);
    }

    Ok(UsageBreakdown {
        referenced,
        counts,
        fractions,
    })
}

/// Computes the [`VocabularyReport`] for `report`: per-candidate and overall feature-usage
/// breakdowns, plus the dataset's column counts by tier.
pub(crate) fn vocabulary_report(report: &MiningReport) -> Result<VocabularyReport, CorpusError> {
    let mut candidates = Vec::with_capacity(report.candidates.len());
    let mut union: BTreeSet<String> = BTreeSet::new();

    for candidate in &report.candidates {
        let refs = candidate.heuristic.references();
        union.extend(refs.iter().cloned());
        let usage = breakdown(&refs, |name| candidate.heuristic.definitions.get(name).map(|d| d.tier))?;
        candidates.push(CandidateUsage {
            name: candidate.name.clone(),
            usage,
        });
    }

    let overall = breakdown(&union, |name| {
        report
            .candidates
            .iter()
            .find_map(|c| c.heuristic.definitions.get(name).map(|d| d.tier))
    })?;

    let mut tiers = std::collections::BTreeMap::new();
    tiers.insert("primitive".to_string(), 0usize);
    tiers.insert("supplied".to_string(), 0usize);
    tiers.insert("invented".to_string(), 0usize);
    for column in &report.dataset.columns {
        *tiers.get_mut(tier_key(column.tier)).expect("seeded with all three keys") += 1;
    }

    Ok(VocabularyReport {
        schema_version: SCHEMA_VERSION,
        game: report.game.clone(),
        run_id: report.run_id.clone(),
        tiers,
        candidates,
        overall,
    })
}

impl<G: EngineGame + CorpusGame> Analyzer<G> for VocabularyAnalyzer {
    fn name(&self) -> &str {
        "vocabulary"
    }
    fn description(&self) -> &str {
        "reports given-versus-discovered feature usage of mined heuristics"
    }
    fn requires_annotations(&self) -> bool {
        false
    }
    fn run(&self, ctx: &AnalyzeContext<'_, G>, _options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
        let path = ctx.corpus_dir.join(HEURISTICS_FILE);
        if !path.is_file() {
            return Err(CorpusError::Precondition(
                "heuristics.json not found; run the `mine` analyzer first".to_string(),
            ));
        }
        let report: MiningReport = read_json(&path)?;
        AnalyzerOutput::new(VOCABULARY_FILE, &vocabulary_report(&report)?, true)
    }
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError> {
        let report: VocabularyReport = serde_json::from_value(output.clone())
            .map_err(|e| CorpusError::Io(IoError::Invalid(format!("{VOCABULARY_FILE}: {e}"))))?;
        Ok(render_vocabulary(&report))
    }
}

/// Renders `report` into the deterministic Markdown produced by [`VocabularyAnalyzer::render`]: a
/// `## Vocabulary` table, one row per candidate in report order plus a final `overall` row, the
/// whole string ending in a single `\n`.
fn render_vocabulary(report: &VocabularyReport) -> String {
    let mut lines = vec![
        "## Vocabulary".to_string(),
        String::new(),
        "| candidate | referenced | tier-1 | tier-2 | tier-3 |".to_string(),
        "| --- | --- | --- | --- | --- |".to_string(),
    ];
    for candidate in &report.candidates {
        lines.push(format!(
            "| {} | {} | {:.3} | {:.3} | {:.3} |",
            candidate.name,
            candidate.usage.referenced,
            candidate.usage.fractions["primitive"],
            candidate.usage.fractions["supplied"],
            candidate.usage.fractions["invented"]
        ));
    }
    lines.push(format!(
        "| overall | {} | {:.3} | {:.3} | {:.3} |",
        report.overall.referenced,
        report.overall.fractions["primitive"],
        report.overall.fractions["supplied"],
        report.overall.fractions["invented"]
    ));
    format!("{}\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dsl::{ActionSelector, HeuristicStrategy, Rule};
    use crate::core::features::{CmpOp, FeatureDef, FeatureExpr};
    use crate::games::tictactoe::TicTacToe;
    use crate::io::{DatasetColumn, DatasetManifest, MineParams, MinedHeuristic};
    use std::collections::BTreeMap;

    fn hand_built_report() -> MiningReport {
        let columns = vec![
            DatasetColumn {
                name: "a".to_string(),
                tier: Tier::Primitive,
                kind: "int".to_string(),
                description: String::new(),
            },
            DatasetColumn {
                name: "b".to_string(),
                tier: Tier::Supplied,
                kind: "set".to_string(),
                description: String::new(),
            },
            DatasetColumn {
                name: "c".to_string(),
                tier: Tier::Invented,
                kind: "bool".to_string(),
                description: String::new(),
            },
        ];

        let dataset = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
            tiers: vec!["primitive".to_string(), "supplied".to_string(), "invented".to_string()],
            columns,
            classes: Vec::new(),
            rows: 0,
            annotated: 0,
            nonterminal: 0,
            collapsed: 0,
            label_counts: BTreeMap::new(),
            value_counts: BTreeMap::new(),
            side_to_move_counts: BTreeMap::new(),
            unlabeled: 0,
        };

        let mut cand1 = HeuristicStrategy::new("cand-1");
        cand1.define(FeatureDef::native("a", Tier::Primitive, "a")).unwrap();
        cand1.define(FeatureDef::native("b", Tier::Supplied, "b")).unwrap();
        cand1
            .define(FeatureDef::derived(
                "c",
                Tier::Invented,
                "c",
                FeatureExpr::compare(CmpOp::Gt, FeatureExpr::named("a"), FeatureExpr::int(0)),
            ))
            .unwrap();
        cand1.push_rule(Rule::new(
            "r1",
            1,
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::named("a"), FeatureExpr::int(0)),
            ActionSelector::TargetIn {
                expr: FeatureExpr::named("b"),
            },
        ));
        cand1.push_rule(Rule::new("r2", 0, FeatureExpr::named("c"), ActionSelector::AnyLegal));
        cand1.fallback = ActionSelector::AnyLegal;

        let mined1 = MinedHeuristic {
            name: "cand-1".to_string(),
            engine: "cart".to_string(),
            max_depth: 0,
            min_leaf: 0,
            heuristic: cand1,
            rules: 0,
            leaves: 0,
            depth: 0,
            train_rows: 0,
            holdout_rows: 0,
            train_accuracy: 0.0,
            train_soundness: 0.0,
            holdout_accuracy: None,
            holdout_soundness: None,
            fallback_rows: 0,
            evidence: Vec::new(),
        };

        let mut cand2 = HeuristicStrategy::new("cand-2");
        cand2.define(FeatureDef::native("a", Tier::Primitive, "a")).unwrap();
        cand2.push_rule(Rule::new(
            "r1",
            0,
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::named("a"), FeatureExpr::int(0)),
            ActionSelector::AnyLegal,
        ));
        cand2.fallback = ActionSelector::AnyLegal;

        let mined2 = MinedHeuristic {
            name: "cand-2".to_string(),
            engine: "cart".to_string(),
            max_depth: 0,
            min_leaf: 0,
            heuristic: cand2,
            rules: 0,
            leaves: 0,
            depth: 0,
            train_rows: 0,
            holdout_rows: 0,
            train_accuracy: 0.0,
            train_soundness: 0.0,
            holdout_accuracy: None,
            holdout_soundness: None,
            fallback_rows: 0,
            evidence: Vec::new(),
        };

        MiningReport {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            params: MineParams::default(),
            dataset,
            candidates: vec![mined1, mined2],
        }
    }

    #[test]
    fn vocabulary_fractions_from_hand_built_report() {
        let report = hand_built_report();
        let vr = vocabulary_report(&report).unwrap();

        let cand1 = &vr.candidates[0];
        assert_eq!(cand1.name, "cand-1");
        assert_eq!(cand1.usage.referenced, 3);
        assert_eq!(cand1.usage.counts["primitive"], 1);
        assert_eq!(cand1.usage.counts["supplied"], 1);
        assert_eq!(cand1.usage.counts["invented"], 1);
        for key in ["primitive", "supplied", "invented"] {
            assert!((cand1.usage.fractions[key] - (1.0 / 3.0)).abs() < 1e-9);
        }

        let cand2 = &vr.candidates[1];
        assert_eq!(cand2.name, "cand-2");
        assert_eq!(cand2.usage.referenced, 1);
        assert_eq!(cand2.usage.counts["primitive"], 1);
        assert_eq!(cand2.usage.counts["supplied"], 0);
        assert_eq!(cand2.usage.counts["invented"], 0);
        assert_eq!(cand2.usage.fractions["primitive"], 1.0);
        assert_eq!(cand2.usage.fractions["supplied"], 0.0);
        assert_eq!(cand2.usage.fractions["invented"], 0.0);

        assert_eq!(vr.overall.referenced, 3);
        assert_eq!(vr.overall.counts["primitive"], 1);
        assert_eq!(vr.overall.counts["supplied"], 1);
        assert_eq!(vr.overall.counts["invented"], 1);

        assert_eq!(vr.tiers["primitive"], 1);
        assert_eq!(vr.tiers["supplied"], 1);
        assert_eq!(vr.tiers["invented"], 1);

        for m in [
            &cand1.usage.counts.keys().cloned().collect::<Vec<_>>(),
            &cand1.usage.fractions.keys().cloned().collect::<Vec<_>>(),
            &vr.overall.counts.keys().cloned().collect::<Vec<_>>(),
            &vr.overall.fractions.keys().cloned().collect::<Vec<_>>(),
            &vr.tiers.keys().cloned().collect::<Vec<_>>(),
        ] {
            assert_eq!(
                m,
                &vec!["invented".to_string(), "primitive".to_string(), "supplied".to_string()]
            );
        }
    }

    #[test]
    fn vocabulary_render_shape() {
        let report = hand_built_report();
        let vr = vocabulary_report(&report).unwrap();

        let rendered =
            <VocabularyAnalyzer as Analyzer<TicTacToe>>::render(&VocabularyAnalyzer, &serde_json::to_value(&vr).unwrap()).unwrap();

        assert!(rendered.starts_with("## Vocabulary\n\n"));
        assert!(rendered.contains("| candidate | referenced | tier-1 | tier-2 | tier-3 |"));
        assert!(rendered.contains("| overall | 3 | 0.333 | 0.333 | 0.333 |"));
        assert!(rendered.contains("| cand-2 | 1 | 1.000 | 0.000 | 0.000 |"));
        assert!(rendered.ends_with('\n'));
        assert!(!rendered.ends_with("\n\n"));
        assert!(!rendered.contains("|---"));
    }
}
