//! End-to-end CLI tests for `play`: a transcript per game, byte-identity with `generate` on
//! the equivalent sweep TOML when `--out` is given, and the `--players` validation errors.

use std::path::{Path, PathBuf};

/// Fresh, empty output directory for one test, under the integration-test temp dir.
fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the built binary with `args` and returns `(exit_code, stdout, stderr)`.
fn cli(args: &[&str]) -> (i32, String, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_strategy-discovery"))
        .args(args)
        .output()
        .expect("binary runs");
    (
        output.status.code().expect("process exited normally"),
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        String::from_utf8(output.stderr).expect("stderr is utf-8"),
    )
}

#[test]
fn play_prints_a_transcript_per_game() {
    let (code, stdout, _stderr) = cli(&[
        "play",
        "--game",
        "tictactoe",
        "--players",
        "random,perfect",
        "--games",
        "2",
        "--seed",
        "7",
    ]);
    assert_eq!(code, 0, "stdout: {stdout}");

    let game_lines: Vec<&str> = stdout.lines().filter(|line| line.starts_with("game ")).collect();
    assert_eq!(game_lines.len(), 2, "stdout: {stdout}");

    assert_eq!(stdout.matches(" (opening)").count(), 0, "stdout: {stdout}");

    let outcome_lines: Vec<&str> = stdout.lines().filter(|line| line.starts_with("outcome=")).collect();
    assert_eq!(outcome_lines.len(), 2, "stdout: {stdout}");

    let last_non_empty = stdout.lines().rev().find(|line| !line.is_empty()).expect("non-empty line");
    assert!(last_non_empty.starts_with("games=2 wins="), "last line: {last_non_empty}");
    assert!(last_non_empty.contains(" draws="), "last line: {last_non_empty}");
    assert!(last_non_empty.contains(" unfinished=0"), "last line: {last_non_empty}");
}

#[test]
fn play_out_matches_generate_bytes() {
    let dir = out_dir("play_out_matches_generate_bytes");
    let config_path = dir.join("equivalent.toml");
    std::fs::write(
        &config_path,
        r#"
schema_version = 1
game = "tictactoe"
seed = 7
games_per_cell = 2
evaluators = ["default"]
pairings = [["random", "perfect"]]
random_opening_plies = [0]

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"

[[strategies]]
name = "perfect"
[strategies.spec]
kind = "minimax"
depth = 9

[[openings]]
name = "play"
"#,
    )
    .unwrap();

    let gen_dir = dir.join("gen");
    let play_dir = dir.join("play");
    let config_str = config_path.to_str().expect("temp path is utf-8");
    let gen_dir_str = gen_dir.to_str().expect("temp path is utf-8");
    let play_dir_str = play_dir.to_str().expect("temp path is utf-8");

    let (code, gen_stdout, gen_stderr) = cli(&["generate", "--config", config_str, "--out", gen_dir_str]);
    assert_eq!(code, 0, "stderr: {gen_stderr}");

    let (code, _play_stdout, play_stderr) = cli(&[
        "play",
        "--game",
        "tictactoe",
        "--players",
        "random,perfect",
        "--games",
        "2",
        "--seed",
        "7",
        "--out",
        play_dir_str,
    ]);
    assert_eq!(code, 0, "stderr: {play_stderr}");

    for file in ["run.json", "games.jsonl", "positions.jsonl"] {
        let gen_bytes = std::fs::read(gen_dir.join(file)).unwrap();
        let play_bytes = std::fs::read(play_dir.join(file)).unwrap();
        assert_eq!(gen_bytes, play_bytes, "{file} differs");
    }

    let run_id = gen_stdout
        .split_whitespace()
        .find_map(|token| token.strip_prefix("run_id="))
        .expect("generate stdout has run_id=");
    let play_run_json = std::fs::read_to_string(play_dir.join("run.json")).unwrap();
    assert!(play_run_json.contains(run_id), "run.json: {play_run_json}");
}

#[test]
fn play_rejects_a_bad_player_count() {
    let (code, _stdout, stderr) = cli(&["play", "--game", "tictactoe", "--players", "random"]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("--players must name 2 strategies"), "stderr: {stderr}");
}

#[test]
fn play_rejects_an_unknown_strategy() {
    let (code, _stdout, stderr) = cli(&["play", "--game", "tictactoe", "--players", "random,nope"]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("unknown strategy `nope`"), "stderr: {stderr}");
}
