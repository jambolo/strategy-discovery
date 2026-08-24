//! Non-failing micro-benchmark for concept induction: end-to-end timing of
//! [`strategy_discovery::discovery::induce_concepts`] over `tests/fixtures/generate-small.toml`,
//! plus a probe-equivalent single cart fit over the same dataset for comparison. Ordinary
//! `#[test]` fn timed with `std::time::Instant`; it never fails on timing and only asserts
//! sanity. Run with `cargo test --release --test induction_bench -- --nocapture` for
//! meaningful numbers.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use strategy_discovery::core::featurizer::FeaturizerSpec;
use strategy_discovery::core::traits::StrategyGenerator;
use strategy_discovery::discovery::annotate::{AnnotateOptions, annotate_corpus};
use strategy_discovery::discovery::config::GenerateConfig;
use strategy_discovery::discovery::corpus::{GenerateOptions, generate};
use strategy_discovery::discovery::dataset::{DatasetSource, build_dataset};
use strategy_discovery::discovery::evaluate::load_annotations;
use strategy_discovery::discovery::induce_concepts;
use strategy_discovery::discovery::mine::Miner;
use strategy_discovery::games::tictactoe::{Move, game_bundle};
use strategy_discovery::io::{InductionParams, MineParams};

#[test]
fn bench_concept_induction() {
    let bundle = game_bundle();

    let config = GenerateConfig::<Move>::from_toml_file(Path::new("tests/fixtures/generate-small.toml")).unwrap();

    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("induction_bench");
    let _ = std::fs::remove_dir_all(&dir);

    let metadata = generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
    annotate_corpus(&bundle, &dir, &AnnotateOptions::default()).unwrap();

    let annotations = load_annotations(&bundle, &dir).unwrap();

    let featurizer = Arc::new(
        bundle
            .featurizer_with(FeaturizerSpec {
                include_supplied: false,
                extended: true,
            })
            .unwrap(),
    );

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

    println!(
        "[bench] induce-dataset rows={} columns={}",
        dataset.manifest.rows,
        dataset.manifest.columns.len()
    );
    assert!(dataset.manifest.rows > 0);

    let params = InductionParams {
        enabled: true,
        withhold_tier2: true,
        rounds: 2,
        beam: 24,
        top_k: 4,
        max_promoted: 2,
        min_train_gain: 0.001,
        min_holdout_gain: 0.0005,
        holdout_fraction: 0.25,
        ..Default::default()
    };

    let start = Instant::now();
    let report = induce_concepts(&bundle, &dataset, &params).unwrap();
    let ms = start.elapsed().as_secs_f64() * 1000.0;

    println!(
        "[bench] induce ms={ms:.1} rounds={} promoted={}",
        report.rounds.len(),
        report.promoted.len()
    );

    for round in &report.rounds {
        println!(
            "[bench] induce-round round={} enumerated={} deduplicated={} scored={} passed={} shortlisted={} promoted={}",
            round.round,
            round.enumerated,
            round.deduplicated,
            round.scored,
            round.passed,
            round.shortlisted.len(),
            round.promoted.len()
        );
    }

    assert!(!report.rounds.is_empty());

    let featurizer2 = Arc::new(
        bundle
            .featurizer_with(FeaturizerSpec {
                include_supplied: false,
                extended: true,
            })
            .unwrap(),
    );
    let mine_params = MineParams {
        engine: "cart".to_string(),
        depths: vec![6],
        min_leaf: 1,
        seed: 0,
        holdout_fraction: 0.0,
    };
    let mut miner = Miner::new(Arc::clone(&featurizer2), mine_params);

    let start = Instant::now();
    let candidates = miner.generate(&dataset, 0).unwrap();
    let ms = start.elapsed().as_secs_f64() * 1000.0;

    assert_eq!(candidates.len(), 1);
    println!(
        "[bench] probe-fit engine=cart depth=6 ms={ms:.1} rules={}",
        candidates[0].rules
    );
}
