//! Non-failing micro-benchmark for match throughput: depth-9 tic-tac-toe self-play games/s,
//! serial vs parallel, plus `TieBreak::Engine` PV-replay overhead. Ordinary `#[test]` fns timed
//! with `std::time::Instant`; they never fail on timing and only assert sanity. Run with
//! `cargo test --release --test match_bench -- --nocapture` for meaningful numbers; the debug
//! build just checks the tests complete (both tests together take roughly 20 s in debug).

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;
use strategy_discovery::core::traits::{GameRules, MatchConfig, MatchEngine, StrategyProvider};
use strategy_discovery::discovery::RayonMatchEngine;
use strategy_discovery::games::tictactoe::{Player, TicTacToe, TicTacToeEvaluator, TicTacToeRules};
use strategy_discovery::strategy::{MinimaxConfig, MinimaxProvider, TieBreak};

#[test]
fn bench_depth9_selfplay_serial_vs_parallel() {
    let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
    let provider = MinimaxProvider::new(
        Arc::clone(&rules),
        Arc::new(TicTacToeEvaluator),
        MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        },
    )
    .unwrap();
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &provider), (Player::O, &provider)];
    let config = MatchConfig {
        games: 200,
        seed: 20260821,
        max_plies: None,
    };

    let serial_engine = RayonMatchEngine::serial(Arc::clone(&rules));
    let start = Instant::now();
    let serial_results = serial_engine.run(&config, players).unwrap();
    let serial_elapsed = start.elapsed();
    black_box(&serial_results);

    let parallel_engine = RayonMatchEngine::new(Arc::clone(&rules));
    let start = Instant::now();
    let parallel_results = parallel_engine.run(&config, players).unwrap();
    let parallel_elapsed = start.elapsed();
    black_box(&parallel_results);

    let serial_gps = config.games as f64 / serial_elapsed.as_secs_f64();
    let parallel_gps = config.games as f64 / parallel_elapsed.as_secs_f64();
    let speedup = parallel_gps / serial_gps;

    println!(
        "[bench] games/s serial={serial_gps:.0} parallel={parallel_gps:.0} speedup={speedup:.2}x (200 games, depth 9, seeded-uniform)"
    );

    assert_eq!(serial_results.len(), 200);
    assert_eq!(parallel_results.len(), 200);
    assert_eq!(serial_results, parallel_results);
}

#[test]
fn bench_depth9_selfplay_engine_tie_break() {
    let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
    let provider = MinimaxProvider::new(
        Arc::clone(&rules),
        Arc::new(TicTacToeEvaluator),
        MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::Engine,
        },
    )
    .unwrap();
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &provider), (Player::O, &provider)];
    let config = MatchConfig {
        games: 200,
        seed: 20260821,
        max_plies: None,
    };

    let engine = RayonMatchEngine::serial(rules);
    let start = Instant::now();
    let results = engine.run(&config, players).unwrap();
    let elapsed = start.elapsed();
    black_box(&results);

    let gps = config.games as f64 / elapsed.as_secs_f64();
    println!("[bench] games/s engine-tie-break serial={gps:.0} (200 games, depth 9)");

    assert_eq!(results.len(), 200);
}
