//! End-to-end CLI tests for the `pipeline` subcommand: a full experiment run through all four
//! stages, `--stages` subset selection and its precondition errors, unknown-stage rejection, and
//! `--strict` propagating to exit code 3.

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
fn pipeline_runs_every_stage() {
    let dir = out_dir("pipeline_runs_every_stage");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, stdout, stderr) = cli(&[
        "pipeline",
        "--experiment",
        "tests/fixtures/experiment-small.toml",
        "--out",
        dir_str,
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.iter().any(|l| l.starts_with("run_id=5eafa65f76637df3 ")),
        "stdout: {stdout}"
    );
    assert!(
        lines.iter().any(|l| l.starts_with("mode=corpus annotated=390 ")),
        "stdout: {stdout}"
    );
    assert!(lines.iter().any(|l| l.starts_with("games=144 ")), "stdout: {stdout}");
    assert!(lines.iter().any(|l| l.starts_with("report=")), "stdout: {stdout}");
    let last = lines.last().expect("at least one stdout line");
    assert!(last.starts_with("pipeline=experiment-small out="), "last line: {last}");
    assert!(
        last.ends_with(" stages=generate,annotate,analyze,report"),
        "last line: {last}"
    );

    for name in [
        "run.json",
        "games.jsonl",
        "positions.jsonl",
        "annotations.jsonl",
        "annotate.json",
        "summary.json",
        "agreement.json",
        "analyze.json",
        "report.md",
    ] {
        assert!(dir.join(name).is_file(), "missing {name}");
    }

    let report = std::fs::read_to_string(dir.join("report.md")).expect("report.md reads");
    assert!(report.contains("# Run 5eafa65f76637df3"), "report: {report}");
    assert!(report.contains("## Summary"), "report: {report}");
    assert!(report.contains("## Agreement"), "report: {report}");
}

#[test]
fn pipeline_stage_subset_requires_its_inputs() {
    let dir = out_dir("pipeline_stage_subset_requires_its_inputs");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&[
        "pipeline",
        "--experiment",
        "tests/fixtures/experiment-small.toml",
        "--out",
        dir_str,
        "--stages",
        "analyze",
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("run.json"), "stderr: {stderr}");
    assert!(stderr.contains("generate"), "stderr: {stderr}");
}

#[test]
fn pipeline_rejects_an_unknown_stage() {
    let dir = out_dir("pipeline_rejects_an_unknown_stage");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&[
        "pipeline",
        "--experiment",
        "tests/fixtures/experiment-small.toml",
        "--out",
        dir_str,
        "--stages",
        "nope",
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("known stages"), "stderr: {stderr}");
}

#[test]
fn pipeline_strict_failure_exits_three() {
    let dir = out_dir("pipeline_strict_failure_exits_three");

    let sweep = std::env::current_dir().unwrap().join("tests/fixtures/generate-draws.toml");
    let sweep_str = sweep.to_str().expect("cwd path is utf-8");

    let experiment_path = dir.join("draws-experiment.toml");
    std::fs::write(
        &experiment_path,
        format!(
            r#"schema_version = 1
name = "draws"
game = "tictactoe"
out = "runs/draws"

[generate]
sweep = '{sweep_str}'

[annotate]
enabled = false

[analyze]
analyzers = ["summary"]
strict = true
"#
        ),
    )
    .unwrap();
    let experiment_str = experiment_path.to_str().expect("temp path is utf-8");

    let run_out = dir.join("run");
    let run_out_str = run_out.to_str().expect("temp path is utf-8");

    let (code, stdout, stderr) = cli(&[
        "pipeline",
        "--experiment",
        experiment_str,
        "--out",
        run_out_str,
        "--stages",
        "generate,analyze",
    ]);
    assert_eq!(code, 3, "stderr: {stderr}");
    assert!(stdout.contains("diversity_pass=false"), "stdout: {stdout}");
    assert!(
        stderr.contains("check failed: diversity thresholds not met"),
        "stderr: {stderr}"
    );
}
