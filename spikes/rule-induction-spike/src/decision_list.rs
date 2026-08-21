//! Renders one fitted tree as an ordered decision list: a depth-first walk (left before right)
//! of the root-to-leaf paths, each becoming one `IF ... THEN class=...` rule.

use linfa_trees::{DecisionTree, TreeNode};

/// The printed decision list plus its source and default (`ELSE`) class.
pub struct Decision {
    pub source: String,
    pub rules: Vec<String>,
    pub majority_class: String,
}

fn class_name(v: usize) -> &'static str {
    match v {
        0 => "loss",
        1 => "draw",
        2 => "win",
        _ => "unknown",
    }
}

/// Walks `model`'s tree and returns the numbered rule list plus the majority class of
/// `train_labels`, used for the trailing `ELSE` line.
pub fn render(model: &DecisionTree<f64, usize>, column_names: &[String], train_labels: &[i64]) -> Decision {
    let mut rules = Vec::new();
    let mut conditions: Vec<String> = Vec::new();
    walk(model.root_node(), column_names, &mut conditions, &mut rules);

    let mut counts = [0usize; 3];
    for &class in train_labels {
        counts[class as usize] += 1;
    }
    let majority = counts
        .iter()
        .enumerate()
        .max_by_key(|&(_, &count)| count)
        .map_or(1, |(i, _)| i);

    Decision {
        source: "linfa-trees".to_string(),
        rules,
        majority_class: class_name(majority).to_string(),
    }
}

fn walk(node: &TreeNode<f64, usize>, names: &[String], conditions: &mut Vec<String>, rules: &mut Vec<String>) {
    if node.is_leaf() {
        let prediction = node.prediction().expect("leaf node has a prediction");
        let rule = format!(
            "{}. IF {} THEN class={}",
            rules.len() + 1,
            conditions.join(" AND "),
            class_name(prediction)
        );
        rules.push(rule);
        return;
    }

    let (feature_idx, split_value, _impurity) = node.split();
    let name = names
        .get(feature_idx)
        .cloned()
        .unwrap_or_else(|| format!("feature-{feature_idx}"));
    let children = node.children();

    if let Some(left) = children[0] {
        conditions.push(format!("{name} <= {split_value:.3}"));
        walk(left, names, conditions, rules);
        conditions.pop();
    }
    if let Some(right) = children[1] {
        conditions.push(format!("{name} > {split_value:.3}"));
        walk(right, names, conditions, rules);
        conditions.pop();
    }
}
