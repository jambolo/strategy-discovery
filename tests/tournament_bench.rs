//! Non-failing micro-benchmark for the Milestone 1 evaluation harness: tournament throughput of
//! `perfect` against the full `ttt-benchmark-v1` roster (serial vs the global rayon pool vs
//! private pools of 2/4/8 threads), plus agreement throughput of `perfect` over the small
//! corpus's annotations. Ordinary `#[test]` fns timed with `std::time::Instant`; they never fail
//! on timing and only assert sanity. Run with
//! `cargo test --release --test tournament_bench -- --nocapture` for meaningful numbers; debug
//! builds use `games_per_pairing = 1` (a debug-assertions check) so plain `cargo test` stays fast.

use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use strategy_discovery::discovery::annotate::{AnnotateOptions, annotate_corpus};
use strategy_discovery::discovery::config::GenerateConfig;
use strategy_discovery::discovery::corpus::{GenerateOptions, generate};
use strategy_discovery::discovery::evaluate::{load_annotations, measure_agreement};
use strategy_discovery::discovery::tournament::{TournamentConfig, run_tournament_detailed};
use strategy_discovery::games::tictactoe::{Move, game_bundle};
use strategy_discovery::strategy::registry::StrategyRegistry;

/// One tournament mode: how the games are scheduled and the label printed for `threads=`.
struct Mode {
    mode: &'static str,
    serial: bool,
    threads: Option<usize>,
    label: &'static str,
}

const MODES: [Mode; 5] = [
    Mode {
        mode: "serial",
        serial: true,
        threads: None,
        label: "1",
    },
    Mode {
        mode: "pool",
        serial: false,
        threads: None,
        label: "global",
    },
    Mode {
        mode: "threads",
        serial: false,
        threads: Some(2),
        label: "2",
    },
    Mode {
        mode: "threads",
        serial: false,
        threads: Some(4),
        label: "4",
    },
    Mode {
        mode: "threads",
        serial: false,
        threads: Some(8),
        label: "8",
    },
];

#[test]
fn bench_tournament_threads_scaling() {
    let bundle = game_bundle();
    let registry = StrategyRegistry::new(bundle.engine_bundle("default").unwrap());
    let under_test = bundle.roster.get("perfect").unwrap();
    let roster = &bundle.roster;
    let games = if cfg!(debug_assertions) { 1 } else { 20 };
    let seed = 20260822;
    let reference = "perfect".to_string();

    let mut serial_elapsed_s = 0.0_f64;
    let mut serial_result = None;

    for m in &MODES {
        let config = TournamentConfig {
            games_per_pairing: games,
            seed,
            max_plies: None,
            reference: reference.clone(),
            threads: m.threads,
            serial: m.serial,
        };

        let start = Instant::now();
        let run = run_tournament_detailed(&bundle, &registry, under_test, roster, &config).unwrap();
        let elapsed_s = start.elapsed().as_secs_f64();
        black_box(&run);

        if m.serial {
            serial_elapsed_s = elapsed_s;
        }

        let games_total = run.result.totals.games;
        let plies = run.plies;
        let gps = games_total as f64 / elapsed_s;
        let us_per_move = elapsed_s * 1e6 / plies as f64;
        let speedup = serial_elapsed_s / elapsed_s;

        println!(
            "[bench] tournament mode={} threads={} games={games_total} plies={plies} games/s={gps:.1} us/move={us_per_move:.1} \
             speedup={speedup:.2}x (perfect vs ttt-benchmark-v1, {games} games/pairing, depth 9)",
            m.mode, m.label
        );

        assert_eq!(games_total, 14 * games);
        assert!(plies > 0);
        assert_eq!(run.result.totals.losses, 0);
        match &serial_result {
            None => serial_result = Some(run.result.clone()),
            Some(reference_result) => assert_eq!(&run.result, reference_result),
        }
    }
}

#[test]
fn bench_agreement_positions_per_second() {
    let bundle = game_bundle();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("tournament_bench");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let config =
        GenerateConfig::<Move>::from_toml_str(&std::fs::read_to_string("tests/fixtures/generate-small.toml").unwrap()).unwrap();
    generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();
    annotate_corpus(&bundle, &dir, &AnnotateOptions::default()).unwrap();
    let annotations = load_annotations(&bundle, &dir).unwrap();

    let registry = StrategyRegistry::new(bundle.engine_bundle("default").unwrap());
    let provider = registry.build(&bundle.roster.get("perfect").unwrap().spec).unwrap();

    let start = Instant::now();
    let outcome = measure_agreement(&bundle, provider.as_ref(), &annotations, 0).unwrap();
    let elapsed_s = start.elapsed().as_secs_f64();
    black_box(&outcome);

    let positions = outcome.summary.positions;
    let rate = outcome.summary.rate;
    let pps = positions as f64 / elapsed_s;

    println!("[bench] agreement positions/s={pps:.0} positions={positions} rate={rate:.3} (perfect, corpus annotations, depth 9)");

    assert_eq!(positions, 390);
    assert_eq!(rate, 1.0);
    assert_eq!(outcome.signature.actions.len(), 256);
}
