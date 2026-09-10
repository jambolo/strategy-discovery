//! CLI-level coverage of the `agreement` analyzer: run over a corpus through
//! `generate -> annotate -> analyze`, `--list-analyzers`, and the missing-annotations error path.

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
fn agreement_analyzer_runs_over_a_corpus() {
    let dir = out_dir("agreement_analyzer_runs_over_a_corpus");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0);

    let (code, _stdout, _stderr) = cli(&["annotate", "--corpus", dir_str]);
    assert_eq!(code, 0);

    let (code, stdout, _stderr) = cli(&["analyze", "--corpus", dir_str, "--analyzers", "summary,agreement"]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.starts_with("games=144 "), "stdout: {stdout}");

    assert!(dir.join("agreement.json").is_file());
    assert!(dir.join("analyze.json").is_file());

    let analyze_text = std::fs::read_to_string(dir.join("analyze.json")).unwrap();
    assert!(
        analyze_text.contains("\"name\": \"agreement\""),
        "analyze.json: {analyze_text}"
    );

    let agreement_text = std::fs::read_to_string(dir.join("agreement.json")).unwrap();
    let agreement: serde_json::Value = serde_json::from_str(&agreement_text).unwrap();
    assert_eq!(agreement["by_strategy"]["perfect"]["rate"], 1.0);
    assert!(agreement["by_strategy"]["random"]["rate"].as_f64().unwrap() < 1.0);
    assert_eq!(agreement["overall"]["positions"].as_u64(), Some(1083));
}

#[test]
fn list_analyzers_lists_both_builtins() {
    let (code, stdout, stderr) = cli(&["analyze", "--list-analyzers"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let agreement_lines = stdout.lines().filter(|l| l.starts_with("agreement")).count();
    let summary_lines = stdout.lines().filter(|l| l.starts_with("summary")).count();
    assert_eq!(agreement_lines, 1, "stdout: {stdout}");
    assert_eq!(summary_lines, 1, "stdout: {stdout}");
}

#[test]
fn agreement_without_annotations_is_rejected() {
    let dir = out_dir("agreement_without_annotations_is_rejected");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-draws.toml", "--out", dir_str]);
    assert_eq!(code, 0);

    let (code, _stdout, stderr) = cli(&["analyze", "--corpus", dir_str, "--analyzers", "agreement"]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("annotations.jsonl"), "stderr: {stderr}");
}
