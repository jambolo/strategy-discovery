//! End-to-end CLI tests for the `discover` stage: the full file set and provenance chain of
//! a `discover-small` run, byte-identical repeat runs, standalone `analyze`/`report` replay
//! on a copied run directory, and the exit-2 (usage) / exit-3 (strict check failed) paths.

use std::path::{Path, PathBuf};

use strategy_discovery::io::{ArchiveEntry, DiscoverManifest, EvaluationReport, MiningReport, read_json, read_jsonl};

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

/// Walks `root` recursively and returns every regular file's path relative to `root`, with
/// `/` separators, sorted.
fn relative_files(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let rel = path.strip_prefix(root).expect("under root");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// Recursively copies every file under `src` into `dst`, preserving relative layout.
fn copy_dir(src: &Path, dst: &Path) {
    for entry in std::fs::read_dir(src).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&path, &target);
        } else {
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

const EXPECTED_FILES: &[&str] = &[
    "analyze.json",
    "annotate.json",
    "annotations.jsonl",
    "archive/archive.json",
    "archive/entries.jsonl",
    "dataset.json",
    "dataset.jsonl",
    "discover.json",
    "evaluation.json",
    "games.jsonl",
    "heuristics.json",
    "positions.jsonl",
    "report.md",
    "run.json",
    "summary.json",
];

#[test]
fn discover_runs_end_to_end_and_repeats_byte_identically() {
    let a = out_dir("discover_e2e_a");
    let a_str = a.to_str().expect("temp path is utf-8");

    let (code, a_stdout, stderr) = cli(&["discover", "--config", "tests/fixtures/discover-small.toml", "--out", a_str]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let mut expected: Vec<String> = EXPECTED_FILES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(relative_files(&a), expected);

    let mining: MiningReport = read_json(&a.join("heuristics.json")).expect("heuristics.json");
    assert_eq!(mining.candidates.len(), 1);
    let candidate = &mining.candidates[0];
    assert_eq!(candidate.name, "mined-d6-l1");
    assert!(candidate.heuristic.validate().is_ok());

    let report_md = std::fs::read_to_string(a.join("report.md")).expect("report.md");
    assert!(report_md.contains("## Mine"));
    assert!(report_md.contains("#### Rules"));
    assert!(report_md.contains("#### Feature definitions"));
    assert!(
        report_md
            .lines()
            .any(|line| line.starts_with("1. if ") && line.contains(" then play a position in "))
    );

    let evaluation: EvaluationReport = read_json(&a.join("evaluation.json")).expect("evaluation.json");
    assert_eq!(evaluation.strategies.len(), 1);
    assert_eq!(evaluation.strategies[0].kind, "heuristic-rules");
    assert_eq!(evaluation.strategies[0].tournament.totals.games, 12);

    let entries: Vec<ArchiveEntry> = read_jsonl(&a.join("archive/entries.jsonl")).expect("entries.jsonl");
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.provenance.source, "discover");
    assert_eq!(entry.provenance.corpus_run_id, Some("5eafa65f76637df3".to_string()));
    let discovery = entry.provenance.discovery.as_ref().expect("discovery provenance");
    assert!(!discovery.config_hash.is_empty());
    assert_eq!(discovery.corpus_run_id, "5eafa65f76637df3");
    assert_eq!(discovery.params.max_depth, 6);

    let manifest: DiscoverManifest = read_json(&a.join("discover.json")).expect("discover.json");
    assert_eq!(manifest.run_id, "5eafa65f76637df3");
    assert_eq!(manifest.candidates.len(), 1);
    assert_eq!(manifest.candidates[0].name, "mined-d6-l1");
    assert_eq!(manifest.candidates[0].entry_id, entry.entry_id);

    let b = out_dir("discover_e2e_b");
    let b_str = b.to_str().expect("temp path is utf-8");
    let (code, b_stdout, stderr) = cli(&["discover", "--config", "tests/fixtures/discover-small.toml", "--out", b_str]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(relative_files(&a), relative_files(&b));

    for rel in relative_files(&a) {
        let bytes_a = std::fs::read(a.join(&rel)).unwrap();
        let bytes_b = std::fs::read(b.join(&rel)).unwrap();
        assert_eq!(bytes_a, bytes_b, "mismatch at {rel}");
    }

    assert_eq!(a_stdout.replace(a_str, "OUT"), b_stdout.replace(b_str, "OUT"));
}

#[test]
fn analyze_and_report_run_standalone_on_a_copy() {
    let orig = out_dir("discover_analyze_replay_orig");
    let orig_str = orig.to_str().expect("temp path is utf-8");
    let (code, _stdout, stderr) = cli(&[
        "discover",
        "--config",
        "tests/fixtures/discover-small.toml",
        "--out",
        orig_str,
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let copy = out_dir("discover_analyze_replay_copy");
    copy_dir(&orig, &copy);
    let copy_str = copy.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&["analyze", "--corpus", copy_str, "--analyzers", "mine", "--mine-depths", "6"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let bytes_orig = std::fs::read(orig.join("heuristics.json")).unwrap();
    let bytes_copy = std::fs::read(copy.join("heuristics.json")).unwrap();
    assert_eq!(bytes_orig, bytes_copy);

    let (code, stdout, stderr) = cli(&["report", "--input", copy_str]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("## Mine"));
}

#[test]
fn unknown_mine_engine_is_a_usage_error() {
    let dir = out_dir("discover_bad_engine");
    let sweep = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/generate-small.toml").replace('\\', "/");
    let config_path = dir.join("experiment.toml");
    std::fs::write(
        &config_path,
        format!(
            "schema_version = 1\nname = \"bad-engine\"\ngame = \"tictactoe\"\nout = \"run\"\n\n[generate]\nsweep = \"{sweep}\"\n\n[mine]\nengine = \"x\"\n"
        ),
    )
    .unwrap();

    let config_str = config_path.to_str().expect("temp path is utf-8");
    let out = dir.join("out");
    let out_str = out.to_str().expect("temp path is utf-8");
    let (code, _stdout, stderr) = cli(&["discover", "--config", config_str, "--out", out_str]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("engine `x`"), "stderr: {stderr}");
}

#[test]
fn strict_discover_exits_three_after_writing_evaluation() {
    let dir = out_dir("discover_strict_exit3");
    let sweep = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/generate-small.toml").replace('\\', "/");
    let roster = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/roster-tiny.toml").replace('\\', "/");
    let config_path = dir.join("experiment.toml");
    std::fs::write(
        &config_path,
        format!(
            "schema_version = 1\nname = \"strict-single-leaf\"\ngame = \"tictactoe\"\nout = \"run\"\n\n[generate]\nsweep = \"{sweep}\"\n\n[mine]\ndepths = [6]\nmin_leaf = 400\n\n[discover]\ngames = 2\nroster = \"{roster}\"\nstrict = true\n"
        ),
    )
    .unwrap();

    let config_str = config_path.to_str().expect("temp path is utf-8");
    let out = dir.join("out");
    let out_str = out.to_str().expect("temp path is utf-8");
    let (code, _stdout, stderr) = cli(&["discover", "--config", config_str, "--out", out_str]);
    assert_eq!(code, 3, "stderr: {stderr}");
    assert!(stderr.contains("check failed"), "stderr: {stderr}");
    assert!(out.join("evaluation.json").exists());
    assert!(out.join("discover.json").exists());
}
