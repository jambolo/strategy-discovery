//! Builds the labelled per-position feature dataset over all 5478 reachable tic-tac-toe
//! positions and the seeded 80/20 train/held-out split.

use crate::solver::{self, Solved};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use strategy_discovery::core::derived::PrimitiveFeatures;
use strategy_discovery::core::features::FeatureValue;
use strategy_discovery::core::traits::{Canonicalize, FeatureExtractor, GameRules};
use strategy_discovery::games::tictactoe::TicTacToeRules;
use strategy_discovery::games::tictactoe::canonical::TicTacToeCanonicalizer;
use strategy_discovery::games::tictactoe::features::TicTacToeFeatures;
use strategy_discovery::games::tictactoe::primitives::TicTacToePrimitives;

/// Exact versions of the candidate crates, resolved from the spike's own `Cargo.lock`.
pub struct Versions {
    pub linfa: String,
    pub linfa_trees: String,
    pub smartcore: String,
    pub ndarray: String,
}

/// The built dataset: full feature matrix, labels, seeded train/held-out split, and metadata.
pub struct Built {
    pub seed: u64,
    pub n_rows: usize,
    pub n_features: usize,
    pub column_names: Vec<String>,
    pub class_counts: [usize; 3],
    pub train_rows: Vec<Vec<f64>>,
    pub train_labels: Vec<i64>,
    pub holdout_rows: Vec<Vec<f64>>,
    pub holdout_labels: Vec<i64>,
    pub versions: Versions,
}

fn feature_value_to_f64(v: &FeatureValue) -> f64 {
    match v {
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
    }
}

/// Resolves `name`'s exact version from `Cargo.lock` by scanning for `name = "<name>"` followed
/// by a `version = "..."` line.
fn crate_version(lock_contents: &str, name: &str) -> String {
    let needle = format!("name = \"{name}\"");
    let mut lines = lock_contents.lines();
    while let Some(line) = lines.next() {
        if line.trim() == needle
            && let Some(version_line) = lines.next()
            && let Some(rest) = version_line.trim().strip_prefix("version = \"")
        {
            return rest.trim_end_matches('"').to_string();
        }
    }
    "unknown".to_string()
}

fn resolve_versions() -> Versions {
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    let contents = fs::read_to_string(lock_path).expect("read spike Cargo.lock");
    Versions {
        linfa: crate_version(&contents, "linfa"),
        linfa_trees: crate_version(&contents, "linfa-trees"),
        smartcore: crate_version(&contents, "smartcore"),
        ndarray: crate_version(&contents, "ndarray"),
    }
}

/// Builds the full dataset and seeded 80/20 split.
pub fn build(seed: u64) -> Built {
    let rules = TicTacToeRules;
    let primitives = TicTacToePrimitives;
    let canon = TicTacToeCanonicalizer::new();
    let tier1 = PrimitiveFeatures::new(rules, primitives);
    let tier2 = TicTacToeFeatures;

    let reachable = rules.reachable_positions();

    let mut memo: HashMap<_, Solved> = HashMap::new();
    solver::solve(&rules, &mut memo, rules.initial_state());

    let mut rows: Vec<Vec<f64>> = Vec::with_capacity(reachable.len());
    let mut labels: Vec<i64> = Vec::with_capacity(reachable.len());
    let mut column_names: Vec<String> = Vec::new();

    for (i, position) in reachable.iter().enumerate() {
        let canonical = canon.canonicalize(position);
        let fv = tier1.extract(&canonical).merged(&tier2.extract(&canonical));

        if i == 0 {
            column_names = fv.iter().map(|(name, _)| name.clone()).collect();
        }

        let row: Vec<f64> = fv.iter().map(|(_, value)| feature_value_to_f64(value)).collect();
        rows.push(row);

        let solved = memo.get(position).expect("every reachable position was solved");
        labels.push(i64::from(solved.value) + 1);
    }

    let n_rows = rows.len();
    let n_features = column_names.len();

    let mut class_counts = [0usize; 3];
    for &class in &labels {
        class_counts[class as usize] += 1;
    }

    let mut indices: Vec<usize> = (0..n_rows).collect();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    indices.shuffle(&mut rng);

    let train_len = n_rows * 80 / 100;
    let (train_idx, holdout_idx) = indices.split_at(train_len);

    let gather_rows = |idx: &[usize]| -> Vec<Vec<f64>> { idx.iter().map(|&i| rows[i].clone()).collect() };
    let gather_labels = |idx: &[usize]| -> Vec<i64> { idx.iter().map(|&i| labels[i]).collect() };

    Built {
        seed,
        n_rows,
        n_features,
        column_names,
        class_counts,
        train_rows: gather_rows(train_idx),
        train_labels: gather_labels(train_idx),
        holdout_rows: gather_rows(holdout_idx),
        holdout_labels: gather_labels(holdout_idx),
        versions: resolve_versions(),
    }
}
