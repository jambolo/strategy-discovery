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
    assert_eq!(lines.len(), 4, "stdout: {stdout}");
    for line in &lines {
        assert!(line.contains('\t'), "line missing tab: {line}");
    }
    for name in ["agreement", "dataset", "mine", "summary"] {
        let matches: Vec<&&str> = lines.iter().filter(|l| l.starts_with(&format!("{name}\t"))).collect();
        assert_eq!(matches.len(), 1, "expected exactly one `{name}` line, stdout: {stdout}");
    }
}

#[test]
fn mine_flags_are_validated() {
    let (code, _stdout, stderr) = cli(&["analyze", "--mine-engine", "x"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("engine `x`"), "stderr: {stderr}");

    let (code, _stdout, stderr) = cli(&["analyze", "--mine-depths", "4,x"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("is not a depth"), "stderr: {stderr}");

    let (code, _stdout, stderr) = cli(&["analyze", "--mine-holdout", "1.0"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("holdout_fraction"), "stderr: {stderr}");
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

#[test]
fn dataset_and_mine_end_to_end_byte_identical() {
    use strategy_discovery::io::MiningReport;

    let dir = out_dir("cli_analyze_dataset_and_mine_end_to_end");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, _stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0);

    let (code, _stdout, _stderr) = cli(&["annotate", "--corpus", dir_str]);
    assert_eq!(code, 0);

    let (code, _stdout, _stderr) = cli(&[
        "analyze",
        "--corpus",
        dir_str,
        "--analyzers",
        "summary,agreement,dataset,mine",
        "--mine-depths",
        "4,6",
    ]);
    assert_eq!(code, 0);

    for name in ["dataset.jsonl", "dataset.json", "heuristics.json", "analyze.json"] {
        assert!(dir.join(name).is_file(), "missing {name}");
    }
    let analyze_json = std::fs::read_to_string(dir.join("analyze.json")).unwrap();
    for name in [
        "\"name\": \"summary\"",
        "\"name\": \"agreement\"",
        "\"name\": \"dataset\"",
        "\"name\": \"mine\"",
    ] {
        assert!(analyze_json.contains(name), "analyze.json: {analyze_json}");
    }

    let heuristics_bytes = std::fs::read(dir.join("heuristics.json")).unwrap();
    let report: MiningReport = serde_json::from_slice(&heuristics_bytes).expect("heuristics.json parses");
    assert_eq!(report.candidates.len(), 2, "candidates: {:?}", report.candidates);
    assert_eq!(report.candidates[0].name, "mined-d4-l1");
    assert_eq!(report.candidates[1].name, "mined-d6-l1");
    for candidate in &report.candidates {
        candidate.heuristic.validate().unwrap();
        assert!(
            candidate.rules >= 1,
            "candidate {}: rules={}",
            candidate.name,
            candidate.rules
        );
    }
    assert_eq!(report.params.depths, vec![4, 6]);
    assert!(report.dataset.rows > 0);

    let before: Vec<(&str, Vec<u8>)> = [
        "summary.json",
        "agreement.json",
        "dataset.jsonl",
        "dataset.json",
        "heuristics.json",
        "analyze.json",
    ]
    .into_iter()
    .map(|name| (name, std::fs::read(dir.join(name)).unwrap()))
    .collect();

    let (code, _stdout, _stderr) = cli(&[
        "analyze",
        "--corpus",
        dir_str,
        "--analyzers",
        "summary,agreement,dataset,mine",
        "--mine-depths",
        "4,6",
    ]);
    assert_eq!(code, 0);

    for (name, bytes_before) in &before {
        let bytes_after = std::fs::read(dir.join(name)).unwrap();
        assert_eq!(
            bytes_after, *bytes_before,
            "{name} not byte-identical across repeated analyze runs"
        );
    }
}
