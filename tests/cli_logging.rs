//! Logging controls are stderr-only: verbosity and `RUST_LOG` never change stdout or a
//! persisted file's bytes.

use std::path::{Path, PathBuf};

/// Fresh, empty output directory for one test, under the integration-test temp dir.
fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the built binary with `args` and `env` (after clearing any inherited `RUST_LOG`),
/// returning `(exit_code, stdout, stderr)`.
fn cli_env(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_strategy-discovery"));
    command.args(args).env_remove("RUST_LOG");
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command.output().expect("binary runs");
    (
        output.status.code().expect("process exited normally"),
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        String::from_utf8(output.stderr).expect("stderr is utf-8"),
    )
}

#[test]
fn verbose_adds_stderr_and_keeps_stdout_identical() {
    let dir = out_dir("verbose_adds_stderr_and_keeps_stdout_identical");
    let dir_str = dir.to_str().expect("temp path is utf-8");
    let args = ["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str];

    let (code_plain, stdout_plain, stderr_plain) = cli_env(&args, &[]);
    assert_eq!(code_plain, 0);
    assert!(stderr_plain.is_empty(), "stderr: {stderr_plain}");
    let run_json_after_first = std::fs::read(dir.join("run.json")).unwrap();

    let verbose_args = [
        "-v",
        "generate",
        "--config",
        "tests/fixtures/generate-small.toml",
        "--out",
        dir_str,
    ];
    let (code_verbose, stdout_verbose, stderr_verbose) = cli_env(&verbose_args, &[]);
    assert_eq!(code_verbose, 0);
    assert_eq!(stdout_plain, stdout_verbose);
    assert!(stderr_verbose.contains("INFO"), "stderr: {stderr_verbose}");
    assert!(stderr_verbose.contains("cell played"), "stderr: {stderr_verbose}");
    let run_json_after_second = std::fs::read(dir.join("run.json")).unwrap();
    assert_eq!(run_json_after_first, run_json_after_second);
}

#[test]
fn rust_log_off_silences_verbose() {
    let dir = out_dir("rust_log_off_silences_verbose");
    let dir_str = dir.to_str().expect("temp path is utf-8");
    let args = [
        "-v",
        "generate",
        "--config",
        "tests/fixtures/generate-small.toml",
        "--out",
        dir_str,
    ];

    let (code, stdout, stderr) = cli_env(&args, &[("RUST_LOG", "off")]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(stdout.starts_with("run_id=5eafa65f76637df3"), "stdout: {stdout}");
}

#[test]
fn quiet_silences_and_conflicts_with_verbose() {
    let dir = out_dir("quiet_silences_and_conflicts_with_verbose");
    let dir_str = dir.to_str().expect("temp path is utf-8");
    let args = [
        "-q",
        "generate",
        "--config",
        "tests/fixtures/generate-small.toml",
        "--out",
        dir_str,
    ];

    let (code, _stdout, stderr) = cli_env(&args, &[]);
    assert_eq!(code, 0);
    assert!(stderr.is_empty(), "stderr: {stderr}");

    let conflicting_args = [
        "-q",
        "-v",
        "generate",
        "--config",
        "tests/fixtures/generate-small.toml",
        "--out",
        dir_str,
    ];
    let (code, _stdout, _stderr) = cli_env(&conflicting_args, &[]);
    assert_eq!(code, 2);
}

#[test]
fn trace_level_reaches_minimax() {
    let dir = out_dir("trace_level_reaches_minimax");
    let dir_str = dir.to_str().expect("temp path is utf-8");
    let args = [
        "-vvv",
        "generate",
        "--config",
        "tests/fixtures/generate-draws.toml",
        "--out",
        dir_str,
    ];

    let (code, stdout, stderr) = cli_env(&args, &[]);
    assert_eq!(code, 0);
    assert_eq!(
        stdout,
        format!("run_id=933a94e742497e81 cells=1 games=16 positions=144 out={dir_str}\n")
    );
    assert!(stderr.contains("TRACE"), "stderr: {stderr}");
    assert!(stderr.contains("minimax"), "stderr: {stderr}");
}
