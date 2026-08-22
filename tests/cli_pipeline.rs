//! End-to-end CLI tests: spawn the built binary and drive `generate -> annotate -> analyze`,
//! the no-subcommand banner, and the exit-code taxonomy (0 success, 1 runtime failure,
//! 2 usage/input error, 3 a requested check failed — see `cli::error`).

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
fn pipeline_generate_annotate_analyze() {
    let dir = out_dir("pipeline_generate_annotate_analyze");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("run_id="), "stdout: {stdout}");
    assert!(stdout.contains("cells=36"), "stdout: {stdout}");
    assert!(stdout.contains("games=144"), "stdout: {stdout}");
    assert!(dir.join("run.json").exists());
    assert!(dir.join("games.jsonl").exists());
    assert!(dir.join("positions.jsonl").exists());

    let (code, stdout, _stderr) = cli(&["annotate", "--corpus", dir_str]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("mode=corpus "), "stdout: {stdout}");
    assert!(stdout.contains("disagreements=0"), "stdout: {stdout}");
    assert!(dir.join("annotations.jsonl").exists());
    assert!(dir.join("annotate.json").exists());

    let (code, stdout, _stderr) = cli(&["analyze", "--corpus", dir_str]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("games=144 "), "stdout: {stdout}");
    assert!(stdout.contains("diversity_pass="), "stdout: {stdout}");
    assert!(dir.join("summary.json").exists());
}

#[test]
fn strict_analyze_exits_three_on_undiverse_corpus() {
    let dir = out_dir("strict_analyze_exits_three_on_undiverse_corpus");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-draws.toml", "--out", dir_str]);
    assert_eq!(code, 0);

    let (code, stdout, stderr) = cli(&["analyze", "--corpus", dir_str, "--strict"]);
    assert_eq!(code, 3);
    assert!(stdout.contains("diversity_pass=false"), "stdout: {stdout}");
    assert!(stderr.contains("decisive_fraction 0.00 < 0.20"), "stderr: {stderr}");
}

#[test]
fn exhaustive_annotation_runs_from_the_cli() {
    let dir = out_dir("exhaustive_annotation_runs_from_the_cli");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, stdout, _stderr) = cli(&["annotate", "--exhaustive", "--game", "tictactoe", "--out", dir_str]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("mode=exhaustive "), "stdout: {stdout}");
    assert!(stdout.contains("annotated=5478"), "stdout: {stdout}");
    assert!(stdout.contains("disagreements=0"), "stdout: {stdout}");
    assert!(dir.join("annotations.jsonl").exists());
    assert!(dir.join("annotate.json").exists());
}

#[test]
fn no_subcommand_prints_name_and_version() {
    let (code, stdout, _stderr) = cli(&[]);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "strategy-discovery 0.1.0");
}

#[test]
fn missing_config_file_is_an_error() {
    let dir = out_dir("missing_config_file_is_an_error");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&["generate", "--config", "tests/fixtures/does-not-exist.toml", "--out", dir_str]);
    assert_eq!(code, 2);
    assert!(stderr.contains("error:"), "stderr: {stderr}");
    assert!(stderr.contains("does-not-exist.toml"), "stderr: {stderr}");
}

#[test]
fn unknown_game_is_rejected() {
    let dir = out_dir("unknown_game_is_rejected");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&["annotate", "--exhaustive", "--game", "chess", "--out", dir_str]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown game"), "stderr: {stderr}");
}

#[test]
fn malformed_toml_is_rejected() {
    let dir = out_dir("malformed_toml_is_rejected");

    let config_path = dir.join("bad.toml");
    std::fs::write(&config_path, "schema_version = 1\ngame = \"tictactoe\"\nseed = [\n").unwrap();
    let config_str = config_path.to_str().expect("temp path is utf-8");
    let out_path = dir.join("run");
    let out_str = out_path.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&["generate", "--config", config_str, "--out", out_str]);
    assert_eq!(code, 2);
    assert!(stderr.contains("bad.toml"), "stderr: {stderr}");
}

#[test]
fn analyze_missing_corpus_is_rejected() {
    let dir = out_dir("analyze_missing_corpus_is_rejected");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&["analyze", "--corpus", dir_str]);
    assert_eq!(code, 2);
    assert!(stderr.contains("run.json"), "stderr: {stderr}");
}
