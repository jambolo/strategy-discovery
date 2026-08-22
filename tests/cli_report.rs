//! End-to-end CLI tests for `report`: rendering a whole run directory and a single analyzer
//! output file, byte-identity across repeated runs and `--out`, and its error paths.

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

/// Every line starting with `#` is immediately followed by a blank line.
fn headings_have_blank_line_after(text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with('#') {
            assert_eq!(lines.get(i + 1), Some(&""), "heading {line:?} not followed by a blank line");
        }
    }
}

#[test]
fn report_renders_a_run_directory() {
    let dir = out_dir("report_renders_a_run_directory");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0);

    let (code, _stdout, _stderr) = cli(&["annotate", "--corpus", dir_str]);
    assert_eq!(code, 0);

    let (code, _stdout, _stderr) = cli(&["analyze", "--corpus", dir_str, "--analyzers", "summary,agreement"]);
    assert_eq!(code, 0);

    let (code, stdout, _stderr) = cli(&["report", "--input", dir_str]);
    assert_eq!(code, 0, "stdout: {stdout}");

    for needle in ["# Run 5eafa65f76637df3", "## Annotation", "## Summary", "## Agreement"] {
        assert_eq!(
            stdout.matches(needle).count(),
            1,
            "expected {needle:?} exactly once in:\n{stdout}"
        );
    }
    assert!(stdout.contains("| games | 144 |"), "stdout: {stdout}");
    assert!(stdout.contains("| positions | 1083 |"), "stdout: {stdout}");
    assert!(!stdout.contains("|---"), "stdout: {stdout}");
    headings_have_blank_line_after(&stdout);
    assert!(stdout.ends_with('\n'));
    assert!(!stdout.ends_with("\n\n"));

    let (code, stdout_again, _stderr) = cli(&["report", "--input", dir_str]);
    assert_eq!(code, 0);
    assert_eq!(stdout, stdout_again, "repeated report runs must be byte-identical");

    let out_path = dir.join("report.md");
    let out_path_str = out_path.to_str().expect("temp path is utf-8");
    let (code, _stdout, _stderr) = cli(&["report", "--input", dir_str, "--out", out_path_str]);
    assert_eq!(code, 0);
    let written = std::fs::read_to_string(&out_path).unwrap();
    assert_eq!(written, stdout, "--out file must be byte-identical to stdout");

    let summary_path = dir.join("summary.json");
    let summary_path_str = summary_path.to_str().expect("temp path is utf-8");
    let (code, stdout, _stderr) = cli(&["report", "--input", summary_path_str]);
    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.starts_with("## Summary"), "stdout: {stdout}");
    assert!(!stdout.contains("# Run "), "stdout: {stdout}");
    assert!(!stdout.contains("## Agreement"), "stdout: {stdout}");

    let (code, _stdout, _stderr) = cli(&["report", "--input", dir.join("nope.json").to_str().unwrap()]);
    assert_eq!(code, 2);
}

#[test]
fn report_missing_input_is_rejected() {
    let dir = out_dir("report_missing_input_is_rejected");
    let missing = dir.join("does-not-exist");

    let (code, _stdout, stderr) = cli(&["report", "--input", missing.to_str().unwrap()]);
    assert_eq!(code, 2, "stderr: {stderr}");
}
