//! Fits and evaluates a depth-8 decision tree with `linfa-trees` and with `smartcore`: fit time
//! (median of 3), train/held-out accuracy, determinism (identical inputs, twice), and structure
//! traversability.

use crate::dataset::Built;
use linfa::traits::{Fit, Predict};
use linfa_trees::{DecisionTree, TreeNode};
use ndarray::{Array1, Array2};
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::tree::decision_tree_classifier::{DecisionTreeClassifier, DecisionTreeClassifierParameters};
use std::time::Instant;

/// Fit/evaluation outcome for `linfa-trees`.
pub struct LinfaResult {
    pub model: DecisionTree<f64, usize>,
    pub fit_ms: f64,
    pub train_accuracy: f64,
    pub holdout_accuracy: f64,
    pub deterministic: bool,
    pub traversable: &'static str,
    pub api_used: String,
    pub node_count: usize,
    pub leaf_count: usize,
}

/// Fit/evaluation outcome for `smartcore`.
pub struct SmartcoreResult {
    pub fit_ms: f64,
    pub train_accuracy: f64,
    pub holdout_accuracy: f64,
    pub deterministic: bool,
    pub traversable: &'static str,
    pub api_used: String,
    pub node_count: usize,
    pub leaf_count: usize,
}

fn to_array2(rows: &[Vec<f64>], n_features: usize) -> Array2<f64> {
    let flat: Vec<f64> = rows.iter().flatten().copied().collect();
    Array2::from_shape_vec((rows.len(), n_features), flat).expect("rectangular row data")
}

fn accuracy_usize(pred: &Array1<usize>, actual: &[usize]) -> f64 {
    let correct = pred.iter().zip(actual.iter()).filter(|(a, b)| a == b).count();
    correct as f64 / actual.len() as f64
}

fn accuracy_u32(pred: &[u32], actual: &[u32]) -> f64 {
    let correct = pred.iter().zip(actual.iter()).filter(|(a, b)| a == b).count();
    correct as f64 / actual.len() as f64
}

/// Canonical string dump of a fitted linfa tree's structure, used only to compare two
/// independently fitted trees for determinism.
fn dump_tree(node: &TreeNode<f64, usize>) -> String {
    if node.is_leaf() {
        format!("L({})", node.prediction().expect("leaf has a prediction"))
    } else {
        let (feature_idx, split_value, _impurity) = node.split();
        let children = node.children();
        let left = children[0].as_ref().map(|b| dump_tree(b)).unwrap_or_else(|| "-".to_string());
        let right = children[1].as_ref().map(|b| dump_tree(b)).unwrap_or_else(|| "-".to_string());
        format!("N({feature_idx},{split_value:.6},{left},{right})")
    }
}

/// Fits and evaluates `linfa_trees::DecisionTree` at `max_depth = 8`.
pub fn fit_linfa(built: &Built) -> LinfaResult {
    let train_records = to_array2(&built.train_rows, built.n_features);
    let holdout_records = to_array2(&built.holdout_rows, built.n_features);
    let train_targets: Vec<usize> = built.train_labels.iter().map(|&l| l as usize).collect();
    let holdout_targets: Vec<usize> = built.holdout_labels.iter().map(|&l| l as usize).collect();

    let dataset = linfa::Dataset::new(train_records.clone(), Array1::from_vec(train_targets.clone()))
        .with_feature_names(built.column_names.clone());

    let mut times = Vec::with_capacity(3);
    let mut models = Vec::with_capacity(3);
    for _ in 0..3 {
        let start = Instant::now();
        let model = DecisionTree::params()
            .max_depth(Some(8))
            .fit(&dataset)
            .expect("linfa fit succeeds");
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        models.push(model);
    }
    times.sort_by(|a, b| a.partial_cmp(b).expect("fit times are finite"));
    let fit_ms = times[1];

    let deterministic = dump_tree(models[0].root_node()) == dump_tree(models[1].root_node());

    let model = models.pop().expect("3 fits produced 3 models");
    let train_pred = model.predict(&train_records);
    let holdout_pred = model.predict(&holdout_records);

    LinfaResult {
        train_accuracy: accuracy_usize(&train_pred, &train_targets),
        holdout_accuracy: accuracy_usize(&holdout_pred, &holdout_targets),
        node_count: model.iter_nodes().count(),
        leaf_count: model.num_leaves(),
        fit_ms,
        deterministic,
        traversable: "YES",
        api_used: "linfa_trees::DecisionTree::root_node, TreeNode::is_leaf/children/split/prediction, \
                    DecisionTree::iter_nodes/num_leaves"
            .to_string(),
        model,
    }
}

/// Fits and evaluates `smartcore::tree::DecisionTreeClassifier` at `max_depth = 8`. `smartcore`
/// exposes no public node accessor; the `serde` feature makes the fitted tree `Serialize`, so
/// `serde_json::to_value` is the traversal route (`YES-via-serde`).
pub fn fit_smartcore(built: &Built) -> SmartcoreResult {
    let train_x: DenseMatrix<f64> = DenseMatrix::from_2d_vec(&built.train_rows).expect("build train matrix");
    let holdout_x: DenseMatrix<f64> = DenseMatrix::from_2d_vec(&built.holdout_rows).expect("build holdout matrix");
    let train_y: Vec<u32> = built.train_labels.iter().map(|&l| l as u32).collect();
    let holdout_y: Vec<u32> = built.holdout_labels.iter().map(|&l| l as u32).collect();

    let mut times = Vec::with_capacity(3);
    let mut models = Vec::with_capacity(3);
    for _ in 0..3 {
        let start = Instant::now();
        let params = DecisionTreeClassifierParameters::default().with_max_depth(8);
        let model = DecisionTreeClassifier::fit(&train_x, &train_y, params).expect("smartcore fit succeeds");
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        models.push(model);
    }
    times.sort_by(|a, b| a.partial_cmp(b).expect("fit times are finite"));
    let fit_ms = times[1];

    let dump0 = serde_json::to_string(&models[0]).expect("serialize smartcore tree");
    let dump1 = serde_json::to_string(&models[1]).expect("serialize smartcore tree");
    let deterministic = dump0 == dump1;

    let model = models.pop().expect("3 fits produced 3 models");
    let value = serde_json::to_value(&model).expect("serialize smartcore tree to value");
    let nodes = value["nodes"].as_array().expect("nodes array present in serialized tree");
    let node_count = nodes.len();
    let leaf_count = nodes.iter().filter(|n| n["true_child"].is_null()).count();

    let train_pred = model.predict(&train_x).expect("smartcore predict on train");
    let holdout_pred = model.predict(&holdout_x).expect("smartcore predict on holdout");

    SmartcoreResult {
        train_accuracy: accuracy_u32(&train_pred, &train_y),
        holdout_accuracy: accuracy_u32(&holdout_pred, &holdout_y),
        fit_ms,
        deterministic,
        traversable: "YES-via-serde",
        api_used: "serde_json::to_value(&DecisionTreeClassifier) exposing nodes[].{output,split_feature,\
                    split_value,true_child,false_child}, classes[]"
            .to_string(),
        node_count,
        leaf_count,
    }
}
