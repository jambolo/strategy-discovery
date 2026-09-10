//! Canonicalized per-position feature dataset for rule mining.
//!
//! [`build_dataset`] turns annotation records into a [`Dataset`]: one row per non-terminal
//! canonical state, with every native AND derived vocabulary feature encoded numerically and,
//! for each `set`-kind
//! feature ("action class"), whether it qualifies as an explanation of the optimal actions at
//! that row. [`dataset_from_context`] is the single path an [`crate::discovery::analyze::Analyzer`]
//! (here, [`DatasetAnalyzer`]) or a later miner uses to build one from an
//! [`crate::discovery::analyze::AnalyzeContext`]. Nothing here names a concrete game.

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use crate::core::features::{FeatureDef, FeatureValue, Tier};
use crate::core::featurizer::{Featurizer, FeaturizerSpec};
use crate::core::traits::GameDomain;
use crate::discovery::analyze::{AnalyzeContext, AnalyzeOptions, Analyzer, AnalyzerOutput, AnnRec};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::discovery::evaluate::mode_name;
use crate::io::{
    ANNOTATE_FILE, CorpusGame, DATASET_FILE, DATASET_MANIFEST_FILE, DatasetColumn, DatasetManifest, DatasetRow, InductionParams,
    IoError, SCHEMA_VERSION, read_json, write_jsonl,
};
use crate::strategy::engine::EngineGame;

/// Provenance of the annotation records a [`Dataset`] was built from.
#[derive(Debug, Clone)]
pub struct DatasetSource {
    /// `run_id` of the corpus the annotations came from, if any.
    pub run_id: Option<String>,
    /// Annotation mode of the input (`"corpus"` or `"exhaustive"`).
    pub annotations_mode: String,
}

/// A feature dataset: its manifest plus every row, in first-appearance order.
pub struct Dataset<G: GameDomain> {
    /// Manifest of this dataset (columns, classes, row statistics).
    pub manifest: DatasetManifest,
    /// Rows, one per non-terminal canonical state, in first-appearance order.
    pub rows: Vec<DatasetRow<G::State>>,
}

impl<G: GameDomain> Dataset<G> {
    /// Position of `name` among [`DatasetManifest::classes`].
    pub fn class_index(&self, name: &str) -> Option<usize> {
        self.manifest.classes.iter().position(|c| c == name)
    }

    /// Position of `name` among [`DatasetManifest::columns`].
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.manifest.columns.iter().position(|c| c.name == name)
    }
}

/// Maps `Tier` to the serde name used in [`DatasetManifest::tiers`].
fn tier_name(tier: Tier) -> &'static str {
    match tier {
        Tier::Primitive => "primitive",
        Tier::Supplied => "supplied",
        Tier::Invented => "invented",
    }
}

/// Builds a [`Dataset`] from annotation `records`, using `bundle`/`featurizer` for
/// canonicalization and feature extraction, then evaluating `featurizer.vocabulary()` (native
/// plus any derived definitions) over each extraction. See module docs for the row/label design.
pub fn build_dataset<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    featurizer: &Featurizer<G>,
    records: &[AnnRec<G>],
    source: DatasetSource,
) -> Result<Dataset<G>, CorpusError>
where
    G::State: Clone + Eq + Hash,
{
    let mut rows: Vec<DatasetRow<G::State>> = Vec::new();
    let mut index_of: HashMap<G::State, usize> = HashMap::new();
    let mut columns: Vec<DatasetColumn> = Vec::new();
    let mut classes: Vec<String> = Vec::new();
    let mut tiers: Vec<String> = Vec::new();

    let mut nonterminal = 0usize;

    for record in records {
        if record.terminal {
            continue;
        }
        nonterminal += 1;

        let (canonical, perm) = featurizer.canonical_frame(&record.state);
        if canonical != record.canonical_state {
            return Err(CorpusError::Precondition(
                "annotation canonical_state does not match the bundle canonicalizer".to_string(),
            ));
        }

        if let Some(&i) = index_of.get(&canonical) {
            rows[i].occurrences += 1;
            continue;
        }

        let legal = featurizer.rules().legal_actions(&record.state);
        let mut legal_positions = featurizer.action_positions(&legal, &perm).ok_or_else(|| {
            CorpusError::Precondition(
                "mining requires position-addressed actions (GamePrimitives::action_position returned None)".to_string(),
            )
        })?;
        legal_positions.sort_unstable();
        legal_positions.dedup();

        let mut optimal_positions = featurizer.action_positions(&record.optimal_actions, &perm).ok_or_else(|| {
            CorpusError::Precondition(
                "mining requires position-addressed actions (GamePrimitives::action_position returned None)".to_string(),
            )
        })?;
        optimal_positions.sort_unstable();
        optimal_positions.dedup();

        let native = featurizer.extract(&canonical);
        let env = featurizer
            .vocabulary()
            .evaluate(&native)
            .map_err(|e| CorpusError::Precondition(format!("evaluating derived features: {e}")))?;

        if rows.is_empty() {
            for def in featurizer.vocabulary().defs() {
                let value = env
                    .get(&def.name)
                    .ok_or_else(|| CorpusError::Precondition(format!("feature `{}` missing from extraction", def.name)))?;
                let kind = value.type_name().to_string();
                if kind == "set" {
                    classes.push(def.name.clone());
                }
                let tname = tier_name(def.tier).to_string();
                if !tiers.contains(&tname) {
                    tiers.push(tname);
                }
                columns.push(DatasetColumn {
                    name: def.name.clone(),
                    tier: def.tier,
                    kind,
                    description: def.description.clone(),
                });
            }
        } else {
            for column in &columns {
                let value = env
                    .get(&column.name)
                    .ok_or_else(|| CorpusError::Precondition(format!("feature `{}` missing from extraction", column.name)))?;
                let kind = value.type_name();
                if kind != column.kind {
                    return Err(CorpusError::Precondition(format!(
                        "feature `{}` changed kind from `{}` to `{kind}`",
                        column.name, column.kind
                    )));
                }
            }
        }

        let mut values: Vec<f64> = Vec::with_capacity(columns.len());
        for column in &columns {
            let value = env.get(&column.name).expect("checked above");
            let encoded = match value {
                FeatureValue::Bool(b) => {
                    if *b {
                        1.0
                    } else {
                        0.0
                    }
                }
                FeatureValue::Int(i) => *i as f64,
                FeatureValue::Float(f) => *f,
                FeatureValue::Set(s) => s.len() as f64,
            };
            values.push(encoded);
        }

        let legal_set: std::collections::BTreeSet<usize> = legal_positions.iter().copied().collect();
        let optimal_set: std::collections::BTreeSet<usize> = optimal_positions.iter().copied().collect();

        let mut qualifying: Vec<String> = Vec::new();
        let mut best: Option<(usize, (i32, usize, usize))> = None; // index in qualifying, sort key
        for (vocab_index, name) in classes.iter().enumerate() {
            let value = env.get(name).expect("class is a known column");
            let set_value = value.as_set().expect("class column is set-kind");
            let c_legal: std::collections::BTreeSet<usize> = legal_set.intersection(set_value).copied().collect();
            if c_legal.is_empty() || !c_legal.is_subset(&optimal_set) {
                continue;
            }
            let column = &columns[featurizer
                .vocabulary()
                .defs()
                .iter()
                .position(|d| d.name == *name)
                .expect("class exists in vocabulary")];
            let tier_rank = match column.tier {
                Tier::Invented => 2,
                Tier::Supplied => 1,
                Tier::Primitive => 0,
            };
            // Sort key: highest tier rank first, then smallest |C_legal|, then lowest vocab index.
            // Negating tier_rank lets the whole tuple be minimized.
            let sort_key = (-tier_rank, c_legal.len(), vocab_index);
            let candidate_index = qualifying.len();
            qualifying.push(name.clone());
            if best.as_ref().is_none_or(|(_, bkey)| sort_key < *bkey) {
                best = Some((candidate_index, sort_key));
            }
        }
        let label = best
            .map(|(idx, _)| qualifying[idx].clone())
            .unwrap_or_else(|| "none".to_string());

        let side_to_move = featurizer
            .side_to_move(&record.state)
            .ok_or_else(|| CorpusError::Precondition("side to move is not in the bundle turn order".to_string()))?;

        index_of.insert(canonical.clone(), rows.len());
        rows.push(DatasetRow {
            schema_version: SCHEMA_VERSION,
            canonical_state: canonical,
            side_to_move,
            value: record.value,
            occurrences: 1,
            legal: legal_positions,
            optimal: optimal_positions,
            qualifying,
            label,
            values,
        });
    }

    if rows.is_empty() {
        return Err(CorpusError::Precondition(
            "dataset requires at least one non-terminal annotation record".to_string(),
        ));
    }

    let mut label_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut value_counts: BTreeMap<String, usize> = BTreeMap::new();
    value_counts.insert("-1".to_string(), 0);
    value_counts.insert("0".to_string(), 0);
    value_counts.insert("1".to_string(), 0);
    let mut side_to_move_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unlabeled = 0usize;

    for row in &rows {
        *label_counts.entry(row.label.clone()).or_insert(0) += 1;
        *value_counts.entry(row.value.to_string()).or_insert(0) += 1;
        *side_to_move_counts.entry(row.side_to_move.to_string()).or_insert(0) += 1;
        if row.label == "none" {
            unlabeled += 1;
        }
    }

    let manifest = DatasetManifest {
        schema_version: SCHEMA_VERSION,
        game: bundle.name.clone(),
        run_id: source.run_id,
        annotations_mode: source.annotations_mode,
        tiers,
        columns,
        classes,
        rows: rows.len(),
        annotated: records.len(),
        nonterminal,
        collapsed: nonterminal - rows.len(),
        label_counts,
        value_counts,
        side_to_move_counts,
        unlabeled,
    };

    Ok(Dataset { manifest, rows })
}

/// Maps `[induction]` parameters to the featurizer construction they imply: tier-2 is withheld
/// only when induction is enabled and asks for it, and the extended tier-1 families are minted
/// only when induction is enabled and asks for them.
pub(crate) fn induction_spec(params: &InductionParams) -> FeaturizerSpec {
    FeaturizerSpec {
        include_supplied: !(params.enabled && params.withhold_tier2),
        extended: params.enabled && params.extended_tier1,
    }
}

/// Reads `annotate.json` and the context's annotations, builds a [`Featurizer`] per `spec`,
/// appends `derived` definitions to it, and calls [`build_dataset`]. The single
/// context-to-dataset path shared by [`DatasetAnalyzer`] and any later mining analyzer.
pub fn dataset_from_context<G: EngineGame + CorpusGame>(
    ctx: &AnalyzeContext<'_, G>,
    spec: FeaturizerSpec,
    derived: &[FeatureDef],
) -> Result<Dataset<G>, CorpusError>
where
    G::State: Clone + Eq + Hash,
{
    let records = ctx.annotations()?;
    let annotate_path = ctx.corpus_dir.join(ANNOTATE_FILE);
    if !annotate_path.is_file() {
        return Err(CorpusError::Precondition(format!(
            "{} not found; run `annotate --corpus {}` first",
            annotate_path.display(),
            ctx.corpus_dir.display()
        )));
    }
    let metadata: crate::discovery::annotate::AnnotateMetadata = read_json(&annotate_path)?;
    let mut featurizer = ctx.bundle.featurizer_with(spec)?;
    for def in derived {
        featurizer
            .push_derived(def.clone())
            .map_err(|e| CorpusError::Config(format!("derived feature `{}`: {e}", def.name)))?;
    }
    build_dataset(
        ctx.bundle,
        &featurizer,
        records,
        DatasetSource {
            run_id: Some(ctx.run_id.clone()),
            annotations_mode: mode_name(metadata.mode).to_string(),
        },
    )
}

/// The `dataset` analyzer: writes the canonicalized per-position feature dataset
/// (`dataset.json` manifest, `dataset.jsonl` rows).
pub struct DatasetAnalyzer;

impl<G: EngineGame + CorpusGame> Analyzer<G> for DatasetAnalyzer
where
    G::State: Clone + Eq + Hash,
{
    fn name(&self) -> &str {
        "dataset"
    }
    fn description(&self) -> &str {
        "canonicalized per-position feature dataset for rule mining"
    }
    fn requires_annotations(&self) -> bool {
        true
    }
    fn run(&self, ctx: &AnalyzeContext<'_, G>, options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
        let dataset = dataset_from_context(ctx, induction_spec(&options.induction), &[])?;
        write_jsonl(&ctx.corpus_dir.join(DATASET_FILE), dataset.rows.iter())?;
        AnalyzerOutput::new(DATASET_MANIFEST_FILE, &dataset.manifest, true)
    }
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError> {
        let manifest: DatasetManifest = serde_json::from_value(output.clone())
            .map_err(|e| CorpusError::Io(IoError::Invalid(format!("{DATASET_MANIFEST_FILE}: {e}"))))?;
        Ok(render_dataset(&manifest))
    }
}

/// Renders `manifest` into the deterministic Markdown produced by [`DatasetAnalyzer::render`]: a
/// `## Dataset` overview table followed by a `### Labels` section, the whole string ending in a
/// single `\n`.
fn render_dataset(manifest: &DatasetManifest) -> String {
    let mut blocks: Vec<String> = Vec::new();

    blocks.push("## Dataset".to_string());
    blocks.push(
        [
            "| metric | value |".to_string(),
            "| --- | --- |".to_string(),
            format!("| rows | {} |", manifest.rows),
            format!("| nonterminal | {} |", manifest.nonterminal),
            format!("| collapsed | {} |", manifest.collapsed),
            format!("| columns | {} |", manifest.columns.len()),
            format!("| classes | {} |", manifest.classes.len()),
            format!("| unlabeled | {} |", manifest.unlabeled),
        ]
        .join("\n"),
    );

    blocks.push("### Labels".to_string());
    let mut label_rows = vec!["| label | rows |".to_string(), "| --- | --- |".to_string()];
    for (label, count) in &manifest.label_counts {
        label_rows.push(format!("| {label} | {count} |"));
    }
    blocks.push(label_rows.join("\n"));

    format!("{}\n", blocks.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::analyze::{AnalyzeContext, load_corpus};
    use crate::discovery::annotate::annotate_corpus;
    use crate::discovery::config::GenerateConfig;
    use crate::discovery::corpus::{GenerateOptions, generate};
    use crate::discovery::evaluate::load_annotations;
    use crate::games::tictactoe::{Move, TicTacToe, game_bundle};
    use crate::io::read_jsonl;
    use std::path::PathBuf;
    use std::sync::OnceLock;

    const CONFIG: &str = r#"
schema_version = 1
game = "tictactoe"
seed = 7
games_per_cell = 2
pairings = [["random", "depth-1"], ["depth-1", "random"]]

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

    /// Generates and annotates the tiny [`CONFIG`] corpus once per process, returning its
    /// directory.
    fn shared_dir() -> &'static PathBuf {
        static DIR: OnceLock<PathBuf> = OnceLock::new();
        DIR.get_or_init(|| {
            let dir = std::env::temp_dir().join(format!("sd-dataset-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let bundle = game_bundle();
            let config = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();
            generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
            annotate_corpus(&bundle, &dir, &crate::discovery::annotate::AnnotateOptions::default()).unwrap();
            dir
        })
    }

    #[test]
    fn manifest_shape_and_row_invariants() {
        let dir = shared_dir();
        let bundle = game_bundle();
        let annotations = load_annotations(&bundle, dir).unwrap();
        let records = &annotations.records;
        let featurizer = bundle.featurizer(true).unwrap();

        let dataset = build_dataset(
            &bundle,
            &featurizer,
            records,
            DatasetSource {
                run_id: None,
                annotations_mode: "corpus".to_string(),
            },
        )
        .unwrap();
        let manifest = &dataset.manifest;

        assert_eq!(manifest.columns.len(), 70);
        assert_eq!(manifest.columns[0].name, "side_to_move");
        assert_eq!(manifest.columns[0].kind, "int");
        assert_eq!(manifest.classes.len(), 17);
        assert_eq!(manifest.classes[0], "free");
        assert!(manifest.classes.iter().any(|c| c == "ttt.winning_cells"));
        assert_eq!(manifest.tiers, vec!["primitive".to_string(), "supplied".to_string()]);
        assert_eq!(manifest.rows + manifest.collapsed, manifest.nonterminal);
        assert_eq!(manifest.annotated, records.len());
        assert_eq!(manifest.value_counts.len(), 3);
        assert_eq!(manifest.unlabeled, dataset.rows.iter().filter(|r| r.label == "none").count());

        for row in &dataset.rows {
            assert_eq!(row.values.len(), 70);
            for class in &row.qualifying {
                assert!(manifest.classes.contains(class));
            }
            assert!(row.label == "none" || manifest.classes.contains(&row.label));
            assert!(!row.legal.is_empty());
            let mut sorted = row.legal.clone();
            sorted.sort_unstable();
            assert_eq!(row.legal, sorted);
        }
    }

    #[test]
    fn build_twice_is_identical() {
        let dir = shared_dir();
        let bundle = game_bundle();
        let annotations = load_annotations(&bundle, dir).unwrap();
        let records = &annotations.records;
        let featurizer = bundle.featurizer(true).unwrap();

        let source = || DatasetSource {
            run_id: None,
            annotations_mode: "corpus".to_string(),
        };
        let first = build_dataset(&bundle, &featurizer, records, source()).unwrap();
        let second = build_dataset(&bundle, &featurizer, records, source()).unwrap();

        assert_eq!(first.manifest, second.manifest);
        assert_eq!(first.rows, second.rows);
    }

    #[test]
    fn analyzer_writes_jsonl_and_renders() {
        let dir = shared_dir();
        let bundle = game_bundle();
        let loaded = load_corpus::<TicTacToe>(&bundle, dir).unwrap();
        let ctx = AnalyzeContext::new(&bundle, dir, loaded.header.run_id.clone(), &loaded.games, &loaded.positions);

        let analyzer = DatasetAnalyzer;
        let output = analyzer.run(&ctx, &AnalyzeOptions::default()).unwrap();
        assert_eq!(output.file, "dataset.json");
        assert!(output.checks_pass);

        let manifest: DatasetManifest = serde_json::from_str(&output.json).unwrap();
        assert!(dir.join(DATASET_FILE).is_file());
        let rows: Vec<DatasetRow<<TicTacToe as GameDomain>::State>> = read_jsonl(&dir.join(DATASET_FILE)).unwrap();
        assert_eq!(rows.len(), manifest.rows);

        let value = output.value().unwrap();
        let analyzer: &dyn Analyzer<TicTacToe> = &analyzer;
        let rendered = analyzer.render(&value).unwrap();
        assert!(rendered.starts_with("## Dataset\n\n"));
        assert!(rendered.contains("### Labels"));
        assert!(!rendered.contains("|---"));
    }
}
