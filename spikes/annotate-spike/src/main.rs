//! Throwaway spike: measures `game_player::minimax::search` per position over all tic-tac-toe
//! positions, documents what the API exposes, prototypes an exhaustive solver fallback,
//! cross-checks the two, and writes the annotation evidence artifacts.

use chrono::Utc;
use clap::Parser;
use game_player::StaticEvaluator;
use game_player::minimax::{ResponseGenerator, search};
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use strategy_discovery::core::traits::{Canonicalize, GameRules};
use strategy_discovery::games::tictactoe::board::{Board, Move, Outcome, Player};
use strategy_discovery::games::tictactoe::canonical::TicTacToeCanonicalizer;
use strategy_discovery::games::tictactoe::rules::TicTacToeRules;

const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

/// Spike-local static evaluator: exact win values on terminal states, open-lines heuristic
/// otherwise. Alice is X, Bob is O (see `TicTacToe`'s `From<Player> for PlayerId`).
struct Evaluator;

impl StaticEvaluator for Evaluator {
    type State = Board;

    fn evaluate(&self, board: &Board) -> f32 {
        match board.winner() {
            Some(Player::X) => self.alice_wins_value(),
            Some(Player::O) => self.bob_wins_value(),
            None => {
                let mut value = 0.0;
                for line in &LINES {
                    let has_x = line.iter().any(|&i| board.cell(i) == Some(Player::X));
                    let has_o = line.iter().any(|&i| board.cell(i) == Some(Player::O));
                    if !has_o {
                        value += 1.0;
                    }
                    if !has_x {
                        value -= 1.0;
                    }
                }
                value
            }
        }
    }

    fn alice_wins_value(&self) -> f32 {
        100.0
    }

    fn bob_wins_value(&self) -> f32 {
        -100.0
    }
}

/// Spike-local response generator: delegates to `TicTacToeRules::legal_actions`, which is
/// already empty iff terminal.
struct Moves;

impl ResponseGenerator for Moves {
    type State = Board;

    fn generate(&self, board: &Board, _depth: u32) -> Vec<Move> {
        TicTacToeRules.legal_actions(board)
    }
}

/// Result of exhaustively solving a position by negamax, from the side-to-move's perspective:
/// `+1` wins, `0` draws, `-1` loses.
#[derive(Clone)]
struct Solved {
    value: i8,
    optimal: Vec<Move>,
}

/// Exhaustive negamax solver with memoization. `value` is from the perspective of the side to
/// move in `state`. `optimal` lists every legal action achieving `value`, ascending by cell.
fn solve(rules: &TicTacToeRules, state: &Board, memo: &mut HashMap<Board, Solved>) -> Solved {
    if let Some(cached) = memo.get(state) {
        return cached.clone();
    }
    let solved = if let Some(outcome) = rules.outcome(state) {
        match outcome {
            // The side to move at a won terminal is always the loser (the *previous* mover won).
            Outcome::Win(_) => Solved {
                value: -1,
                optimal: Vec::new(),
            },
            Outcome::Draw => Solved {
                value: 0,
                optimal: Vec::new(),
            },
        }
    } else {
        let mut best_value = i8::MIN;
        let mut child_values: Vec<(Move, i8)> = Vec::new();
        for action in rules.legal_actions(state) {
            let child_state = rules.apply(state, &action).expect("legal action must apply");
            let child = solve(rules, &child_state, memo);
            let value = -child.value;
            child_values.push((action, value));
            if value > best_value {
                best_value = value;
            }
        }
        let optimal = child_values
            .into_iter()
            .filter(|(_, v)| *v == best_value)
            .map(|(a, _)| a)
            .collect();
        Solved {
            value: best_value,
            optimal,
        }
    };
    memo.insert(*state, solved.clone());
    solved
}

/// Game-value label from the side-to-move's perspective and solver value.
fn value_label(to_move: Player, value: i8) -> &'static str {
    match (to_move, value) {
        (Player::X, 1) | (Player::O, -1) => "X wins",
        (Player::O, 1) | (Player::X, -1) => "O wins",
        (_, 0) => "draw",
        _ => unreachable!("solver value must be -1, 0, or 1"),
    }
}

/// 9-character `X`/`O`/`.` form of `board`, row-major, built from `cells()`.
fn board_string(board: &Board) -> String {
    board
        .cells()
        .iter()
        .map(|c| match c {
            Some(Player::X) => 'X',
            Some(Player::O) => 'O',
            None => '.',
        })
        .collect()
}

/// FNV-1a 64 over `bytes`.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// `p`-th percentile (0..=100) of an ascending-sorted slice, nearest-rank via rounding.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[derive(Serialize, Clone)]
struct TimingRow {
    subset: String,
    depth: u32,
    positions: usize,
    mean_us: f64,
    median_us: f64,
    p99_us: f64,
    total_ms: f64,
    none_results: usize,
}

/// Times one `search` call per position in `positions` at `depth`, returning the aggregate row
/// and the chosen cell (`None` iff terminal) for every position, in input order.
fn run_timing_pass(
    eval: &Evaluator,
    moves: &Moves,
    positions: &[Board],
    depth: u32,
    subset: &str,
) -> (TimingRow, Vec<Option<usize>>) {
    let mut micros: Vec<f64> = Vec::with_capacity(positions.len());
    let mut cells: Vec<Option<usize>> = Vec::with_capacity(positions.len());
    let mut none_results = 0usize;
    for p in positions {
        let start = Instant::now();
        let result = search(eval, moves, p, depth);
        let elapsed_us = start.elapsed().as_secs_f64() * 1_000_000.0;
        micros.push(elapsed_us);
        if result.is_none() {
            none_results += 1;
        }
        cells.push(result.map(|m| m.0));
    }
    let mut sorted = micros.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("timings are finite"));
    let n = micros.len();
    let mean_us = micros.iter().sum::<f64>() / n as f64;
    let median_us = percentile(&sorted, 50.0);
    let p99_us = percentile(&sorted, 99.0);
    let total_ms = micros.iter().sum::<f64>() / 1000.0;
    (
        TimingRow {
            subset: subset.to_string(),
            depth,
            positions: n,
            mean_us,
            median_us,
            p99_us,
            total_ms,
            none_results,
        },
        cells,
    )
}

#[derive(Serialize, Clone)]
struct Disagreement {
    position: String,
    to_move: String,
    game_player_cell: usize,
    solver_optimal: Vec<usize>,
    solver_value: i8,
}

/// For every non-terminal position, checks that the `search`-chosen cell (from a prior
/// `run_timing_pass` over the same `positions` at the depth being cross-checked) is among the
/// solver's optimal cells. Returns the disagreements, in position order.
fn cross_check(
    rules: &TicTacToeRules,
    positions: &[Board],
    cells: &[Option<usize>],
    memo: &HashMap<Board, Solved>,
) -> Vec<Disagreement> {
    let mut disagreements = Vec::new();
    for (p, cell) in positions.iter().zip(cells.iter()) {
        if rules.outcome(p).is_some() {
            continue;
        }
        let solved = &memo[p];
        let gp_cell = cell.expect("search must return an action for a non-terminal position");
        let agrees = solved.optimal.iter().any(|m| m.0 == gp_cell);
        if !agrees {
            disagreements.push(Disagreement {
                position: board_string(p),
                to_move: format!("{:?}", p.to_move()),
                game_player_cell: gp_cell,
                solver_optimal: solved.optimal.iter().map(|m| m.0).collect(),
                solver_value: solved.value,
            });
        }
    }
    disagreements
}

#[derive(Serialize, Clone)]
struct Header {
    date: String,
    git_commit: String,
    rustc: String,
    os: String,
    cpu: String,
    ram: String,
    build_profile: String,
    command: String,
}

fn build_header(args: &[String]) -> Header {
    let date = Utc::now().format("%Y-%m-%d").to_string();
    let git_commit = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("git rev-parse HEAD")
            .stdout,
    )
    .expect("git output is valid utf-8")
    .trim()
    .to_string();
    let rustc = String::from_utf8(
        Command::new("rustc")
            .arg("--version")
            .output()
            .expect("rustc --version")
            .stdout,
    )
    .expect("rustc output is valid utf-8")
    .trim()
    .to_string();
    let build_profile = if cfg!(debug_assertions) { "debug" } else { "release" }.to_string();
    let command = format!(
        "cargo run --release --manifest-path spikes/annotate-spike/Cargo.toml -- {}",
        args.join(" ")
    );
    Header {
        date,
        git_commit,
        rustc,
        os: "Windows 11 Pro 10.0.26200".to_string(),
        cpu: "AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)".to_string(),
        ram: "31 GiB".to_string(),
        build_profile,
        command,
    }
}

fn header_table(header: &Header) -> String {
    format!(
        "| field | value |\n\
         | --- | --- |\n\
         | date | {} |\n\
         | git_commit | {} |\n\
         | rustc | {} |\n\
         | os | {} |\n\
         | cpu | {} |\n\
         | ram | {} |\n\
         | build_profile | {} |\n\
         | command | {} |\n",
        header.date, header.git_commit, header.rustc, header.os, header.cpu, header.ram, header.build_profile, header.command,
    )
}

#[derive(Parser)]
struct Cli {
    /// Output directory; `benchmarks/` and `reproducibility/` subdirectories are created under it.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Primary search depth (also the determinism-pass depth). Depth 10 is always additionally
    /// timed and cross-checked since it is exhaustive from any position.
    #[arg(long, default_value_t = 9)]
    depth: u32,
}

fn main() {
    let cli = Cli::parse();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = cli
        .out
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts")));
    let depth = cli.depth;

    let header = build_header(&args);

    let rules = TicTacToeRules;
    let eval = Evaluator;
    let moves = Moves;

    let positions = rules.reachable_positions();
    assert_eq!(positions.len(), 5478, "expected 5478 reachable positions");

    let canonicalizer = TicTacToeCanonicalizer::new();
    let mut seen: HashSet<Board> = HashSet::new();
    let mut canonical: Vec<Board> = Vec::new();
    for p in &positions {
        let c = canonicalizer.canonicalize(p);
        if seen.insert(c) {
            canonical.push(c);
        }
    }
    assert_eq!(canonical.len(), 765, "expected 765 canonical forms");

    // --- Timing pass (step 3a) ---
    let (row_all_depth, cells_all_depth) = run_timing_pass(&eval, &moves, &positions, depth, "all_positions");
    let (row_canon_depth, _cells_canon_depth) = run_timing_pass(&eval, &moves, &canonical, depth, "canonical");
    let (row_all_10, cells_all_10) = run_timing_pass(&eval, &moves, &positions, 10, "all_positions");
    let (row_canon_10, _cells_canon_10) = run_timing_pass(&eval, &moves, &canonical, 10, "canonical");
    let timing_rows = vec![row_all_depth, row_canon_depth, row_all_10, row_canon_10];

    // --- Determinism (step 3b): two more full passes over all 5478 positions at `depth` ---
    let (_run1_row, run1_cells) = run_timing_pass(&eval, &moves, &positions, depth, "all_positions");
    let (_run2_row, run2_cells) = run_timing_pass(&eval, &moves, &positions, depth, "all_positions");
    let run1_json = serde_json::to_string(&run1_cells).expect("chosen-cell vector serializes");
    let run2_json = serde_json::to_string(&run2_cells).expect("chosen-cell vector serializes");
    let byte_identical = run1_json == run2_json;
    let fnv1 = fnv1a64(run1_json.as_bytes());
    let fnv2 = fnv1a64(run2_json.as_bytes());

    // --- Exhaustive solver (step 3c) ---
    let solve_start = Instant::now();
    let mut memo: HashMap<Board, Solved> = HashMap::new();
    solve(&rules, &rules.initial_state(), &mut memo);
    let solve_ms = solve_start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(memo.len(), 5478, "solver memo must cover all reachable positions");

    let labels = ["X wins", "draw", "O wins"];
    let mut dist_all: HashMap<&str, usize> = HashMap::new();
    let mut dist_nonterm: HashMap<&str, usize> = HashMap::new();
    for p in &positions {
        let solved = &memo[p];
        let label = value_label(p.to_move(), solved.value);
        *dist_all.entry(label).or_insert(0) += 1;
        if rules.outcome(p).is_none() {
            *dist_nonterm.entry(label).or_insert(0) += 1;
        }
    }

    // --- Cross-check (step 3d) ---
    let cross_9 = cross_check(&rules, &positions, &cells_all_depth, &memo);
    let cross_10 = cross_check(&rules, &positions, &cells_all_10, &memo);

    // --- Write annotation-per-position.md ---
    let mut md = String::new();
    let _ = write!(md, "## Header\n\n{}\n", header_table(&header));
    let _ = write!(
        md,
        "## Per-position search timing\n\n\
         | subset | depth | positions | mean_us | median_us | p99_us | total_ms | none_results |\n\
         | --- | --- | --- | --- | --- | --- | --- | --- |\n"
    );
    for row in &timing_rows {
        let _ = writeln!(
            md,
            "| {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {} |",
            row.subset, row.depth, row.positions, row.mean_us, row.median_us, row.p99_us, row.total_ms, row.none_results
        );
    }
    md.push('\n');
    let _ = write!(
        md,
        "## Capability findings\n\n\
         - value exposed: NO\n\
         - best-move set exposed: NO\n\
         - tie enumeration exposed: NO\n\n\
         `game_player::minimax::search` returns only `Option<S::Action>`; the position value, the \
         set of equal-best moves, and tie enumeration live on the private `Response` type and \
         inside the private `search_recursive` function, and ties among equal-value candidates are \
         resolved by stable sort order rather than exposed to callers.\n\n"
    );
    let _ = write!(
        md,
        "## Exhaustive solver\n\n\
         - positions solved: {}\n\
         - solve_ms: {:.1}\n\n\
         | label | all_positions | non_terminal |\n\
         | --- | --- | --- |\n",
        memo.len(),
        solve_ms
    );
    for label in labels {
        let _ = writeln!(
            md,
            "| {} | {} | {} |",
            label,
            dist_all.get(label).copied().unwrap_or(0),
            dist_nonterm.get(label).copied().unwrap_or(0)
        );
    }
    md.push('\n');
    let _ = write!(
        md,
        "## Cross-check\n\n\
         - depth {} disagreements: {}\n\
         - depth 10 disagreements: {}\n",
        depth,
        cross_9.len(),
        cross_10.len()
    );
    if !cross_9.is_empty() {
        let _ = write!(
            md,
            "\n| position | to_move | game_player_cell | solver_optimal | solver_value |\n\
             | --- | --- | --- | --- | --- |\n"
        );
        for d in &cross_9 {
            let optimal = d.solver_optimal.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",");
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} |",
                d.position, d.to_move, d.game_player_cell, optimal, d.solver_value
            );
        }
    }
    if !cross_10.is_empty() {
        let _ = write!(
            md,
            "\n| position | to_move | game_player_cell | solver_optimal | solver_value |\n\
             | --- | --- | --- | --- | --- |\n"
        );
        for d in &cross_10 {
            let optimal = d.solver_optimal.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",");
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} |",
                d.position, d.to_move, d.game_player_cell, optimal, d.solver_value
            );
        }
    }

    // --- Write annotation-per-position.json ---
    let value_distribution: Vec<serde_json::Value> = labels
        .iter()
        .map(|label| {
            json!({
                "label": label,
                "all_positions": dist_all.get(label).copied().unwrap_or(0),
                "non_terminal": dist_nonterm.get(label).copied().unwrap_or(0),
            })
        })
        .collect();
    let doc = json!({
        "header": header,
        "timing": timing_rows,
        "capabilities": {
            "value_exposed": false,
            "best_move_set_exposed": false,
            "tie_enumeration_exposed": false,
        },
        "solver": {
            "positions_solved": memo.len(),
            "solve_ms": solve_ms,
            "value_distribution": value_distribution,
        },
        "cross_check": {
            depth.to_string(): {
                "disagreements": cross_9.len(),
                "list": cross_9,
            },
            "10": {
                "disagreements": cross_10.len(),
                "list": cross_10,
            },
        },
    });

    // --- Write annotation-determinism.md ---
    let mut det_md = String::new();
    let _ = write!(det_md, "## Header\n\n{}\n", header_table(&header));
    let _ = write!(
        det_md,
        "## Result\n\n\
         - runs: 2\n\
         - positions: {}\n\
         - depth: {}\n\
         - byte-identical: {}\n\
         - fnv1a64 run1: 0x{:016x}\n\
         - fnv1a64 run2: 0x{:016x}\n",
        positions.len(),
        depth,
        if byte_identical { "YES" } else { "NO" },
        fnv1,
        fnv2,
    );

    let benchmarks_dir = out.join("benchmarks");
    let reproducibility_dir = out.join("reproducibility");
    fs::create_dir_all(&benchmarks_dir).expect("create benchmarks dir");
    fs::create_dir_all(&reproducibility_dir).expect("create reproducibility dir");
    fs::write(benchmarks_dir.join("annotation-per-position.md"), md).expect("write per-position md");
    fs::write(
        benchmarks_dir.join("annotation-per-position.json"),
        serde_json::to_string_pretty(&doc).expect("serialize per-position json"),
    )
    .expect("write per-position json");
    fs::write(reproducibility_dir.join("annotation-determinism.md"), det_md).expect("write determinism md");

    println!(
        "wrote annotation evidence to {} (depth {} disagreements: {}, depth 10 disagreements: {}, byte-identical: {})",
        out.display(),
        depth,
        cross_9.len(),
        cross_10.len(),
        byte_identical
    );
}
