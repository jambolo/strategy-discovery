//! Throwaway throughput spike: measures minimax self-play games/sec at depths {2,4,9},
//! serially and under rayon with {1,2,4,8,16} threads, and proves serial and parallel
//! results are byte-identical. Not part of the shipped pipeline.

use clap::Parser;
use game_player::StaticEvaluator;
use game_player::minimax::{ResponseGenerator, search};
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::Serialize;
use std::cell::Cell;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use strategy_discovery::core::traits::GameRules;
use strategy_discovery::games::tictactoe::{Board, LINES, Move, Outcome, Player, TicTacToeRules};

const DEPTHS: [u32; 3] = [2, 4, 9];
const THREAD_COUNTS: [usize; 5] = [1, 2, 4, 8, 16];

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts"))]
    out: PathBuf,
    #[arg(long, default_value_t = 200)]
    games: usize,
    #[arg(long, default_value_t = 3)]
    repeats: usize,
    #[arg(long, default_value_t = 20260820)]
    seed: u64,
}

/// Alice-perspective static evaluator: win values +100/-100; heuristic = lines still open
/// for X minus lines still open for O.
struct Evaluator;

impl StaticEvaluator for Evaluator {
    type State = Board;

    fn evaluate(&self, board: &Board) -> f32 {
        match board.winner() {
            Some(Player::X) => self.alice_wins_value(),
            Some(Player::O) => self.bob_wins_value(),
            None => {
                let mut value = 0.0;
                for line in LINES {
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

/// Wraps `TicTacToeRules::legal_actions`, counting every `generate` call in a per-game
/// `Cell<u64>` (game-player's search state carries `Rc`/`RefCell`, so one `Moves` per game).
struct Moves {
    rules: TicTacToeRules,
    calls: Cell<u64>,
}

impl Moves {
    fn new() -> Moves {
        Moves {
            rules: TicTacToeRules,
            calls: Cell::new(0),
        }
    }
}

impl ResponseGenerator for Moves {
    type State = Board;

    fn generate(&self, board: &Board, _depth: u32) -> Vec<Move> {
        self.calls.set(self.calls.get() + 1);
        self.rules.legal_actions(board)
    }
}

/// splitmix64, used to derive a per-game seed from the master seed and game index.
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[derive(Serialize, Clone)]
struct GameResult {
    game_index: u64,
    seed: u64,
    moves: Vec<usize>,
    outcome: Outcome,
    nodes: u64,
}

fn play_game(master_seed: u64, game_index: u64, depth: u32) -> GameResult {
    let seed = splitmix64(master_seed ^ (game_index.wrapping_mul(0x9E3779B97F4A7C15)));
    let rules = TicTacToeRules;
    let eval = Evaluator;
    let moves_gen = Moves::new();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut board = rules.initial_state();
    let mut moves: Vec<usize> = Vec::new();

    // First ply: uniform random opening for diversity.
    let legal = rules.legal_actions(&board);
    let first = *legal.choose(&mut rng).expect("initial position has legal actions");
    board = rules.apply(&board, &first).expect("first ply is legal");
    moves.push(first.0);

    // Every later ply: minimax search for the side to move.
    while rules.outcome(&board).is_none() {
        let action = search(&eval, &moves_gen, &board, depth).expect("non-terminal state has a move");
        board = rules.apply(&board, &action).expect("search returns a legal action");
        moves.push(action.0);
    }

    let outcome = rules.outcome(&board).expect("loop exits only on terminal states");
    GameResult {
        game_index,
        seed,
        moves,
        outcome,
        nodes: moves_gen.calls.get(),
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

fn run_serial(seed: u64, n: usize, depth: u32) -> Vec<GameResult> {
    (0..n as u64).map(|i| play_game(seed, i, depth)).collect()
}

fn run_parallel(seed: u64, n: usize, depth: u32, threads: usize) -> Vec<GameResult> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool builds");
    pool.install(|| (0..n as u64).into_par_iter().map(|i| play_game(seed, i, depth)).collect())
}

fn command_line(args: &Args) -> String {
    format!(
        "cargo run --release --manifest-path spikes/throughput-spike/Cargo.toml -- --out {} --games {} --repeats {} --seed {}",
        args.out.display(),
        args.games,
        args.repeats,
        args.seed
    )
}

fn header_table(git_commit: &str, rustc_version: &str, command: &str) -> String {
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let mut s = String::new();
    s.push_str("## Header\n\n");
    s.push_str("| field | value |\n");
    s.push_str("| --- | --- |\n");
    s.push_str(&format!("| date | {date} |\n"));
    s.push_str(&format!("| git_commit | {git_commit} |\n"));
    s.push_str(&format!("| rustc | {rustc_version} |\n"));
    s.push_str("| os | Windows 11 Pro 10.0.26200 |\n");
    s.push_str("| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |\n");
    s.push_str("| ram | 31 GiB |\n");
    s.push_str(&format!("| build_profile | {profile} |\n"));
    s.push_str(&format!("| command | {command} |\n"));
    s
}

#[derive(Serialize)]
struct HeaderJson {
    date: String,
    git_commit: String,
    rustc: String,
    os: String,
    cpu: String,
    ram: String,
    build_profile: String,
    command: String,
}

#[derive(Serialize)]
struct SettingsJson {
    games_per_configuration: usize,
    repeats: usize,
    master_seed: u64,
    depths: Vec<u32>,
}

#[derive(Serialize)]
struct ResultJson {
    depth: u32,
    mode: String,
    threads: Option<usize>,
    median_secs: f64,
    games_per_sec: f64,
    nodes_per_sec: f64,
    speedup: f64,
    repeats_secs: Vec<f64>,
}

#[derive(Serialize)]
struct OutcomeJson {
    depth: u32,
    plies_total: u64,
    x_wins: u64,
    o_wins: u64,
    draws: u64,
}

struct ModeResult {
    mode: String,
    threads: Option<usize>,
    repeats_secs: Vec<f64>,
    median_secs: f64,
    games_per_sec: f64,
    nodes_per_sec: f64,
}

fn time_repeats<F: Fn() -> Vec<GameResult>>(repeats: usize, run: F) -> (Vec<f64>, Vec<GameResult>) {
    let mut secs = Vec::with_capacity(repeats);
    let mut last = Vec::new();
    for _ in 0..repeats {
        let start = Instant::now();
        last = run();
        secs.push(start.elapsed().as_secs_f64());
    }
    (secs, last)
}

fn main() {
    let args = Args::parse();

    let git_commit = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("git rev-parse runs")
            .stdout,
    )
    .expect("git output is utf8")
    .trim()
    .to_string();
    let rustc_version = String::from_utf8(Command::new("rustc").arg("--version").output().expect("rustc runs").stdout)
        .expect("rustc output is utf8")
        .trim()
        .to_string();
    let command = command_line(&args);

    let bench_dir = args.out.join("benchmarks");
    let repro_dir = args.out.join("reproducibility");
    fs::create_dir_all(&bench_dir).expect("create benchmarks dir");
    fs::create_dir_all(&repro_dir).expect("create reproducibility dir");

    let mut all_results: Vec<ResultJson> = Vec::new();
    let mut outcomes: Vec<OutcomeJson> = Vec::new();
    let mut repro_lines: Vec<String> = Vec::new();
    let mut all_identical = true;

    let mut games_table = String::new();
    let mut speedup_table = String::new();
    let mut nodes_table = String::new();
    let mut outcomes_table = String::new();

    let col_header = "| depth | serial | threads_1 | threads_2 | threads_4 | threads_8 | threads_16 |\n| --- | --- | --- | --- | --- | --- | --- |\n";
    games_table.push_str(col_header);
    speedup_table.push_str(col_header);
    nodes_table.push_str(col_header);
    outcomes_table.push_str("| depth | plies_total | x_wins | o_wins | draws |\n| --- | --- | --- | --- | --- |\n");

    for &depth in DEPTHS.iter() {
        // Serial baseline.
        let (serial_secs, serial_results) = time_repeats(args.repeats, || run_serial(args.seed, args.games, depth));
        let serial_median = median(serial_secs.clone());
        let serial_total_nodes: u64 = serial_results.iter().map(|r| r.nodes).sum();
        let serial_games_per_sec = args.games as f64 / serial_median;
        let serial_nodes_per_sec = serial_total_nodes as f64 / serial_median;

        let serial_json = serde_json::to_string(&serial_results).expect("serial results serialize");
        let serial_hash = fnv1a64(serial_json.as_bytes());

        let mut modes: Vec<ModeResult> = Vec::new();
        modes.push(ModeResult {
            mode: "serial".to_string(),
            threads: None,
            repeats_secs: serial_secs,
            median_secs: serial_median,
            games_per_sec: serial_games_per_sec,
            nodes_per_sec: serial_nodes_per_sec,
        });

        for &t in THREAD_COUNTS.iter() {
            let (secs, results) = time_repeats(args.repeats, || run_parallel(args.seed, args.games, depth, t));
            let med = median(secs.clone());
            let total_nodes: u64 = results.iter().map(|r| r.nodes).sum();
            let games_per_sec = args.games as f64 / med;
            let nodes_per_sec = total_nodes as f64 / med;

            let json = serde_json::to_string(&results).expect("parallel results serialize");
            let hash = fnv1a64(json.as_bytes());
            let identical = json == serial_json;
            if !identical {
                all_identical = false;
            }
            repro_lines.push(format!(
                "- depth {depth} threads {t}: identical: {} serial 0x{serial_hash:016x} parallel 0x{hash:016x}",
                if identical { "YES" } else { "NO" }
            ));

            modes.push(ModeResult {
                mode: "parallel".to_string(),
                threads: Some(t),
                repeats_secs: secs,
                median_secs: med,
                games_per_sec,
                nodes_per_sec,
            });
        }

        // Tables: depth row across serial, threads_1..threads_16.
        let mut games_row = format!("| {depth} | {:.1}", serial_games_per_sec);
        let mut speedup_row = format!("| {depth} | 1.00");
        let mut nodes_row = format!("| {depth} | {}", serial_nodes_per_sec.round() as u64);
        for m in modes.iter().skip(1) {
            games_row.push_str(&format!(" | {:.1}", m.games_per_sec));
            speedup_row.push_str(&format!(" | {:.2}", m.games_per_sec / serial_games_per_sec));
            nodes_row.push_str(&format!(" | {}", m.nodes_per_sec.round() as u64));
        }
        games_row.push_str(" |\n");
        speedup_row.push_str(" |\n");
        nodes_row.push_str(" |\n");
        games_table.push_str(&games_row);
        speedup_table.push_str(&speedup_row);
        nodes_table.push_str(&nodes_row);

        for m in modes.iter() {
            all_results.push(ResultJson {
                depth,
                mode: m.mode.clone(),
                threads: m.threads,
                median_secs: m.median_secs,
                games_per_sec: m.games_per_sec,
                nodes_per_sec: m.nodes_per_sec,
                speedup: m.games_per_sec / serial_games_per_sec,
                repeats_secs: m.repeats_secs.clone(),
            });
        }

        let plies_total: u64 = serial_results.iter().map(|r| r.moves.len() as u64).sum();
        let x_wins = serial_results.iter().filter(|r| r.outcome == Outcome::Win(Player::X)).count() as u64;
        let o_wins = serial_results.iter().filter(|r| r.outcome == Outcome::Win(Player::O)).count() as u64;
        let draws = serial_results.iter().filter(|r| r.outcome == Outcome::Draw).count() as u64;
        outcomes_table.push_str(&format!("| {depth} | {plies_total} | {x_wins} | {o_wins} | {draws} |\n"));
        outcomes.push(OutcomeJson {
            depth,
            plies_total,
            x_wins,
            o_wins,
            draws,
        });
    }

    // selfplay-throughput.md
    let mut md = String::new();
    md.push_str(&header_table(&git_commit, &rustc_version, &command));
    md.push('\n');
    md.push_str("## Settings\n\n");
    md.push_str(&format!("- games per configuration: {}\n", args.games));
    md.push_str(&format!("- repeats: {} (median reported)\n", args.repeats));
    md.push_str(&format!("- master seed: {}\n", args.seed));
    md.push_str("- depths: 2, 4, 9\n");
    md.push_str("- first ply: seeded uniform random; later plies: minimax search both sides\n");
    md.push('\n');
    md.push_str("## Games per second\n\n");
    md.push_str(&games_table);
    md.push('\n');
    md.push_str("## Speedup vs serial\n\n");
    md.push_str(&speedup_table);
    md.push('\n');
    md.push_str("## Nodes per second\n\n");
    md.push_str("nodes = `ResponseGenerator::generate` calls.\n\n");
    md.push_str(&nodes_table);
    md.push('\n');
    md.push_str("## Outcomes\n\n");
    md.push_str(&outcomes_table);

    fs::write(bench_dir.join("selfplay-throughput.md"), md).expect("write selfplay-throughput.md");

    // selfplay-throughput.json
    let header_json = HeaderJson {
        date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        git_commit: git_commit.clone(),
        rustc: rustc_version.clone(),
        os: "Windows 11 Pro 10.0.26200".to_string(),
        cpu: "AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)".to_string(),
        ram: "31 GiB".to_string(),
        build_profile: if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "release".to_string()
        },
        command: command.clone(),
    };
    let settings_json = SettingsJson {
        games_per_configuration: args.games,
        repeats: args.repeats,
        master_seed: args.seed,
        depths: DEPTHS.to_vec(),
    };

    #[derive(Serialize)]
    struct ThroughputJson {
        header: HeaderJson,
        settings: SettingsJson,
        results: Vec<ResultJson>,
        outcomes: Vec<OutcomeJson>,
    }
    let throughput_json = ThroughputJson {
        header: header_json,
        settings: settings_json,
        results: all_results,
        outcomes,
    };
    fs::write(
        bench_dir.join("selfplay-throughput.json"),
        serde_json::to_string_pretty(&throughput_json).expect("serialize throughput json"),
    )
    .expect("write selfplay-throughput.json");

    // selfplay-serial-vs-parallel.md
    let mut repro_md = String::new();
    repro_md.push_str(&header_table(&git_commit, &rustc_version, &command));
    repro_md.push('\n');
    repro_md.push_str("## Result\n\n");
    for line in &repro_lines {
        repro_md.push_str(line);
        repro_md.push('\n');
    }
    repro_md.push_str(&format!("- all identical: {}\n", if all_identical { "YES" } else { "NO" }));
    fs::write(repro_dir.join("selfplay-serial-vs-parallel.md"), repro_md).expect("write selfplay-serial-vs-parallel.md");

    std::io::stdout().flush().ok();
}
