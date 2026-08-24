//! Dataset acceptance tests: symmetric-duplicate collapse on exhaustive annotations, corpus-mode
//! dataset construction, the winning-cells soundness guarantee, and JSONL/JSON round trips.
//! The expensive exhaustive annotation is run exactly once per process, behind a `OnceLock`,
//! and shared by every test that needs it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use strategy_discovery::core::features::{CmpOp, FeatureDef, FeatureExpr, Tier};
use strategy_discovery::discovery::annotate::{AnnotateOptions, annotate_corpus, annotate_exhaustive};
use strategy_discovery::discovery::config::GenerateConfig;
use strategy_discovery::discovery::corpus::{GenerateOptions, generate};
use strategy_discovery::discovery::dataset::{DatasetSource, build_dataset};
use strategy_discovery::discovery::evaluate::load_annotations;
use strategy_discovery::games::tictactoe::{Board, Move, game_bundle};
use strategy_discovery::io::{DatasetManifest, DatasetRow, read_json, read_jsonl, write_json_pretty, write_jsonl};

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
        let dir = out_dir("dataset-exhaustive");
        annotate_exhaustive(&game_bundle(), &dir, &AnnotateOptions::default()).unwrap();
        dir
    })
}

#[test]
fn dataset_collapses_symmetric_duplicates() {
    let bundle = game_bundle();
    let dir = exhaustive_dir();
    let annotations = load_annotations(&bundle, dir).unwrap();
    let records = &annotations.records;

    let nonterminal = records.iter().filter(|r| !r.terminal).count();
    assert_eq!(nonterminal, 4520);

    let distinct: HashSet<_> = records.iter().filter(|r| !r.terminal).map(|r| r.canonical_state).collect();

    let featurizer = bundle.featurizer(true).unwrap();
    let dataset = build_dataset(
        &bundle,
        &featurizer,
        records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap();
    let manifest = &dataset.manifest;

    assert_eq!(manifest.rows, distinct.len());
    assert!(manifest.rows < 765);
    assert_eq!(manifest.collapsed, manifest.nonterminal - manifest.rows);
    assert_eq!(manifest.nonterminal, 4520);
    assert_eq!(manifest.annotated, 5478);
    assert_eq!(dataset.rows.len(), manifest.rows);
}

#[test]
fn winning_cells_always_qualify() {
    let bundle = game_bundle();
    let dir = exhaustive_dir();
    let annotations = load_annotations(&bundle, dir).unwrap();
    let records = &annotations.records;
    let featurizer = bundle.featurizer(true).unwrap();

    let dataset = build_dataset(
        &bundle,
        &featurizer,
        records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap();
    let manifest = &dataset.manifest;

    let w = dataset.column_index("ttt.winning_cells").unwrap();

    let mut winning_rows = 0usize;
    for row in &dataset.rows {
        if row.values[w] > 0.0 {
            winning_rows += 1;
            assert!(row.qualifying.iter().any(|c| c == "ttt.winning_cells"));
            assert_ne!(row.label, "none");
            let label_column = manifest.columns.iter().find(|c| c.name == row.label).unwrap();
            assert_eq!(label_column.tier, Tier::Supplied);
        }
    }
    assert!(winning_rows >= 100);

    assert!(manifest.unlabeled * 100 < manifest.rows * 5);
}

#[test]
fn corpus_mode_dataset_matches_annotations() {
    let bundle = game_bundle();
    let dir = out_dir("dataset-corpus");

    let config = GenerateConfig::<Move>::from_toml_file(Path::new("tests/fixtures/generate-small.toml")).unwrap();
    generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
    annotate_corpus(&bundle, &dir, &AnnotateOptions::default()).unwrap();

    let annotations = load_annotations(&bundle, &dir).unwrap();
    let records = &annotations.records;
    let featurizer = bundle.featurizer(true).unwrap();

    let dataset = build_dataset(
        &bundle,
        &featurizer,
        records,
        DatasetSource {
            run_id: annotations.metadata.run_id.clone(),
            annotations_mode: "corpus".to_string(),
        },
    )
    .unwrap();
    let manifest = &dataset.manifest;

    assert_eq!(manifest.run_id.as_deref(), Some("5eafa65f76637df3"));
    assert_eq!(manifest.annotated, 390);

    let nonterminal_distinct: HashSet<_> = records.iter().filter(|r| !r.terminal).map(|r| r.canonical_state).collect();
    assert_eq!(manifest.rows, nonterminal_distinct.len());

    for row in &dataset.rows {
        assert_eq!(row.values.len(), manifest.columns.len());
        assert!(!row.legal.is_empty());
        assert!(!row.optimal.is_empty());
        let mut sorted_legal = row.legal.clone();
        sorted_legal.sort_unstable();
        sorted_legal.dedup();
        assert_eq!(row.legal, sorted_legal, "legal must be strictly increasing");
        let mut sorted_optimal = row.optimal.clone();
        sorted_optimal.sort_unstable();
        sorted_optimal.dedup();
        assert_eq!(row.optimal, sorted_optimal, "optimal must be strictly increasing");
        let legal_set: HashSet<_> = row.legal.iter().copied().collect();
        assert!(
            row.optimal.iter().all(|p| legal_set.contains(p)),
            "optimal must be a subset of legal"
        );
    }
}

#[test]
fn dataset_evaluates_pushed_derived_defs() {
    let bundle = game_bundle();
    let dir = exhaustive_dir();
    let annotations = load_annotations(&bundle, dir).unwrap();
    let records = &annotations.records;

    let baseline_featurizer = bundle.featurizer(true).unwrap();
    let baseline = build_dataset(
        &bundle,
        &baseline_featurizer,
        records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap();

    let mut featurizer = bundle.featurizer(true).unwrap();
    featurizer
        .push_derived(FeatureDef::derived(
            "derived.can_win",
            Tier::Invented,
            "a winning move exists",
            FeatureExpr::compare(
                CmpOp::Gt,
                FeatureExpr::count(FeatureExpr::named("ttt.winning_cells")),
                FeatureExpr::int(0),
            ),
        ))
        .unwrap();

    let dataset = build_dataset(
        &bundle,
        &featurizer,
        records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap();
    let manifest = &dataset.manifest;

    assert_eq!(manifest.columns.len(), 71);
    let last = manifest.columns.last().unwrap();
    assert_eq!(last.name, "derived.can_win");
    assert_eq!(last.kind, "bool");
    assert_eq!(last.tier, Tier::Invented);
    assert_eq!(last.description, "a winning move exists");
    assert_eq!(
        manifest.tiers,
        vec!["primitive".to_string(), "supplied".to_string(), "invented".to_string()]
    );
    assert_eq!(manifest.classes, baseline.manifest.classes);
    assert_eq!(manifest.rows, baseline.manifest.rows);

    let w = dataset.column_index("ttt.winning_cells").unwrap();
    for (row, base_row) in dataset.rows.iter().zip(&baseline.rows) {
        assert_eq!(row.values.len(), 71);
        assert_eq!(&row.values[..70], &base_row.values[..]);
        let expected = if row.values[w] > 0.0 { 1.0 } else { 0.0 };
        assert_eq!(row.values[70], expected);
        assert_eq!(row.label, base_row.label);
        assert_eq!(row.qualifying, base_row.qualifying);
    }
}

#[test]
fn rows_and_manifest_round_trip_jsonl() {
    let bundle = game_bundle();
    let dir = exhaustive_dir();
    let annotations = load_annotations(&bundle, dir).unwrap();
    let records = &annotations.records;
    let featurizer = bundle.featurizer(true).unwrap();

    let dataset = build_dataset(
        &bundle,
        &featurizer,
        records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap();

    let sample: Vec<DatasetRow<Board>> = dataset.rows.iter().take(5).cloned().collect();
    let round_trip_dir = out_dir("dataset-round-trip");
    let rows_path = round_trip_dir.join("rows.jsonl");
    write_jsonl(&rows_path, sample.iter()).unwrap();
    let read_back: Vec<DatasetRow<Board>> = read_jsonl(&rows_path).unwrap();
    assert_eq!(sample, read_back);

    let manifest_path = round_trip_dir.join("manifest.json");
    write_json_pretty(&manifest_path, &dataset.manifest).unwrap();
    let manifest_back: DatasetManifest = read_json(&manifest_path).unwrap();
    assert_eq!(dataset.manifest, manifest_back);
}
