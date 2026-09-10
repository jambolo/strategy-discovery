//! Integration tests for the miner: the exhaustive `cart` depth-8 candidate is valid, sound and
//! byte-deterministic, `cart` and `linfa-trees` agree on a tie-free toy dataset, and the holdout
//! split behaves. The expensive exhaustive annotation is run exactly once per process, behind a
//! `OnceLock`, shared by every test that needs it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use strategy_discovery::core::features::Tier;
use strategy_discovery::core::traits::{GameRules, StrategyGenerator};
use strategy_discovery::discovery::annotate::{AnnotateOptions, annotate_exhaustive};
use strategy_discovery::discovery::dataset::{Dataset, DatasetSource, build_dataset};
use strategy_discovery::discovery::evaluate::load_annotations;
use strategy_discovery::discovery::mine::Miner;
use strategy_discovery::games::tictactoe::{TicTacToe, TicTacToeRules, game_bundle};
use strategy_discovery::io::{
    DatasetColumn, DatasetManifest, DatasetRow, MINE_ENGINE_CART, MINE_ENGINE_LINFA_TREES, MineParams, MiningReport, SCHEMA_VERSION,
};

/// Fresh, empty output directory for one test, under the integration-test temp dir.
fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the exhaustive annotation exactly once per process and returns its output directory.
fn exhaustive_dir() -> &'static PathBuf {
    static EXHAUSTIVE: OnceLock<PathBuf> = OnceLock::new();
    EXHAUSTIVE.get_or_init(|| {
        let dir = out_dir("mine-exhaustive");
        annotate_exhaustive(&game_bundle(), &dir, &AnnotateOptions::default()).unwrap();
        dir
    })
}

/// Builds the exhaustive dataset fresh from the shared annotation directory.
fn exhaustive_dataset() -> Dataset<TicTacToe> {
    let bundle = game_bundle();
    let dir = exhaustive_dir();
    let annotations = load_annotations(&bundle, dir).unwrap();
    let featurizer = bundle.featurizer(true).unwrap();
    build_dataset(
        &bundle,
        &featurizer,
        &annotations.records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap()
}

/// A hand-built, tie-free toy dataset (mirrors `src/discovery/mine.rs::tests::toy_dataset_tie_free`):
/// the unique best split (`ttt.threats.mine <= 1`) makes both engines emit identical rules.
fn toy_dataset_tie_free() -> Dataset<TicTacToe> {
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

    let initial = <TicTacToeRules as GameRules<TicTacToe>>::initial_state(&TicTacToeRules);

    let mut rows: Vec<DatasetRow<strategy_discovery::games::tictactoe::Board>> = Vec::new();
    for x in [0.0f64, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0] {
        let (label, qualifying) = if x <= 1.0 {
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

#[test]
fn exhaustive_cart_depth8_candidate_is_sound_and_deterministic() {
    let bundle = game_bundle();
    let featurizer = Arc::new(bundle.featurizer(true).unwrap());

    let dataset_a = exhaustive_dataset();
    let dataset_b = exhaustive_dataset();

    let params = MineParams {
        engine: MINE_ENGINE_CART.to_string(),
        depths: vec![8],
        min_leaf: 1,
        seed: 0,
        holdout_fraction: 0.0,
    };

    let mut miner_a = Miner::new(featurizer.clone(), params.clone());
    let candidates_a = miner_a.generate(&dataset_a, 0).unwrap();
    let mut miner_b = Miner::new(featurizer.clone(), params.clone());
    let candidates_b = miner_b.generate(&dataset_b, 0).unwrap();

    let candidate = &candidates_a[0];
    assert_eq!(candidate.name, "mined-d8-l1");
    assert_eq!(candidate.engine, "cart");
    assert!(candidate.heuristic.validate().is_ok());
    assert!(candidate.rules >= 1);
    assert!(candidate.train_soundness >= 0.9);
    assert_eq!(candidate.train_rows, dataset_a.manifest.rows);
    assert_eq!(candidate.holdout_rows, 0);
    assert!(candidate.holdout_accuracy.is_none());
    assert!(candidate.fallback_rows < candidate.train_rows);
    assert_eq!(candidate.evidence.len(), candidate.rules);

    let report_a = MiningReport {
        schema_version: SCHEMA_VERSION,
        game: "tictactoe".to_string(),
        run_id: None,
        params: params.clone(),
        dataset: dataset_a.manifest.clone(),
        candidates: candidates_a,
    };
    let report_b = MiningReport {
        schema_version: SCHEMA_VERSION,
        game: "tictactoe".to_string(),
        run_id: None,
        params,
        dataset: dataset_b.manifest.clone(),
        candidates: candidates_b,
    };

    let json_a = serde_json::to_string_pretty(&report_a).unwrap();
    let json_b = serde_json::to_string_pretty(&report_b).unwrap();
    assert_eq!(json_a, json_b);
}

#[test]
fn holdout_split_is_seeded_and_sized() {
    let bundle = game_bundle();
    let featurizer = Arc::new(bundle.featurizer(true).unwrap());
    let dataset = exhaustive_dataset();

    let params = MineParams {
        holdout_fraction: 0.2,
        ..MineParams::default()
    };

    let mut miner_1 = Miner::new(featurizer.clone(), params.clone());
    let candidates_1 = miner_1.generate(&dataset, 7).unwrap();
    let mut miner_2 = Miner::new(featurizer.clone(), params.clone());
    let candidates_2 = miner_2.generate(&dataset, 7).unwrap();

    let json_1 = serde_json::to_string_pretty(&candidates_1).unwrap();
    let json_2 = serde_json::to_string_pretty(&candidates_2).unwrap();
    assert_eq!(json_1, json_2);

    let candidate = &candidates_1[0];
    assert_eq!(candidate.holdout_rows, dataset.manifest.rows / 5);
    assert_eq!(candidate.train_rows + candidate.holdout_rows, dataset.manifest.rows);
    assert!(candidate.holdout_accuracy.is_some());
    assert!(candidate.holdout_soundness.is_some());

    let mut miner_3 = Miner::new(featurizer, params);
    let candidates_3 = miner_3.generate(&dataset, 8).unwrap();
    assert!(!candidates_3.is_empty());
}

#[test]
fn linfa_and_cart_agree_on_tie_free_toy() {
    let bundle = game_bundle();
    let featurizer = Arc::new(bundle.featurizer(true).unwrap());
    let dataset = toy_dataset_tie_free();

    let cart_params = MineParams {
        engine: MINE_ENGINE_CART.to_string(),
        depths: vec![2],
        min_leaf: 1,
        seed: 0,
        holdout_fraction: 0.0,
    };
    let mut cart_miner = Miner::new(featurizer.clone(), cart_params);
    let cart_candidates = cart_miner.generate(&dataset, 0).unwrap();
    let cart_candidate = &cart_candidates[0];

    let linfa_params = MineParams {
        engine: MINE_ENGINE_LINFA_TREES.to_string(),
        depths: vec![2],
        min_leaf: 1,
        seed: 0,
        holdout_fraction: 0.0,
    };
    let mut linfa_miner = Miner::new(featurizer, linfa_params);
    let linfa_candidates = linfa_miner.generate(&dataset, 0).unwrap();
    let linfa_candidate = &linfa_candidates[0];

    assert_eq!(cart_candidate.heuristic.rules, linfa_candidate.heuristic.rules);
    assert_eq!(cart_candidate.rules, 2);
    assert_eq!(linfa_candidate.rules, 2);
    assert_eq!(cart_candidate.evidence, linfa_candidate.evidence);
    assert_eq!(cart_candidate.name, "mined-d2-l1");
    assert_eq!(linfa_candidate.name, "mined-linfa-d2-l1");
    assert!(cart_candidate.heuristic.validate().is_ok());
    assert!(linfa_candidate.heuristic.validate().is_ok());
}
