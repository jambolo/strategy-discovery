//! Non-failing micro-benchmark for dataset construction and decision-tree mining: end-to-end
//! timing of [`strategy_discovery::discovery::dataset::build_dataset`] and per-engine timing of
//! [`strategy_discovery::discovery::mine::Miner::generate`] over `tests/fixtures/generate-small.toml`.
//! Ordinary `#[test]` fn timed with `std::time::Instant`; it never fails on timing and only
//! asserts sanity (non-zero rows, expected candidate counts). Run with
//! `cargo test --release --test mining_bench -- --nocapture` for meaningful numbers.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use strategy_discovery::core::traits::StrategyGenerator;
use strategy_discovery::discovery::annotate::{AnnotateOptions, annotate_corpus, annotate_exhaustive};
use strategy_discovery::discovery::config::GenerateConfig;
use strategy_discovery::discovery::corpus::{GenerateOptions, generate};
use strategy_discovery::discovery::dataset::{DatasetSource, build_dataset};
use strategy_discovery::discovery::evaluate::load_annotations;
use strategy_discovery::discovery::mine::Miner;
use strategy_discovery::games::tictactoe::{Move, game_bundle};
use strategy_discovery::io::MineParams;

#[test]
fn bench_dataset_build_and_mining() {
    let bundle = game_bundle();

    let config = GenerateConfig::<Move>::from_toml_file(Path::new("tests/fixtures/generate-small.toml")).unwrap();

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("mining_bench");
    let _ = std::fs::remove_dir_all(&dir);

    let metadata = generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
    annotate_corpus(&bundle, &dir, &AnnotateOptions::default()).unwrap();

    let annotations = load_annotations(&bundle, &dir).unwrap();
    let featurizer = Arc::new(bundle.featurizer(true).unwrap());

    let start = Instant::now();
    let dataset = build_dataset(
        &bundle,
        &featurizer,
        &annotations.records,
        DatasetSource {
            run_id: Some(metadata.run_id.clone()),
            annotations_mode: "corpus".to_string(),
        },
    )
    .unwrap();
    let ms = start.elapsed().as_secs_f64() * 1000.0;

    println!(
        "[bench] dataset-build rows={} columns={} ms={ms:.1}",
        dataset.manifest.rows,
        dataset.manifest.columns.len()
    );
    assert!(dataset.manifest.rows > 0);

    for engine in ["cart", "linfa-trees"] {
        let params = MineParams {
            engine: engine.to_string(),
            depths: vec![4, 8, 0],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        let mut miner = Miner::new(Arc::clone(&featurizer), params);

        let start = Instant::now();
        let candidates = miner.generate(&dataset, 0).unwrap();
        // Whole-call elapsed divided by candidate count: a per-candidate approximation, not an
        // isolated per-depth measurement (all three depths are fit in one `generate` call).
        let ms = start.elapsed().as_secs_f64() * 1000.0 / candidates.len() as f64;

        assert_eq!(candidates.len(), 3);
        for candidate in &candidates {
            println!(
                "[bench] mine engine={engine} depth={} ms={ms:.1} rules={} leaves={} soundness={:.3}",
                candidate.max_depth, candidate.rules, candidate.leaves, candidate.train_soundness
            );
        }
    }

    let exhaustive_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("mining_bench_exhaustive");
    let _ = std::fs::remove_dir_all(&exhaustive_dir);

    annotate_exhaustive(&bundle, &exhaustive_dir, &AnnotateOptions::default()).unwrap();
    let exhaustive_annotations = load_annotations(&bundle, &exhaustive_dir).unwrap();

    let start = Instant::now();
    let dataset = build_dataset(
        &bundle,
        &featurizer,
        &exhaustive_annotations.records,
        DatasetSource {
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
        },
    )
    .unwrap();
    let ms = start.elapsed().as_secs_f64() * 1000.0;

    println!(
        "[bench] dataset-build source=exhaustive rows={} columns={} ms={ms:.1}",
        dataset.manifest.rows,
        dataset.manifest.columns.len()
    );
    assert!(dataset.manifest.rows > 0 && dataset.manifest.rows < 765);
    assert_eq!(dataset.manifest.annotated, 5478);
    assert_eq!(dataset.manifest.nonterminal, 4520);

    for engine in ["cart", "linfa-trees"] {
        let params = MineParams {
            engine: engine.to_string(),
            depths: vec![4, 8, 0],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.2,
        };
        let mut miner = Miner::new(Arc::clone(&featurizer), params);

        let start = Instant::now();
        let candidates = miner.generate(&dataset, 0).unwrap();
        // Whole-call elapsed divided by candidate count: a per-candidate approximation, not an
        // isolated per-depth measurement (all three depths are fit in one `generate` call).
        let ms = start.elapsed().as_secs_f64() * 1000.0 / candidates.len() as f64;

        assert_eq!(candidates.len(), 3);
        for candidate in &candidates {
            println!(
                "[bench] mine source=exhaustive engine={engine} depth={} ms={ms:.1} rules={} leaves={} train_soundness={:.3} holdout_soundness={:.3}",
                candidate.max_depth,
                candidate.rules,
                candidate.leaves,
                candidate.train_soundness,
                candidate.holdout_soundness.expect("holdout_fraction 0.2 yields a holdout")
            );
        }
    }
}
