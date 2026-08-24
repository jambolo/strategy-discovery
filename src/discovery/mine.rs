//! The miner: decision-tree induction over the feature dataset, emitted as validated
//! self-contained rule-list strategies.
//!
//! [`Miner`] fits one decision tree per requested depth ([`crate::io::MineParams::depths`]) over
//! a train/holdout split of a [`crate::discovery::dataset::Dataset`], then converts each fitted
//! tree into an ordered [`crate::core::dsl::HeuristicStrategy`]: one rule per non-fallback leaf,
//! walked depth-first (left before right). A leaf's emitted class is **not** the fit-time Gini
//! label carried by [`crate::discovery::tree::Node::Leaf::class`] — that label is provisional and
//! is superseded here by a soundness-driven relabeling that picks the class whose action set most
//! often qualifies as an explanation of the optimal actions at the leaf's rows. [`MineAnalyzer`]
//! runs the miner over a corpus's dataset and renders the result to Markdown.

use std::sync::Arc;

use linfa::traits::Fit;
use linfa_trees::{DecisionTree, TreeNode};
use ndarray::{Array1, Array2};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

use crate::core::dsl::{ActionSelector, HeuristicStrategy, Rule};
use crate::core::features::{FeatureDef, FeatureExpr};
use crate::core::featurizer::Featurizer;
use crate::core::traits::{GameDomain, GeneratorError, StrategyGenerator};
use crate::discovery::analyze::{AnalyzeContext, AnalyzeOptions, Analyzer, AnalyzerOutput};
use crate::discovery::config::CorpusError;
use crate::discovery::dataset::{Dataset, dataset_from_context, induction_spec};
use crate::discovery::tree::{self, Node, Tree};
use crate::io::{
    CONCEPTS_FILE, ConceptReport, CorpusGame, HEURISTICS_FILE, IoError, MINE_ENGINE_CART, MINE_ENGINE_LINFA_TREES, MineParams,
    MinedHeuristic, MiningReport, OutcomeTally, RuleEvidence, SCHEMA_VERSION, read_json,
};
use crate::strategy::engine::EngineGame;

/// Builds a `(left, right)` pair of path predicates for a split on a column of kind `kind`
/// (`"bool"`, `"int"`, `"set"`, or `"float"`) named `name`, with stored threshold `threshold`,
/// for `engine` (`cart` or `linfa-trees`). See module docs for the per-kind shapes.
fn split_predicates(kind: &str, name: &str, threshold: f64, engine: &str) -> (FeatureExpr, FeatureExpr) {
    match kind {
        "bool" => (FeatureExpr::negate(FeatureExpr::named(name)), FeatureExpr::named(name)),
        "int" => {
            let v = threshold.floor() as i64;
            (
                FeatureExpr::compare(
                    crate::core::features::CmpOp::Le,
                    FeatureExpr::named(name),
                    FeatureExpr::int(v),
                ),
                FeatureExpr::compare(
                    crate::core::features::CmpOp::Gt,
                    FeatureExpr::named(name),
                    FeatureExpr::int(v),
                ),
            )
        }
        "set" => {
            let v = threshold.floor() as i64;
            (
                FeatureExpr::compare(
                    crate::core::features::CmpOp::Le,
                    FeatureExpr::count(FeatureExpr::named(name)),
                    FeatureExpr::int(v),
                ),
                FeatureExpr::compare(
                    crate::core::features::CmpOp::Gt,
                    FeatureExpr::count(FeatureExpr::named(name)),
                    FeatureExpr::int(v),
                ),
            )
        }
        "float" if engine == MINE_ENGINE_LINFA_TREES => (
            FeatureExpr::compare(
                crate::core::features::CmpOp::Lt,
                FeatureExpr::named(name),
                FeatureExpr::float(threshold),
            ),
            FeatureExpr::compare(
                crate::core::features::CmpOp::Ge,
                FeatureExpr::named(name),
                FeatureExpr::float(threshold),
            ),
        ),
        "float" => (
            FeatureExpr::compare(
                crate::core::features::CmpOp::Le,
                FeatureExpr::named(name),
                FeatureExpr::float(threshold),
            ),
            FeatureExpr::compare(
                crate::core::features::CmpOp::Gt,
                FeatureExpr::named(name),
                FeatureExpr::float(threshold),
            ),
        ),
        other => unreachable!("dataset column kinds are bool/int/float/set, got `{other}`"),
    }
}

/// Candidate name for `engine` at `depth`/`min_leaf`: `mined-d{depth}-l{min_leaf}` for `cart`,
/// `mined-linfa-d{depth}-l{min_leaf}` for `linfa-trees`.
fn candidate_name(engine: &str, depth: usize, min_leaf: usize) -> String {
    if engine == MINE_ENGINE_LINFA_TREES {
        format!("mined-linfa-d{depth}-l{min_leaf}")
    } else {
        format!("mined-d{depth}-l{min_leaf}")
    }
}

/// Splits `0..rows` into (train, holdout) index sets. `fraction == 0.0` trains on every row in
/// order with no RNG created; otherwise the indices are shuffled with a seeded RNG, the last
/// `floor(rows * fraction)` held out, and both sets sorted ascending. Shared with the
/// concept-induction module, which uses it for its own train/holdout split.
pub(crate) fn split_train_holdout(rows: usize, seed: u64, fraction: f64) -> (Vec<usize>, Vec<usize>) {
    if fraction == 0.0 {
        return ((0..rows).collect(), Vec::new());
    }
    let mut idx: Vec<usize> = (0..rows).collect();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    idx.shuffle(&mut rng);
    let n_holdout = (rows as f64 * fraction).floor() as usize;
    let split_at = rows - n_holdout;
    let mut holdout: Vec<usize> = idx.split_off(split_at);
    let mut train = idx;
    train.sort_unstable();
    holdout.sort_unstable();
    (train, holdout)
}

/// One leaf of a fitted tree, resolved to dataset row indices and its soundness-relabeled class.
struct LeafRecord {
    /// Conjunction of the path predicates from the root to this leaf.
    condition: FeatureExpr,
    /// Dataset row indices (not matrix positions) of the train rows reaching this leaf, ascending.
    dataset_rows: Vec<usize>,
    /// The relabeled class, or `"none"`.
    class: String,
    /// Address of the leaf `Node`, used to resolve holdout routing back to this record.
    addr: usize,
}

/// The class `c` (in vocabulary order) maximizing the count of `dataset_rows` whose `qualifying`
/// contains `c`; ties keep the lowest class index; a maximum count of `0` (including no rows)
/// yields `"none"`.
fn classify_leaf<G: GameDomain>(dataset: &Dataset<G>, dataset_rows: &[usize]) -> String {
    let classes = &dataset.manifest.classes;
    let mut best_class: Option<usize> = None;
    let mut best_count = 0usize;
    for (i, class) in classes.iter().enumerate() {
        let count = dataset_rows
            .iter()
            .filter(|&&r| dataset.rows[r].qualifying.contains(class))
            .count();
        if count > best_count {
            best_count = count;
            best_class = Some(i);
        }
    }
    match best_class {
        Some(i) => classes[i].clone(),
        None => "none".to_string(),
    }
}

/// Walks `node` depth-first (left before right), appending one [`LeafRecord`] per leaf to `out`.
fn collect_leaves<G: GameDomain>(
    node: &Node,
    dataset: &Dataset<G>,
    train_idx: &[usize],
    engine: &str,
    path: &mut Vec<FeatureExpr>,
    out: &mut Vec<LeafRecord>,
) {
    match node {
        Node::Leaf { rows, .. } => {
            let dataset_rows: Vec<usize> = rows.iter().map(|&pos| train_idx[pos]).collect();
            let class = classify_leaf(dataset, &dataset_rows);
            out.push(LeafRecord {
                condition: FeatureExpr::all(path.clone()),
                dataset_rows,
                class,
                addr: node as *const Node as usize,
            });
        }
        Node::Split {
            column,
            threshold,
            left,
            right,
            ..
        } => {
            let col = &dataset.manifest.columns[*column];
            let (left_pred, right_pred) = split_predicates(&col.kind, &col.name, *threshold, engine);
            path.push(left_pred);
            collect_leaves(left, dataset, train_idx, engine, path, out);
            path.pop();
            path.push(right_pred);
            collect_leaves(right, dataset, train_idx, engine, path, out);
            path.pop();
        }
    }
}

/// Routes `values` down `node` (`x[column] <= threshold` goes left), returning the leaf reached.
fn route<'a>(node: &'a Node, values: &[f64]) -> &'a Node {
    match node {
        Node::Leaf { .. } => node,
        Node::Split {
            column,
            threshold,
            left,
            right,
            ..
        } => {
            if values[*column] <= *threshold {
                route(left, values)
            } else {
                route(right, values)
            }
        }
    }
}

/// Converts a fitted linfa `TreeNode` into our [`Node`] shape, discarding linfa's own leaf
/// predictions (see [`fit_linfa_tree`] for why) and leaving every `rows` empty for the caller to
/// fill by routing.
fn convert_linfa_node(node: &TreeNode<f64, usize>) -> Node {
    if node.is_leaf() {
        return Node::Leaf {
            class: node.prediction().unwrap_or(0),
            rows: Vec::new(),
        };
    }
    let (column, threshold, _impurity) = node.split();
    let children = node.children();
    match (children[0], children[1]) {
        (Some(left), Some(right)) => Node::Split {
            column,
            threshold,
            left: Box::new(convert_linfa_node(left)),
            right: Box::new(convert_linfa_node(right)),
            rows: Vec::new(),
        },
        // Defensive: linfa-trees always emits both children for an internal node in practice,
        // but a split missing a child carries no routable structure, so treat it as a leaf.
        _ => Node::Leaf {
            class: 0,
            rows: Vec::new(),
        },
    }
}

/// Fills every node's `rows` (ascending, matching the `cart` shape) by routing each row index of
/// `matrix` down `node` with `x[column] <= threshold` going left.
fn route_rows(node: &mut Node, matrix: &[Vec<f64>], rows: &[usize]) {
    match node {
        Node::Leaf { rows: out, .. } => *out = rows.to_vec(),
        Node::Split {
            column,
            threshold,
            left,
            right,
            rows: out,
        } => {
            *out = rows.to_vec();
            let mut left_rows = Vec::new();
            let mut right_rows = Vec::new();
            for &r in rows {
                if matrix[r][*column] <= *threshold {
                    left_rows.push(r);
                } else {
                    right_rows.push(r);
                }
            }
            route_rows(left, matrix, &left_rows);
            route_rows(right, matrix, &right_rows);
        }
    }
}

/// Fits a `linfa_trees::DecisionTree` over `matrix`/`labels` and converts it into our [`Tree`]
/// shape, with every node's `rows` populated by routing.
///
/// linfa fits with `x <= split_value` where `split_value` is the MIDPOINT of consecutive
/// distinct sorted training values, but its own `predict` routes with `x < split_value` —
/// identical on the training data (no row lands exactly on a midpoint of *distinct* values).
/// Leaf class assignment is resolved through linfa's internal `HashMap` iteration and is
/// therefore NOT byte-stable across processes; that is exactly why the converted tree discards
/// linfa's leaf predictions once relabeling runs (`classify_leaf` supersedes `Node::Leaf::class`
/// here), why routing above must reuse the fit-time `<=` predicate over the midpoints rather than
/// linfa's `<` predict-time routing, and why `cart` (not this engine) is the crate's
/// determinism-evidence default.
fn fit_linfa_tree(
    matrix: &[Vec<f64>],
    labels: &[usize],
    column_names: &[String],
    depth: usize,
    min_leaf: usize,
) -> Result<Tree, GeneratorError> {
    if matrix.is_empty() {
        return Ok(Tree {
            root: Node::Leaf {
                class: 0,
                rows: Vec::new(),
            },
        });
    }
    let n_features = matrix[0].len();
    let flat: Vec<f64> = matrix.iter().flatten().copied().collect();
    let records = Array2::from_shape_vec((matrix.len(), n_features), flat)
        .map_err(|e| GeneratorError::Failed(format!("linfa-trees fit failed: {e}")))?;
    let targets = Array1::from_vec(labels.to_vec());
    let dataset = linfa::Dataset::new(records, targets).with_feature_names(column_names.to_vec());

    let model: DecisionTree<f64, usize> = DecisionTree::params()
        .max_depth(if depth == 0 { None } else { Some(depth) })
        .min_weight_leaf(min_leaf as f32)
        .min_weight_split((2 * min_leaf) as f32)
        .fit(&dataset)
        .map_err(|e| GeneratorError::Failed(format!("linfa-trees fit failed: {e}")))?;

    let mut root = convert_linfa_node(model.root_node());
    let rows: Vec<usize> = (0..matrix.len()).collect();
    route_rows(&mut root, matrix, &rows);
    Ok(Tree { root })
}

/// Fits and converts one candidate: one tree at `depth`/`min_leaf` over `engine`, relabeled,
/// converted to a decision list, and evaluated over `train_idx`/`holdout_idx`. Shared with the
/// concept-induction module, which uses it as its downstream promotion probe.
pub(crate) fn mine_one<G: GameDomain>(
    dataset: &Dataset<G>,
    train_idx: &[usize],
    holdout_idx: &[usize],
    engine: &str,
    depth: usize,
    min_leaf: usize,
    featurizer: &Featurizer<G>,
) -> Result<MinedHeuristic, GeneratorError> {
    let classes = &dataset.manifest.classes;
    let n_classes = classes.len() + 1;

    let label_index = |i: usize| -> usize {
        let label = &dataset.rows[i].label;
        dataset.class_index(label).unwrap_or(classes.len())
    };

    let matrix: Vec<Vec<f64>> = train_idx.iter().map(|&i| dataset.rows[i].values.clone()).collect();
    let labels: Vec<usize> = train_idx.iter().map(|&i| label_index(i)).collect();

    let tree: Tree = match engine {
        MINE_ENGINE_CART => tree::fit(&matrix, &labels, n_classes, depth, min_leaf),
        MINE_ENGINE_LINFA_TREES => {
            let column_names: Vec<String> = dataset.manifest.columns.iter().map(|c| c.name.clone()).collect();
            fit_linfa_tree(&matrix, &labels, &column_names, depth, min_leaf)?
        }
        other => return Err(GeneratorError::Failed(format!("unknown mine engine `{other}`"))),
    };

    let mut leaves: Vec<LeafRecord> = Vec::new();
    let mut path: Vec<FeatureExpr> = Vec::new();
    collect_leaves(&tree.root, dataset, train_idx, engine, &mut path, &mut leaves);

    let total_leaves = leaves.len();
    let name = candidate_name(engine, depth, min_leaf);

    let mut rules: Vec<Rule> = Vec::new();
    let mut evidence: Vec<RuleEvidence> = Vec::new();
    let mut fallback_rows = 0usize;
    let mut train_correct = 0usize;
    let mut train_sound = 0usize;

    for (i, leaf) in leaves.iter().enumerate() {
        let k = i + 1;
        let support = leaf.dataset_rows.len();
        let label_matches = leaf
            .dataset_rows
            .iter()
            .filter(|&&r| dataset.rows[r].label == leaf.class)
            .count();
        train_correct += label_matches;

        if leaf.class == "none" {
            fallback_rows += support;
            continue;
        }

        let sound = leaf
            .dataset_rows
            .iter()
            .filter(|&&r| dataset.rows[r].qualifying.contains(&leaf.class))
            .count();
        train_sound += sound;
        let soundness = if support == 0 { 0.0 } else { sound as f64 / support as f64 };
        let occurrences: usize = leaf.dataset_rows.iter().map(|&r| dataset.rows[r].occurrences).sum();
        let mut outcomes = OutcomeTally::default();
        for &r in &leaf.dataset_rows {
            match dataset.rows[r].value {
                1 => outcomes.win += 1,
                0 => outcomes.draw += 1,
                -1 => outcomes.loss += 1,
                _ => {}
            }
        }

        let priority = (total_leaves - k) as i32;
        let rule_name = format!("rule{k}");
        rules.push(Rule::new(
            rule_name.clone(),
            priority,
            leaf.condition.clone(),
            ActionSelector::TargetIn {
                expr: FeatureExpr::named(leaf.class.clone()),
            },
        ));
        evidence.push(RuleEvidence {
            rule: rule_name,
            class: leaf.class.clone(),
            support,
            sound,
            soundness,
            label_matches,
            occurrences,
            outcomes,
        });
    }

    let train_rows = train_idx.len();
    let train_accuracy = if train_rows == 0 {
        0.0
    } else {
        train_correct as f64 / train_rows as f64
    };
    let train_soundness = if train_rows == 0 {
        0.0
    } else {
        train_sound as f64 / train_rows as f64
    };

    let (holdout_accuracy, holdout_soundness) = if holdout_idx.is_empty() {
        (None, None)
    } else {
        let leaf_map: Vec<(usize, &str)> = leaves.iter().map(|l| (l.addr, l.class.as_str())).collect();
        let mut correct = 0usize;
        let mut sound = 0usize;
        for &r in holdout_idx {
            let row = &dataset.rows[r];
            let reached = route(&tree.root, &row.values);
            let addr = reached as *const Node as usize;
            let class = leaf_map
                .iter()
                .find(|(a, _)| *a == addr)
                .map(|(_, c)| *c)
                .expect("every leaf reachable by routing was recorded during collection");
            if row.label == class {
                correct += 1;
            }
            if class != "none" && row.qualifying.iter().any(|q| q == class) {
                sound += 1;
            }
        }
        let n = holdout_idx.len() as f64;
        (Some(correct as f64 / n), Some(sound as f64 / n))
    };

    let mut heuristic = HeuristicStrategy::new(name.clone());
    for rule in &rules {
        heuristic.push_rule(rule.clone());
    }
    for def in featurizer.vocabulary().closure(&heuristic.references()) {
        heuristic
            .define(def)
            .map_err(|e| GeneratorError::Failed(format!("mined candidate `{name}` failed validation: {e}")))?;
    }
    heuristic
        .validate()
        .map_err(|e| GeneratorError::Failed(format!("mined candidate `{name}` failed validation: {e}")))?;

    Ok(MinedHeuristic {
        name,
        engine: engine.to_string(),
        max_depth: depth,
        min_leaf,
        heuristic,
        rules: rules.len(),
        leaves: total_leaves,
        depth: tree.depth(),
        train_rows,
        holdout_rows: holdout_idx.len(),
        train_accuracy,
        train_soundness,
        holdout_accuracy,
        holdout_soundness,
        fallback_rows,
        evidence,
    })
}

/// Induces one or more mined heuristic candidates from a [`Dataset`].
pub struct Miner<G: GameDomain> {
    /// Featurizer supplying the vocabulary closure for self-contained candidates.
    pub featurizer: Arc<Featurizer<G>>,
    /// Mining parameters (engine, depths, min_leaf, seed, holdout fraction).
    pub params: MineParams,
}

impl<G: GameDomain> Miner<G> {
    /// Builds a miner from `featurizer` and `params`.
    pub fn new(featurizer: Arc<Featurizer<G>>, params: MineParams) -> Self {
        Miner { featurizer, params }
    }
}

impl<G: GameDomain> StrategyGenerator<G> for Miner<G> {
    type Input = Dataset<G>;
    type Candidate = MinedHeuristic;

    fn generate(&mut self, input: &Dataset<G>, seed: u64) -> Result<Vec<MinedHeuristic>, GeneratorError> {
        self.params.validate().map_err(|e| GeneratorError::Failed(e.to_string()))?;
        if input.rows.is_empty() {
            return Err(GeneratorError::Failed("mining requires a non-empty dataset".to_string()));
        }

        let (train_idx, holdout_idx) = split_train_holdout(input.rows.len(), seed, self.params.holdout_fraction);

        let mut candidates = Vec::with_capacity(self.params.depths.len());
        for &depth in &self.params.depths {
            let candidate = mine_one(
                input,
                &train_idx,
                &holdout_idx,
                &self.params.engine,
                depth,
                self.params.min_leaf,
                &self.featurizer,
            )?;
            candidates.push(candidate);
        }
        Ok(candidates)
    }
}

/// The `mine` analyzer: induces ordered heuristic rule lists from the feature dataset, writing
/// `heuristics.json`. When `[induction]` is enabled the analyzer reads promoted concepts from
/// `concepts.json` and mines over the concept-augmented dataset and vocabulary.
pub struct MineAnalyzer;

impl<G: EngineGame + CorpusGame> Analyzer<G> for MineAnalyzer
where
    G::State: Clone + Eq + std::hash::Hash,
{
    fn name(&self) -> &str {
        "mine"
    }
    fn description(&self) -> &str {
        "induces ordered heuristic rule lists from the feature dataset"
    }
    fn requires_annotations(&self) -> bool {
        true
    }
    fn run(&self, ctx: &AnalyzeContext<'_, G>, options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
        options.mine.validate()?;
        options.induction.validate()?;
        let spec = induction_spec(&options.induction);
        let derived: Vec<FeatureDef> = if options.induction.enabled {
            let path = ctx.corpus_dir.join(CONCEPTS_FILE);
            if !path.is_file() {
                return Err(CorpusError::Precondition(
                    "concepts.json not found; run the `concepts` analyzer before `mine`".to_string(),
                ));
            }
            let report: ConceptReport = read_json(&path)?;
            report.promoted.iter().map(|p| p.def.clone()).collect()
        } else {
            Vec::new()
        };
        let dataset = dataset_from_context(ctx, spec, &derived)?;
        let mut featurizer = ctx.bundle.featurizer_with(spec)?;
        for def in &derived {
            featurizer
                .push_derived(def.clone())
                .map_err(|e| CorpusError::Config(format!("pushing promoted concept `{}`: {e}", def.name)))?;
        }
        let mut miner = Miner::new(Arc::new(featurizer), options.mine.clone());
        let candidates = miner
            .generate(&dataset, options.mine.seed)
            .map_err(|GeneratorError::Failed(m)| CorpusError::Config(format!("mine analyzer: {m}")))?;

        let report = MiningReport {
            schema_version: SCHEMA_VERSION,
            game: ctx.bundle.name.clone(),
            run_id: dataset.manifest.run_id.clone(),
            params: options.mine.clone(),
            dataset: dataset.manifest.clone(),
            candidates,
        };
        AnalyzerOutput::new(HEURISTICS_FILE, &report, true)
    }
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError> {
        let report: MiningReport = serde_json::from_value(output.clone())
            .map_err(|e| CorpusError::Io(IoError::Invalid(format!("{HEURISTICS_FILE}: {e}"))))?;
        Ok(render_mine(&report))
    }
}

/// Renders `report` into the deterministic Markdown produced by [`MineAnalyzer::render`]: a
/// `## Mine` overview table, then per candidate a `### {name}` metric table, `#### Rules`,
/// an `otherwise:` line, and `#### Feature definitions`, the whole string ending in a single
/// `\n`.
fn render_mine(report: &MiningReport) -> String {
    let mut blocks: Vec<String> = Vec::new();

    blocks.push("## Mine".to_string());
    blocks.push(
        [
            "| metric | value |".to_string(),
            "| --- | --- |".to_string(),
            format!("| engine | {} |", report.params.engine),
            format!("| rows | {} |", report.dataset.rows),
            format!("| classes | {} |", report.dataset.classes.len()),
            format!("| candidates | {} |", report.candidates.len()),
        ]
        .join("\n"),
    );

    for candidate in &report.candidates {
        blocks.push(format!("### {}", candidate.name));

        let holdout_line = match candidate.holdout_accuracy {
            Some(v) => format!("| holdout_accuracy | {v:.3} |"),
            None => "| holdout_accuracy | none |".to_string(),
        };
        blocks.push(
            [
                "| metric | value |".to_string(),
                "| --- | --- |".to_string(),
                format!("| max_depth | {} |", candidate.max_depth),
                format!("| min_leaf | {} |", candidate.min_leaf),
                format!("| rules | {} |", candidate.rules),
                format!("| leaves | {} |", candidate.leaves),
                format!("| depth | {} |", candidate.depth),
                format!("| train_accuracy | {:.3} |", candidate.train_accuracy),
                format!("| train_soundness | {:.3} |", candidate.train_soundness),
                holdout_line,
                format!("| fallback_rows | {} |", candidate.fallback_rows),
            ]
            .join("\n"),
        );

        blocks.push("#### Rules".to_string());
        let mut rule_lines: Vec<String> = Vec::new();
        for (k, (rule, e)) in candidate
            .heuristic
            .ordered_rules()
            .into_iter()
            .zip(candidate.evidence.iter())
            .enumerate()
        {
            rule_lines.push(format!(
                "{}. if {} then {} — support {}, sound {} ({:.3}), outcomes W/D/L {}/{}/{}",
                k + 1,
                rule.condition,
                rule.action,
                e.support,
                e.sound,
                e.soundness,
                e.outcomes.win,
                e.outcomes.draw,
                e.outcomes.loss
            ));
        }
        blocks.push(rule_lines.join("\n"));

        blocks.push(format!("otherwise: {}", candidate.heuristic.fallback));

        blocks.push("#### Feature definitions".to_string());
        let def_lines: Vec<String> = candidate
            .heuristic
            .definitions
            .defs()
            .iter()
            .map(|def| format!("- `{def}`"))
            .collect();
        blocks.push(def_lines.join("\n"));
    }

    format!("{}\n", blocks.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::features::{CmpOp, Tier};
    use crate::discovery::analyze::{AnalyzeContext, GameRec, PosRec};
    use crate::games::tictactoe::{Board, TicTacToe, TicTacToeRules, game_bundle};
    use crate::io::{DatasetColumn, DatasetManifest, DatasetRow, InductionParams};
    use std::collections::{BTreeMap, BTreeSet};

    fn toy_dataset() -> Dataset<TicTacToe> {
        let columns = vec![
            DatasetColumn {
                name: "ttt.winning_cells".to_string(),
                tier: Tier::Supplied,
                kind: "set".to_string(),
                description: "winning cells".to_string(),
            },
            DatasetColumn {
                name: "ttt.blocking_cells".to_string(),
                tier: Tier::Supplied,
                kind: "set".to_string(),
                description: "blocking cells".to_string(),
            },
            DatasetColumn {
                name: "ttt.threats.mine".to_string(),
                tier: Tier::Supplied,
                kind: "int".to_string(),
                description: "threats for the mover".to_string(),
            },
        ];
        let classes = vec!["ttt.winning_cells".to_string(), "ttt.blocking_cells".to_string()];

        let initial = <TicTacToeRules as crate::core::traits::GameRules<TicTacToe>>::initial_state(&TicTacToeRules);

        let mut rows: Vec<DatasetRow<Board>> = Vec::new();
        for x in [0.0f64, 0.0, 1.0, 1.0] {
            let (label, qualifying) = if x <= 0.0 {
                ("ttt.winning_cells".to_string(), vec!["ttt.winning_cells".to_string()])
            } else {
                ("ttt.blocking_cells".to_string(), vec!["ttt.blocking_cells".to_string()])
            };
            rows.push(DatasetRow {
                schema_version: SCHEMA_VERSION,
                canonical_state: initial,
                side_to_move: 0,
                value: 1,
                occurrences: 1,
                legal: vec![0],
                optimal: vec![0],
                qualifying,
                label,
                values: vec![1.0, 1.0, x],
            });
        }

        let manifest = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
            tiers: vec!["supplied".to_string()],
            columns,
            classes,
            rows: rows.len(),
            annotated: rows.len(),
            nonterminal: rows.len(),
            collapsed: 0,
            label_counts: BTreeMap::new(),
            value_counts: BTreeMap::new(),
            side_to_move_counts: BTreeMap::new(),
            unlabeled: 0,
        };

        Dataset { manifest, rows }
    }

    fn toy_featurizer() -> Featurizer<TicTacToe> {
        game_bundle().featurizer(true).unwrap()
    }

    #[test]
    fn split_predicates_shapes() {
        let name = "f";

        assert_eq!(
            split_predicates("int", name, 1.0, MINE_ENGINE_CART),
            (
                FeatureExpr::compare(CmpOp::Le, FeatureExpr::named(name), FeatureExpr::int(1)),
                FeatureExpr::compare(CmpOp::Gt, FeatureExpr::named(name), FeatureExpr::int(1)),
            )
        );
        assert_eq!(
            split_predicates("int", name, 1.5, MINE_ENGINE_LINFA_TREES),
            (
                FeatureExpr::compare(CmpOp::Le, FeatureExpr::named(name), FeatureExpr::int(1)),
                FeatureExpr::compare(CmpOp::Gt, FeatureExpr::named(name), FeatureExpr::int(1)),
            )
        );
        assert_eq!(
            split_predicates("set", name, 2.0, MINE_ENGINE_CART),
            (
                FeatureExpr::compare(CmpOp::Le, FeatureExpr::count(FeatureExpr::named(name)), FeatureExpr::int(2)),
                FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named(name)), FeatureExpr::int(2)),
            )
        );
        assert_eq!(
            split_predicates("bool", name, 0.5, MINE_ENGINE_CART),
            (FeatureExpr::negate(FeatureExpr::named(name)), FeatureExpr::named(name))
        );
        assert_eq!(
            split_predicates("bool", name, 0.5, MINE_ENGINE_LINFA_TREES),
            (FeatureExpr::negate(FeatureExpr::named(name)), FeatureExpr::named(name))
        );
        assert_eq!(
            split_predicates("float", name, 0.5, MINE_ENGINE_CART),
            (
                FeatureExpr::compare(CmpOp::Le, FeatureExpr::named(name), FeatureExpr::float(0.5)),
                FeatureExpr::compare(CmpOp::Gt, FeatureExpr::named(name), FeatureExpr::float(0.5)),
            )
        );
        assert_eq!(
            split_predicates("float", name, 0.5, MINE_ENGINE_LINFA_TREES),
            (
                FeatureExpr::compare(CmpOp::Lt, FeatureExpr::named(name), FeatureExpr::float(0.5)),
                FeatureExpr::compare(CmpOp::Ge, FeatureExpr::named(name), FeatureExpr::float(0.5)),
            )
        );
    }

    #[test]
    fn candidate_names() {
        assert_eq!(candidate_name(MINE_ENGINE_CART, 4, 1), "mined-d4-l1");
        assert_eq!(candidate_name(MINE_ENGINE_LINFA_TREES, 0, 2), "mined-linfa-d0-l2");
    }

    #[test]
    fn cart_generate_emits_validated_rules_and_evidence() {
        let dataset = toy_dataset();
        let params = MineParams {
            engine: MINE_ENGINE_CART.to_string(),
            depths: vec![8],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut miner = Miner::new(Arc::new(toy_featurizer()), params);
        let candidates = miner.generate(&dataset, 0).unwrap();

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.name, "mined-d8-l1");
        assert_eq!(candidate.rules, 2);
        assert_eq!(candidate.leaves, 2);
        assert_eq!(candidate.depth, 1);
        assert_eq!(candidate.train_rows, 4);
        assert_eq!(candidate.holdout_rows, 0);
        assert_eq!(candidate.holdout_accuracy, None);
        assert_eq!(candidate.train_accuracy, 1.0);
        assert_eq!(candidate.train_soundness, 1.0);
        assert_eq!(candidate.fallback_rows, 0);

        let ordered = candidate.heuristic.ordered_rules();
        assert_eq!(ordered[0].name, "rule1");
        assert_eq!(ordered[0].priority, 1);
        assert_eq!(ordered[1].name, "rule2");
        assert_eq!(ordered[1].priority, 0);

        assert_eq!(
            ordered[0].condition,
            FeatureExpr::all(vec![FeatureExpr::compare(
                CmpOp::Le,
                FeatureExpr::named("ttt.threats.mine"),
                FeatureExpr::int(0)
            )])
        );
        assert_eq!(
            ordered[0].action,
            ActionSelector::TargetIn {
                expr: FeatureExpr::named("ttt.winning_cells")
            }
        );

        let evidence = &candidate.evidence[0];
        assert_eq!(evidence.support, 2);
        assert_eq!(evidence.sound, 2);
        assert_eq!(evidence.soundness, 1.0);
        assert_eq!(evidence.occurrences, 2);
        assert_eq!(
            evidence.outcomes,
            OutcomeTally {
                win: 2,
                draw: 0,
                loss: 0
            }
        );

        assert!(candidate.heuristic.validate().is_ok());
        let names: BTreeSet<String> = candidate
            .heuristic
            .definitions
            .defs()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        assert!(names.contains("ttt.winning_cells"));
        assert!(names.contains("ttt.blocking_cells"));
        assert!(names.contains("ttt.threats.mine"));
    }

    #[test]
    fn none_leaves_fall_back() {
        let mut dataset = toy_dataset();
        for row in dataset.rows.iter_mut() {
            if row.values[2] >= 1.0 {
                row.qualifying = Vec::new();
                row.label = "none".to_string();
            }
        }

        let params = MineParams {
            engine: MINE_ENGINE_CART.to_string(),
            depths: vec![8],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut miner = Miner::new(Arc::new(toy_featurizer()), params);
        let candidates = miner.generate(&dataset, 0).unwrap();
        let candidate = &candidates[0];

        assert_eq!(candidate.rules, 1);
        assert_eq!(candidate.fallback_rows, 2);
        assert_eq!(candidate.train_soundness, 0.5);
    }

    fn toy_dataset_tie_free() -> Dataset<TicTacToe> {
        let mut dataset = toy_dataset();
        let initial = dataset.rows[0].canonical_state;
        dataset.rows.clear();
        for x in [0.0f64, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0] {
            let (label, qualifying) = if x <= 1.0 {
                ("ttt.winning_cells".to_string(), vec!["ttt.winning_cells".to_string()])
            } else {
                ("ttt.blocking_cells".to_string(), vec!["ttt.blocking_cells".to_string()])
            };
            dataset.rows.push(DatasetRow {
                schema_version: SCHEMA_VERSION,
                canonical_state: initial,
                side_to_move: 0,
                value: 1,
                occurrences: 1,
                legal: vec![0],
                optimal: vec![0],
                qualifying,
                label,
                values: vec![1.0, 1.0, x],
            });
        }
        dataset.manifest.rows = dataset.rows.len();
        dataset.manifest.annotated = dataset.rows.len();
        dataset.manifest.nonterminal = dataset.rows.len();
        dataset
    }

    #[test]
    fn linfa_and_cart_agree_on_a_tie_free_toy() {
        let dataset = toy_dataset_tie_free();

        let cart_params = MineParams {
            engine: MINE_ENGINE_CART.to_string(),
            depths: vec![2],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut cart_miner = Miner::new(Arc::new(toy_featurizer()), cart_params);
        let cart_candidates = cart_miner.generate(&dataset, 0).unwrap();
        let cart_candidate = &cart_candidates[0];

        let linfa_params = MineParams {
            engine: MINE_ENGINE_LINFA_TREES.to_string(),
            depths: vec![2],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut linfa_miner = Miner::new(Arc::new(toy_featurizer()), linfa_params);
        let linfa_candidates = linfa_miner.generate(&dataset, 0).unwrap();
        let linfa_candidate = &linfa_candidates[0];

        assert_eq!(cart_candidate.heuristic.rules, linfa_candidate.heuristic.rules);
        assert_eq!(cart_candidate.rules, 2);
        assert_eq!(linfa_candidate.rules, 2);
        assert_eq!(
            cart_candidate
                .heuristic
                .definitions
                .defs()
                .iter()
                .map(|d| d.name.clone())
                .collect::<Vec<_>>(),
            linfa_candidate
                .heuristic
                .definitions
                .defs()
                .iter()
                .map(|d| d.name.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(cart_candidate.name, "mined-d2-l1");
        assert_eq!(linfa_candidate.name, "mined-linfa-d2-l1");
        assert_eq!(cart_candidate.evidence, linfa_candidate.evidence);
    }

    #[test]
    fn linfa_engine_reports_metrics() {
        let dataset = toy_dataset_tie_free();
        let params = MineParams {
            engine: MINE_ENGINE_LINFA_TREES.to_string(),
            depths: vec![2],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut miner = Miner::new(Arc::new(toy_featurizer()), params);
        let candidates = miner.generate(&dataset, 0).unwrap();
        let candidate = &candidates[0];

        assert_eq!(candidate.train_rows, 8);
        assert_eq!(candidate.train_accuracy, 1.0);
        assert_eq!(candidate.train_soundness, 1.0);
        assert_eq!(candidate.fallback_rows, 0);
        assert_eq!(candidate.depth, 1);
        assert_eq!(candidate.leaves, 2);
    }

    #[test]
    fn render_shape() {
        let dataset = toy_dataset();
        let params = MineParams {
            engine: MINE_ENGINE_CART.to_string(),
            depths: vec![8],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut miner = Miner::new(Arc::new(toy_featurizer()), params.clone());
        let candidates = miner.generate(&dataset, 0).unwrap();

        let report = MiningReport {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            params,
            dataset: dataset.manifest.clone(),
            candidates,
        };

        let rendered = render_mine(&report);

        assert!(rendered.starts_with("## Mine\n\n"));
        assert!(rendered.contains("### mined-d8-l1"));
        assert!(rendered.contains("#### Rules"));
        assert!(
            rendered
                .lines()
                .any(|l| l.starts_with("1. if ") && l.contains(" then play a position in ") && l.contains(" — support "))
        );
        assert!(rendered.lines().any(|l| l == "otherwise: any legal action"));
        assert!(rendered.contains("#### Feature definitions"));
        assert!(rendered.lines().any(|l| l.starts_with("- `")));
        assert!(!rendered.contains("|---"));

        let lines: Vec<&str> = rendered.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with('#') {
                assert!(
                    i + 1 < lines.len() && lines[i + 1].is_empty(),
                    "heading `{line}` not followed by a blank line"
                );
            }
        }
    }

    #[test]
    fn mine_requires_concepts_json_when_induction_enabled() {
        let bundle = game_bundle();
        let dir = tempfile::tempdir().unwrap();
        let games: Vec<GameRec<TicTacToe>> = Vec::new();
        let positions: Vec<PosRec<TicTacToe>> = Vec::new();
        let ctx = AnalyzeContext::new(&bundle, dir.path(), "run".to_string(), &games, &positions);
        let options = AnalyzeOptions {
            induction: InductionParams {
                enabled: true,
                ..Default::default()
            },
            ..AnalyzeOptions::default()
        };

        let result = <MineAnalyzer as Analyzer<TicTacToe>>::run(&MineAnalyzer, &ctx, &options);

        match result {
            Err(e) => assert!(
                e.to_string()
                    .contains("concepts.json not found; run the `concepts` analyzer before `mine`")
            ),
            Ok(_) => panic!("expected Err"),
        }
    }
}
