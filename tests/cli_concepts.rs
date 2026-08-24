//! End-to-end CLI tests for concept induction wired through `discover`: the full file set and
//! stdout shape of a `concepts-small` run, typed assertions on `concepts.json`/`heuristics.json`/
//! `dataset.json`/`vocabulary.json`/`report.md`/`archive/entries.jsonl`, byte-identical repeat
//! runs, and the three usage-error (exit 2) paths.

use std::path::{Path, PathBuf};

use strategy_discovery::io::{ArchiveEntry, ConceptReport, DatasetManifest, MiningReport, VocabularyReport, read_json, read_jsonl};

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

/// Recursively copies `src` into `dst` (which must already exist).
fn copy_dir(src: &Path, dst: &Path) {
    for entry in std::fs::read_dir(src).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("copy file");
        }
    }
}

const EXPECTED_FILES: &[&str] = &[
    "agreement.json",
    "analyze.json",
    "annotate.json",
    "annotations.jsonl",
    "archive/archive.json",
    "archive/entries.jsonl",
    "concepts.json",
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
    "vocabulary.json",
];

#[test]
fn discover_concepts_small_end_to_end() {
    let a = out_dir("concepts_e2e_a");
    let a_str = a.to_str().expect("temp path is utf-8");

    let (code, stdout, stderr) = cli(&["discover", "--config", "tests/fixtures/concepts-small.toml", "--out", a_str]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let mut expected: Vec<String> = EXPECTED_FILES.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(relative_files(&a), expected);

    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines[0].starts_with("run_id="), "line 0: {}", lines[0]);
    assert!(lines[1].starts_with("mode=corpus annotated=390"), "line 1: {}", lines[1]);
    assert!(lines[2].starts_with("games=144"), "line 2: {}", lines[2]);
    assert!(lines[3].starts_with("report="), "line 3: {}", lines[3]);

    let report_idx = lines.iter().position(|l| l.starts_with("report=")).expect("report= line");
    let concepts_lines: Vec<(usize, &&str)> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("concepts=")).collect();
    assert_eq!(concepts_lines.len(), 1, "stdout: {stdout}");
    let (concepts_idx, concepts_line) = concepts_lines[0];
    assert!(concepts_idx > report_idx);

    let strategy_idx = lines.iter().position(|l| l.starts_with("strategy=")).expect("strategy= line");
    assert!(concepts_idx < strategy_idx);
    assert!(concepts_line.contains(" evaluated="));
    assert!(concepts_line.contains(" rounds=1"));
    assert!(concepts_line.contains(" withhold_tier2=true"));

    assert!(lines[strategy_idx].starts_with("strategy=mined-d6-l1 kind=heuristic-rules"));
    assert!(lines.iter().any(|l| l.starts_with("evaluation=")));

    let last_nonempty = lines.iter().rev().find(|l| !l.is_empty()).expect("last non-empty line");
    assert!(
        last_nonempty.starts_with("discover=concepts-small run_id=5eafa65f76637df3"),
        "last line: {last_nonempty}"
    );

    let promoted_count: usize = concepts_line
        .strip_prefix("concepts=")
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .expect("concepts= count parses");

    let report: ConceptReport = read_json(&a.join("concepts.json")).expect("concepts.json");
    assert_eq!(report.run_id, Some("5eafa65f76637df3".to_string()));
    let p = &report.params;
    assert!(p.enabled);
    assert!(p.withhold_tier2);
    assert_eq!(p.rounds, 1);
    assert_eq!(p.beam, 24);
    assert_eq!(p.top_k, 4);
    assert_eq!(p.max_promoted, 2);
    assert_eq!(p.min_train_gain, 0.001);
    assert_eq!(p.min_holdout_gain, 0.0005);
    assert_eq!(p.holdout_fraction, 0.25);
    assert!(p.extended_tier1);
    assert_eq!(p.seed, 0);

    assert_eq!(report.withheld.len(), 22);
    for name in &report.withheld {
        assert!(name.starts_with("ttt."), "withheld: {name}");
    }

    assert!(!report.promoted.is_empty());
    assert_eq!(report.promoted.len(), promoted_count);

    for concept in &report.promoted {
        assert!(concept.name.starts_with("concept_"), "name: {}", concept.name);
        assert!(
            concept.definition.contains(" [tier-3] = "),
            "definition: {}",
            concept.definition
        );
        assert_eq!(concept.kind, "bool");
        assert!(concept.round >= 1);
        assert_eq!(concept.provenance.params_hash.len(), 16);
        assert!(
            concept
                .provenance
                .params_hash
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(concept.provenance.run_id, Some("5eafa65f76637df3".to_string()));
        assert!(!concept.similarity.is_empty());
    }

    let mining: MiningReport = read_json(&a.join("heuristics.json")).expect("heuristics.json");
    assert_eq!(mining.candidates.len(), 1);
    let candidate = &mining.candidates[0];
    assert_eq!(candidate.name, "mined-d6-l1");
    assert!(candidate.heuristic.validate().is_ok());
    let refs = candidate.heuristic.references();
    let promoted_names: std::collections::BTreeSet<String> = report.promoted.iter().map(|c| c.name.clone()).collect();
    assert!(refs.iter().any(|r| promoted_names.contains(r)), "refs: {refs:?}");

    let dataset: DatasetManifest = read_json(&a.join("dataset.json")).expect("dataset.json");
    assert_eq!(dataset.tiers, vec!["primitive".to_string()]);
    assert_eq!(dataset.columns.len(), 82);
    assert_eq!(dataset.rows, 190);
    for column in &dataset.columns {
        assert!(!column.name.starts_with("ttt."), "column: {}", column.name);
    }

    let vocabulary: VocabularyReport = read_json(&a.join("vocabulary.json")).expect("vocabulary.json");
    assert_eq!(vocabulary.overall.fractions["supplied"], 0.0);
    assert!(vocabulary.overall.fractions["invented"] > 0.0);

    let report_md = std::fs::read_to_string(a.join("report.md")).expect("report.md");
    assert!(report_md.contains("## Concepts"));
    assert!(report_md.contains("### Promoted"));
    assert!(report_md.contains("`concept_"));
    assert!(report_md.contains("## Vocabulary"));
    let feature_defs_idx = report_md
        .find("#### Feature definitions")
        .expect("Feature definitions heading");
    assert!(report_md[feature_defs_idx..].contains("concept_"));

    let entries: Vec<ArchiveEntry> = read_jsonl(&a.join("archive/entries.jsonl")).expect("entries.jsonl");
    assert_eq!(entries.len(), 1);
    let discovery = entries[0].provenance.discovery.as_ref().expect("discovery provenance");
    let concepts_prov = discovery.concepts.as_ref().expect("concepts provenance");
    assert!(concepts_prov.withhold_tier2);
    assert_eq!(concepts_prov.rounds_run, 1);
    assert!(concepts_prov.promoted >= 1);
    assert_eq!(concepts_prov.concepts.len(), concepts_prov.promoted);
    assert_eq!(concepts_prov.params_hash.len(), 16);
    assert!(
        concepts_prov
            .params_hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(concepts_prov.seed, 0);
}

#[test]
fn repeat_run_byte_identical() {
    let a = out_dir("concepts_repeat_a");
    let a_str = a.to_str().expect("temp path is utf-8");
    let b = out_dir("concepts_repeat_b");
    let b_str = b.to_str().expect("temp path is utf-8");

    let (code_a, stdout_a, stderr_a) = cli(&["discover", "--config", "tests/fixtures/concepts-small.toml", "--out", a_str]);
    assert_eq!(code_a, 0, "stderr: {stderr_a}");
    let (code_b, stdout_b, stderr_b) = cli(&["discover", "--config", "tests/fixtures/concepts-small.toml", "--out", b_str]);
    assert_eq!(code_b, 0, "stderr: {stderr_b}");

    assert_eq!(relative_files(&a), relative_files(&b));
    for rel in relative_files(&a) {
        let bytes_a = std::fs::read(a.join(&rel)).unwrap();
        let bytes_b = std::fs::read(b.join(&rel)).unwrap();
        assert_eq!(bytes_a, bytes_b, "mismatch at {rel}");
    }

    assert_eq!(stdout_a.replace(a_str, "OUT"), stdout_b.replace(b_str, "OUT"));
}

#[test]
fn concepts_without_induce_is_usage_error() {
    let dir = out_dir("concepts_no_induce");
    let dir_str = dir.to_str().expect("temp path is utf-8");

    let (code, _stdout, stderr) = cli(&["generate", "--config", "tests/fixtures/generate-small.toml", "--out", dir_str]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, _stdout, stderr) = cli(&["annotate", "--corpus", dir_str]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code, _stdout, stderr) = cli(&["analyze", "--corpus", dir_str, "--analyzers", "concepts"]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("requires [induction] enabled = true"), "stderr: {stderr}");
}

#[test]
fn withhold_without_enabled_is_usage_error() {
    let dir = out_dir("concepts_withhold_no_enable");
    let sweep = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/generate-small.toml").replace('\\', "/");
    let config_path = dir.join("experiment.toml");
    std::fs::write(
        &config_path,
        format!(
            "schema_version = 1\nname = \"withhold-no-enable\"\ngame = \"tictactoe\"\nout = \"run\"\n\n[generate]\nsweep = \"{sweep}\"\n\n[induction]\nwithhold_tier2 = true\n"
        ),
    )
    .unwrap();

    let config_str = config_path.to_str().expect("temp path is utf-8");
    let out = dir.join("out");
    let out_str = out.to_str().expect("temp path is utf-8");
    let (code, _stdout, stderr) = cli(&["discover", "--config", config_str, "--out", out_str]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("withhold_tier2 requires enabled = true"), "stderr: {stderr}");
}

#[test]
fn mine_before_concepts_is_usage_error() {
    let dir = out_dir("concepts_bad_order");
    let sweep = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/generate-small.toml").replace('\\', "/");
    let config_path = dir.join("experiment.toml");
    std::fs::write(
        &config_path,
        format!(
            "schema_version = 1\nname = \"bad-order\"\ngame = \"tictactoe\"\nout = \"run\"\n\n[generate]\nsweep = \"{sweep}\"\n\n[analyze]\nanalyzers = [\"dataset\", \"mine\", \"concepts\", \"vocabulary\"]\n\n[induction]\nenabled = true\n"
        ),
    )
    .unwrap();

    let config_str = config_path.to_str().expect("temp path is utf-8");
    let out = dir.join("out");
    let out_str = out.to_str().expect("temp path is utf-8");
    let (code, _stdout, stderr) = cli(&["discover", "--config", config_str, "--out", out_str]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("required order"), "stderr: {stderr}");
}

#[test]
fn flag_equivalence_matches_config_induction() {
    let dir = out_dir("concepts_flag_eq");
    let sweep = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/generate-small.toml").replace('\\', "/");
    let roster = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/roster-tiny.toml").replace('\\', "/");

    let body = format!(
        "game = \"tictactoe\"\nout = \"run\"\n\n[generate]\nsweep = \"{sweep}\"\n\n[analyze]\nanalyzers = [\"dataset\", \"concepts\", \"mine\", \"vocabulary\"]\n\n[mine]\ndepths = [6]\n\n[induction]\n{{INDUCTION_HEADER}}rounds = 1\nbeam = 24\ntop_k = 4\nmax_promoted = 2\nmin_train_gain = 0.001\nmin_holdout_gain = 0.0005\nholdout_fraction = 0.25\n\n[discover]\ngames = 2\nroster = \"{roster}\"\n"
    );

    // config ONE: enabled/withhold_tier2 present under [induction].
    let config_one = dir.join("one.toml");
    std::fs::write(
        &config_one,
        format!(
            "schema_version = 1\nname = \"flag-eq\"\n{}",
            body.replace("{INDUCTION_HEADER}", "enabled = true\nwithhold_tier2 = true\n")
        ),
    )
    .unwrap();

    // config TWO: omits enabled/withhold_tier2, relies on --induce --withhold-tier2 flags.
    let config_two = dir.join("two.toml");
    std::fs::write(
        &config_two,
        format!(
            "schema_version = 1\nname = \"flag-eq\"\n{}",
            body.replace("{INDUCTION_HEADER}", "")
        ),
    )
    .unwrap();

    let config_one_str = config_one.to_str().expect("temp path is utf-8");
    let config_two_str = config_two.to_str().expect("temp path is utf-8");

    let out1 = dir.join("out1");
    let out1_str = out1.to_str().expect("temp path is utf-8");
    let out2 = dir.join("out2");
    let out2_str = out2.to_str().expect("temp path is utf-8");

    let (code1, _stdout1, stderr1) = cli(&["discover", "--config", config_one_str, "--out", out1_str]);
    assert_eq!(code1, 0, "stderr: {stderr1}");

    let (code2, _stdout2, stderr2) = cli(&[
        "discover",
        "--config",
        config_two_str,
        "--out",
        out2_str,
        "--induce",
        "--withhold-tier2",
    ]);
    assert_eq!(code2, 0, "stderr: {stderr2}");

    for rel in ["concepts.json", "heuristics.json", "vocabulary.json", "dataset.json"] {
        let bytes1 = std::fs::read(out1.join(rel)).expect(rel);
        let bytes2 = std::fs::read(out2.join(rel)).expect(rel);
        assert_eq!(bytes1, bytes2, "mismatch at {rel}");
    }
}

#[test]
fn standalone_analyze_reproduces_discover_outputs() {
    let a = out_dir("concepts_standalone_a");
    let a_str = a.to_str().expect("temp path is utf-8");
    let (code_a, _stdout_a, stderr_a) = cli(&["discover", "--config", "tests/fixtures/concepts-small.toml", "--out", a_str]);
    assert_eq!(code_a, 0, "stderr: {stderr_a}");

    let c = out_dir("concepts_standalone_c");
    copy_dir(&a, &c);
    let c_str = c.to_str().expect("temp path is utf-8");

    let (code_c, _stdout_c, stderr_c) = cli(&[
        "analyze",
        "--corpus",
        c_str,
        "--analyzers",
        "dataset,concepts,mine,vocabulary",
        "--mine-depths",
        "6",
        "--induce",
        "--withhold-tier2",
        "--induction-rounds",
        "1",
        "--induction-beam",
        "24",
        "--induction-top-k",
        "4",
        "--induction-max-promoted",
        "2",
        "--induction-min-train-gain",
        "0.001",
        "--induction-min-holdout-gain",
        "0.0005",
        "--induction-holdout",
        "0.25",
    ]);
    assert_eq!(code_c, 0, "stderr: {stderr_c}");

    for rel in ["concepts.json", "heuristics.json", "vocabulary.json", "dataset.json"] {
        let bytes_a = std::fs::read(a.join(rel)).expect(rel);
        let bytes_c = std::fs::read(c.join(rel)).expect(rel);
        assert_eq!(bytes_a, bytes_c, "mismatch at {rel}");
    }

    let report_path = c.join("report.md");
    let report_path_str = report_path.to_str().expect("temp path is utf-8");
    let (code_r, stdout_r, stderr_r) = cli(&["report", "--input", c_str, "--out", report_path_str]);
    assert_eq!(code_r, 0, "stderr: {stderr_r}");

    let written = std::fs::read(&report_path).expect("report.md");
    assert_eq!(stdout_r.as_bytes(), written.as_slice());
    assert!(stdout_r.contains("## Concepts"));
    assert!(stdout_r.contains("## Vocabulary"));
}

#[test]
fn label_map_renders_labels() {
    let e0 = out_dir("concepts_label_e0");
    let e0_str = e0.to_str().expect("temp path is utf-8");
    let (code0, _stdout0, stderr0) = cli(&["discover", "--config", "tests/fixtures/concepts-small.toml", "--out", e0_str]);
    assert_eq!(code0, 0, "stderr: {stderr0}");

    let e = out_dir("concepts_label_e");
    copy_dir(&e0, &e);
    let e_str = e.to_str().expect("temp path is utf-8");

    let label_map = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/concept-labels.toml").replace('\\', "/");

    let (code, _stdout, stderr) = cli(&[
        "analyze",
        "--corpus",
        e_str,
        "--analyzers",
        "concepts",
        "--induce",
        "--withhold-tier2",
        "--induction-rounds",
        "1",
        "--induction-beam",
        "24",
        "--induction-top-k",
        "4",
        "--induction-max-promoted",
        "2",
        "--induction-min-train-gain",
        "0.001",
        "--induction-min-holdout-gain",
        "0.0005",
        "--induction-holdout",
        "0.25",
        "--label-map",
        &label_map,
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (code_r, stdout_r, stderr_r) = cli(&["report", "--input", e_str]);
    assert_eq!(code_r, 0, "stderr: {stderr_r}");
    assert!(stdout_r.contains("(label: first invented concept)"), "stdout: {stdout_r}");
}
