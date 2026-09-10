//! End-to-end tests for the `evaluate` subcommand at test scale (tiny roster, `--games 2`,
//! corpus-sized annotations): result files and stdout, byte-identity across repeat runs,
//! execution modes and verbosity, archive idempotence, the strict check, the
//! `heuristic-rules`-is-an-error rule, and the usage exit code.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

/// Fresh, empty directory for one test, under the integration-test temp dir.
fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli_evaluate").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs the built binary with `args` and returns `(exit_code, stdout, stderr)`.
fn cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::cargo_bin("strategy-discovery")
        .expect("binary is built")
        .args(args)
        .output()
        .expect("binary runs");
    (
        output.status.code().expect("process exited normally"),
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        String::from_utf8(output.stderr).expect("stderr is utf-8"),
    )
}

/// Generates the `generate-small` corpus into a fresh dir and annotates it (390 records).
fn corpus_annotations(name: &str) -> PathBuf {
    let dir = out_dir(name);
    let dir_str = dir.to_str().unwrap();
    let (code, stdout, stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0, "generate stdout: {stdout} stderr: {stderr}");
    let (code, stdout, stderr) = cli(&["annotate", "--corpus", dir_str]);
    assert_eq!(code, 0, "annotate stdout: {stdout} stderr: {stderr}");
    assert!(stdout.starts_with("mode=corpus annotated=390 "), "stdout: {stdout}");
    dir
}

/// The bytes of `evaluation.json`, `archive/archive.json`, `archive/entries.jsonl` under `out`.
fn result_files(out: &Path) -> [Vec<u8>; 3] {
    [
        std::fs::read(out.join("evaluation.json")).expect("evaluation.json"),
        std::fs::read(out.join("archive").join("archive.json")).expect("archive.json"),
        std::fs::read(out.join("archive").join("entries.jsonl")).expect("entries.jsonl"),
    ]
}

/// `stdout` with the `out=...` echo of the user-supplied `--out` path removed from its last line.
fn without_out_path(stdout: &str) -> String {
    stdout
        .lines()
        .map(|line| match line.find(" out=") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The `key=` value on the first stdout line starting with `prefix`.
fn field<'a>(stdout: &'a str, prefix: &str, key: &str) -> &'a str {
    let line = stdout
        .lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no `{prefix}` line in: {stdout}"));
    line.split(' ')
        .find_map(|token| token.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("no `{key}=` on line: {line}"))
}

/// Runs `evaluate` for `strategies-eval.toml` against the tiny roster with `--games 2` and the
/// corpus annotations at `annotations`, into `out`, with `extra` flags (global flags go first).
fn evaluate(out: &Path, annotations: &Path, globals: &[&str], extra: &[&str]) -> (i32, String, String) {
    let mut args: Vec<&str> = globals.to_vec();
    args.extend_from_slice(&[
        "evaluate",
        "--game",
        "tictactoe",
        "--strategies",
        "tests/fixtures/strategies-eval.toml",
        "--roster",
        "tests/fixtures/roster-tiny.toml",
        "--games",
        "2",
        "--annotations",
        annotations.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    args.extend_from_slice(extra);
    cli(&args)
}

#[test]
fn evaluate_writes_results_and_is_byte_identical() {
    let corpus = corpus_annotations("corpus");
    let out_a = out_dir("eval-a");

    let (code, stdout_a, stderr) = evaluate(&out_a, &corpus, &[], &[]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        stdout_a.lines().filter(|l| l.starts_with("strategy=")).count(),
        3,
        "stdout: {stdout_a}"
    );
    assert_eq!(
        stdout_a.lines().filter(|l| l.starts_with("evaluation=")).count(),
        1,
        "stdout: {stdout_a}"
    );
    assert_eq!(stdout_a.lines().count(), 4, "stdout: {stdout_a}");

    let perfect = stdout_a.lines().find(|l| l.starts_with("strategy=perfect ")).unwrap();
    assert!(perfect.contains(" kind=minimax "), "{perfect}");
    assert!(perfect.contains(" games=12 "), "{perfect}");
    assert!(perfect.contains(" losses=0 "), "{perfect}");
    assert!(perfect.contains(" loss_rate_vs_reference=0.000 "), "{perfect}");
    assert!(perfect.contains(" agreement=1.000 "), "{perfect}");
    let random_loss: f64 = field(&stdout_a, "strategy=random ", "loss_rate_vs_reference")
        .parse()
        .unwrap();
    assert!(random_loss > 0.0, "stdout: {stdout_a}");
    assert_eq!(field(&stdout_a, "evaluation=", "roster"), "tiny-v1");
    assert_eq!(field(&stdout_a, "evaluation=", "reference"), "perfect");
    assert_eq!(field(&stdout_a, "evaluation=", "strategies"), "3");
    assert_eq!(field(&stdout_a, "evaluation=", "archive_entries"), "3");
    assert_eq!(field(&stdout_a, "evaluation=", "out"), out_a.to_str().unwrap());

    let files_a = result_files(&out_a);
    let report: Value = serde_json::from_slice(&files_a[0]).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["game"], "tictactoe");
    assert_eq!(report["roster"]["name"], "tiny");
    assert_eq!(report["roster"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(report["config"]["games_per_pairing"], 2);
    assert_eq!(report["config"]["annotations"]["records"], 390);
    let strategies = report["strategies"].as_array().unwrap();
    assert_eq!(strategies.len(), 3);
    assert_eq!(strategies[0]["name"], "perfect");
    let opponents = strategies[0]["tournament"]["opponents"].as_array().unwrap();
    assert_eq!(opponents.len(), 3);
    for opponent in opponents {
        assert_eq!(opponent["loss_rate"], 0.0, "perfect vs {}", opponent["opponent"]);
        assert_eq!(opponent["by_seat"].as_array().unwrap().len(), 2);
    }
    assert_eq!(strategies[0]["headline"]["loss_rate_vs_reference"], 0.0);
    assert_eq!(strategies[0]["agreement"]["rate"], 1.0);
    assert_eq!(strategies[0]["agreement"]["positions"], 390);
    assert_eq!(strategies[1]["name"], "random");
    assert!(strategies[1]["headline"]["loss_rate_vs_reference"].as_f64().unwrap() > 0.0);
    assert!(strategies[1]["agreement"]["rate"].as_f64().unwrap() < 1.0);

    let index: Value = serde_json::from_slice(&files_a[1]).unwrap();
    assert_eq!(index["game"], "tictactoe");
    assert_eq!(index["entries"].as_array().unwrap().len(), 3);
    let entries: Vec<Value> = std::str::from_utf8(&files_a[2])
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(entries.len(), 3);
    for (sequence, entry) in entries.iter().enumerate() {
        assert_eq!(entry["sequence"], sequence as u64);
        assert_eq!(entry["provenance"]["source"], "evaluate");
        assert_eq!(entry["provenance"]["roster_id"], "tiny-v1");
        assert_eq!(entry["provenance"]["evaluation_id"], report["evaluation_id"]);
        assert_eq!(entry["provenance"]["annotations_mode"], "corpus");
        assert_eq!(entry["novelty"]["method"], "m1-v1");
    }
    assert_eq!(entries[0]["novelty"]["distance"], 1.0);

    // Repeat into the SAME dir, plain and with -v: stdout and files byte-identical, no new entries.
    let (code, stdout_again, _) = evaluate(&out_a, &corpus, &[], &[]);
    assert_eq!(code, 0);
    assert_eq!(stdout_again, stdout_a);
    assert_eq!(result_files(&out_a), files_a);
    let (code, stdout_verbose, stderr_verbose) = evaluate(&out_a, &corpus, &["-v"], &[]);
    assert_eq!(code, 0);
    assert_eq!(stdout_verbose, stdout_a);
    assert!(stderr_verbose.contains("INFO"), "stderr: {stderr_verbose}");
    assert_eq!(result_files(&out_a), files_a);

    // Fresh dir, --serial, --threads 2: files identical; stdout identical up to the out= echo.
    for (name, extra) in [
        ("eval-b", &[][..]),
        ("eval-serial", &["--serial"][..]),
        ("eval-threads", &["--threads", "2"][..]),
    ] {
        let out = out_dir(name);
        let (code, stdout, stderr) = evaluate(&out, &corpus, &[], extra);
        assert_eq!(code, 0, "{name} stderr: {stderr}");
        assert_eq!(without_out_path(&stdout), without_out_path(&stdout_a), "{name}");
        assert_eq!(result_files(&out), files_a, "{name}");
    }
}

#[test]
fn strict_exits_three_after_writing() {
    let out = out_dir("strict");
    let (code, stdout, stderr) = cli(&[
        "evaluate",
        "--game",
        "tictactoe",
        "--strategies",
        "tests/fixtures/strategies-eval.toml",
        "--roster",
        "tests/fixtures/roster-tiny.toml",
        "--games",
        "2",
        "--strict",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, 3, "stderr: {stderr}");
    assert!(
        stderr.contains("error: check failed: reference loss rate exceeded: random "),
        "stderr: {stderr}"
    );
    assert_eq!(
        stdout.lines().filter(|l| l.starts_with("strategy=")).count(),
        3,
        "stdout: {stdout}"
    );
    assert!(stdout.contains(" agreement=none "), "stdout: {stdout}");
    assert!(out.join("evaluation.json").is_file());
    assert!(out.join("archive").join("archive.json").is_file());
    assert!(out.join("archive").join("entries.jsonl").is_file());
}

#[test]
fn heuristic_rules_builds_and_plays() {
    let out = out_dir("heuristic");
    let (code, stdout, stderr) = cli(&[
        "evaluate",
        "--game",
        "tictactoe",
        "--strategies",
        "tests/fixtures/strategies-heuristic.toml",
        "--roster",
        "tests/fixtures/roster-tiny.toml",
        "--games",
        "1",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("strategies=1"), "stdout: {stdout}");

    let line = stdout
        .lines()
        .find(|l| l.starts_with("strategy=empty "))
        .unwrap_or_else(|| panic!("no strategy=empty line in stdout: {stdout}"));
    assert!(line.contains("kind=heuristic-rules"), "line: {line}");
    assert!(line.contains("games=6"), "line: {line}");
    assert!(line.contains("unfinished=0"), "line: {line}");
    assert!(line.contains("agreement=none"), "line: {line}");

    assert!(out.join("evaluation.json").exists());
}

#[test]
fn usage_errors_exit_two() {
    let out = out_dir("usage");
    let out_str = out.to_str().unwrap();
    let tiny = "tests/fixtures/roster-tiny.toml";
    let strategies = "tests/fixtures/strategies-eval.toml";

    let (code, _, stderr) = cli(&["evaluate", "--game", "chess", "--strategies", strategies, "--out", out_str]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("unknown game `chess`"), "stderr: {stderr}");

    let (code, _, stderr) = cli(&[
        "evaluate",
        "--game",
        "tictactoe",
        "--strategies",
        "tests/fixtures/nope.toml",
        "--out",
        out_str,
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("nope.toml"), "stderr: {stderr}");

    let (code, _, stderr) = cli(&[
        "evaluate",
        "--game",
        "tictactoe",
        "--strategies",
        strategies,
        "--roster",
        tiny,
        "--games",
        "1",
        "--reference",
        "nope",
        "--out",
        out_str,
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("unknown reference `nope`"), "stderr: {stderr}");

    let missing = out.join("no-annotations");
    let (code, _, stderr) = cli(&[
        "evaluate",
        "--game",
        "tictactoe",
        "--strategies",
        strategies,
        "--roster",
        tiny,
        "--games",
        "1",
        "--annotations",
        missing.to_str().unwrap(),
        "--out",
        out_str,
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("annotations.jsonl"), "stderr: {stderr}");

    assert!(!out.join("evaluation.json").exists());
}
