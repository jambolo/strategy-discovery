//! Non-failing micro-benchmark for corpus generation: end-to-end throughput of
//! [`strategy_discovery::discovery::corpus::generate`] (serial vs parallel) plus the read+aggregate
//! cost of [`strategy_discovery::discovery::summary::analyze_corpus`] over the resulting run. Ordinary
//! `#[test]` fn timed with `std::time::Instant`; it never fails on timing and only asserts sanity.
//! Run with `cargo test --release --test corpus_bench -- --nocapture` for meaningful numbers.
//!
//! `configs/tictactoe-default.toml` sweeps all 49 pairing cells; its `games_per_cell` is
//! overridden here to 2 in debug builds (98 games total, ~1.7 s in debug with depth-9 minimax
//! cells) and 20 in release (980 games, ~1.3 s in release) so plain `cargo test` stays fast while
//! `--release` still produces a meaningful measurement.

use std::hint::black_box;
use std::time::Instant;
use strategy_discovery::discovery::config::GenerateConfig;
use strategy_discovery::discovery::corpus::{GenerateOptions, generate};
use strategy_discovery::discovery::summary::{DiversityThresholds, analyze_corpus};
use strategy_discovery::games::tictactoe::{Move, game_bundle};

#[test]
fn bench_corpus_generation_and_summary() {
    let bundle = game_bundle();

    let toml_text = std::fs::read_to_string("configs/tictactoe-default.toml").expect("configs/tictactoe-default.toml must exist");
    let mut config = GenerateConfig::<Move>::from_toml_str(&toml_text).unwrap();
    config.games_per_cell = if cfg!(debug_assertions) { 2 } else { 20 };
    let games_per_cell = config.games_per_cell;

    let base_dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("corpus_bench");
    let serial_dir = base_dir.join("serial");
    let parallel_dir = base_dir.join("parallel");

    let _ = std::fs::remove_dir_all(&serial_dir);
    let _ = std::fs::remove_dir_all(&parallel_dir);

    let serial_config = config.clone();
    let start = Instant::now();
    let serial_metadata = generate(
        &bundle,
        serial_config,
        &GenerateOptions {
            threads: None,
            serial: true,
        },
        &serial_dir,
    )
    .unwrap();
    let serial_elapsed = start.elapsed();
    black_box(&serial_metadata);

    let start = Instant::now();
    let parallel_metadata = generate(&bundle, config, &GenerateOptions::default(), &parallel_dir).unwrap();
    let parallel_elapsed = start.elapsed();
    black_box(&parallel_metadata);

    let serial_gps = serial_metadata.games as f64 / serial_elapsed.as_secs_f64();
    let parallel_gps = parallel_metadata.games as f64 / parallel_elapsed.as_secs_f64();
    let write_ms = parallel_elapsed.as_secs_f64() * 1000.0;

    println!(
        "[bench] corpus games/s serial={serial_gps:.0} parallel={parallel_gps:.0} games={} positions={} write_ms={write_ms:.1}",
        parallel_metadata.games, parallel_metadata.positions
    );

    assert_eq!(serial_metadata.games, parallel_metadata.games);
    assert_eq!(serial_metadata.positions, parallel_metadata.positions);
    assert_eq!(serial_metadata.games, 49 * games_per_cell);

    let start = Instant::now();
    let summary = analyze_corpus(&bundle, &parallel_dir, &DiversityThresholds::default()).unwrap();
    let summary_elapsed = start.elapsed();
    let ms = summary_elapsed.as_secs_f64() * 1000.0;

    println!(
        "[bench] summary read+aggregate ms={ms:.1} distinct_games={}",
        summary.distinct_games
    );

    assert!(parallel_dir.join("summary.json").is_file());
}
