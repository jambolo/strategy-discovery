//! Non-failing micro-benchmarks for tic-tac-toe: board-clone cost and legal-move-generation
//! throughput. Ordinary `#[test]` functions timed with `std::time::Instant`; they never fail on
//! timing and only assert sanity. Run with `cargo test --release --test ttt_bench -- --nocapture`
//! for meaningful numbers; the debug build just checks the tests complete quickly.

use std::hint::black_box;
use std::time::Instant;
use strategy_discovery::core::traits::GameRules;
use strategy_discovery::games::tictactoe::{Move, TicTacToeRules};

#[test]
#[allow(clippy::clone_on_copy)] // benchmarking `Clone::clone` itself, not just a copy
fn bench_board_clone() {
    let rules = TicTacToeRules;
    let board = rules.initial_state();
    let board = rules.apply(&board, &Move(4)).unwrap();
    let board = rules.apply(&board, &Move(0)).unwrap();

    let iters = 1_000_000;
    let mut acc: u64 = 0;
    let start = Instant::now();
    for _ in 0..iters {
        let c = black_box(board).clone();
        acc ^= black_box(c).cells()[0].is_some() as u64;
    }
    let elapsed = start.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    println!("[bench] clone: {ns_per_op:.2} ns/op over {iters} iters (debug/release unknown to test)");
    assert!(iters > 0);
    black_box(acc);
}

#[test]
fn bench_move_generation() {
    let rules = TicTacToeRules;
    let positions = rules.reachable_positions();
    assert_eq!(positions.len(), 5478);

    let rounds = 20;
    let mut total_moves: u64 = 0;
    let start = Instant::now();
    for _ in 0..rounds {
        for p in &positions {
            total_moves += rules.legal_actions(black_box(p)).len() as u64;
        }
    }
    let elapsed = start.elapsed();
    let calls = rounds * positions.len();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
    let calls_per_sec = calls as f64 / elapsed.as_secs_f64();

    println!("[bench] legal_actions: {calls_per_sec:.0} calls/s ({calls} calls, {total_moves} moves) in {elapsed_ms:.1} ms");
    assert!(total_moves > 0);
}
