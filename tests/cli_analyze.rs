//! CLI tests for the analyzer-registry-backed `analyze` subcommand: `--list-analyzers`,
//! running named analyzers, rejecting an unknown analyzer, and requiring `--corpus`.

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
fn lists_registered_analyzers() {
    let (code, stdout, _stderr) = cli(&["analyze", "--list-analyzers"]);
    assert_eq!(code, 0);

    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(!lines.is_empty(), "stdout: {stdout}");
    for line in &lines {
        assert!(line.contains('\t'), "line missing tab: {line}");
    }
    assert!(lines.iter().any(|l| l.starts_with("summary\t")), "stdout: {stdout}");
}

#[test]
fn runs_named_analyzers_and_rejects_unknown() {
    let dir = out_dir("cli_analyze_runs_named_analyzers");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0);

    let (code, stdout, _stderr) = cli(&["analyze", "--corpus", dir_str, "--analyzers", "summary"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("games=144 "), "stdout: {stdout}");
    assert!(stdout.contains("diversity_pass="), "stdout: {stdout}");
    assert!(dir.join("summary.json").is_file());
    assert!(dir.join("analyze.json").is_file());
    let analyze_json = std::fs::read_to_string(dir.join("analyze.json")).unwrap();
    assert!(analyze_json.contains("\"name\": \"summary\""), "analyze.json: {analyze_json}");

    let (code, _stdout, stderr) = cli(&["analyze", "--corpus", dir_str, "--analyzers", "nope"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unknown analyzer `nope`"), "stderr: {stderr}");
}

#[test]
fn analyze_without_corpus_is_rejected() {
    let (code, _stdout, stderr) = cli(&["analyze"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("--corpus"), "stderr: {stderr}");
}
