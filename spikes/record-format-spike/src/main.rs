//! Throwaway spike: writes/reads >= 10,000 games of `PositionRecord`s as JSONL and Parquet,
//! aggregates them three ways (polars-lazy over JSONL, polars-lazy over Parquet, plain
//! serde+HashMap), and writes size/write/read/compile evidence artifacts. See
//! `implementation-artifacts/phase3-component-eval-10-record-format-spike.md` for the full
//! spec this crate satisfies.

mod aggregate;
mod evidence;
mod jsonl;
#[cfg(feature = "polars")]
mod polars_support;
mod record;
mod selfplay;
mod versions;

use aggregate::{aggregate_a, aggregate_b};
use clap::Parser;
use evidence::{Header, md_table_2, median3};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// Findings from the earlier attempt at `polars = "0.55"` in this same evaluation: it failed to
/// compile on this project's rustc (1.94.1) because `polars-io` 0.55.2 unconditionally enables
/// `polars-utils`'s `sysinfo` feature, and every `sysinfo` 0.39.x release requires rustc 1.95
/// (confirmed as a genuine `cfg_select!` unstable-feature compile error, not just an MSRV
/// warning, by building with `--ignore-rust-version`). Hard-coded here because it is a fact
/// about this evaluation, not something to reconstruct at runtime; only `version_used` below is
/// read from the spike's own `Cargo.lock`.
mod toolchain_note {
    pub const ATTEMPTED_VERSION: &str = "polars 0.55.2";
    pub const FAILING_CRATE: &str = "sysinfo 0.39.x";
    pub const FAILING_CRATE_RUST_VERSION: &str = "1.95";
    pub const VIA: &str = "polars-io 0.55.2 unconditionally enabling polars-utils/sysinfo (sysinfo ^0.39)";
    pub const ERROR_CLASS: &str = "E0658";
    pub const ERROR_DETAIL: &str = "cfg_select! unstable on rustc 1.94.1";
    pub const SENTENCE: &str =
        "polars' effective MSRV moves faster than this project's toolchain; this is maintenance-risk evidence for the evaluation.";
}

#[derive(Parser)]
#[command(name = "record-format-spike")]
struct Args {
    /// Directory containing (or to receive) `benchmarks/record-format*.{md,json}`.
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts"))]
    out: String,
    #[arg(long, default_value_t = 10_000)]
    games: u64,
    #[arg(long, default_value_t = 20_260_820)]
    seed: u64,
}

/// One row of the "Write read size" table; `None` fields render as "not measured".
struct FormatRow {
    format: &'static str,
    size_bytes: Option<u64>,
    write_ms: Option<f64>,
    read_ms: Option<f64>,
    rows: Option<u64>,
}

impl FormatRow {
    fn md_row(&self) -> String {
        format!(
            "| {} | {} | {} | {} | {} |\n",
            self.format,
            opt_u64(self.size_bytes),
            opt_f64(self.write_ms),
            opt_f64(self.read_ms),
            opt_u64(self.rows)
        )
    }

    fn json_row(&self) -> Value {
        json!({
            "format": self.format,
            "size_bytes": self.size_bytes.map(Value::from).unwrap_or_else(|| json!("not measured")),
            "write_ms": self.write_ms.map(Value::from).unwrap_or_else(|| json!("not measured")),
            "read_ms": self.read_ms.map(Value::from).unwrap_or_else(|| json!("not measured")),
            "rows": self.rows.map(Value::from).unwrap_or_else(|| json!("not measured")),
        })
    }
}

fn opt_u64(v: Option<u64>) -> String {
    v.map_or_else(|| "not measured".to_string(), |n| n.to_string())
}

fn opt_f64(v: Option<f64>) -> String {
    v.map_or_else(|| "not measured".to_string(), |n| format!("{n:.3}"))
}

fn opt_f64_json(v: Option<f64>) -> Value {
    v.map(Value::from).unwrap_or_else(|| json!("not measured"))
}

fn main() {
    let args = Args::parse();
    let invocation = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let command = format!("cargo run --release --manifest-path spikes/record-format-spike/Cargo.toml -- {invocation}");
    let header = Header::gather(command);

    let out_dir = PathBuf::from(&args.out);
    let benchmarks_dir = out_dir.join("benchmarks");
    fs::create_dir_all(&benchmarks_dir).expect("create benchmarks dir");

    let data_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/target/data"));
    fs::create_dir_all(&data_dir).expect("create data dir");

    // --- Generate dataset ---
    let records = selfplay::generate(args.games, args.seed);
    let record_count = records.len() as u64;

    // --- JSONL write/read ---
    let jsonl_path = data_dir.join("records.jsonl");
    let mut write_ms = [0.0; 3];
    for slot in &mut write_ms {
        if jsonl_path.exists() {
            fs::remove_file(&jsonl_path).expect("remove jsonl before rewrite");
        }
        let start = Instant::now();
        jsonl::write(&jsonl_path, &records);
        *slot = start.elapsed().as_secs_f64() * 1000.0;
    }
    let jsonl_write_ms = median3(write_ms);

    let mut read_ms = [0.0; 3];
    let mut jsonl_rows = 0u64;
    for slot in &mut read_ms {
        let start = Instant::now();
        let read_back = jsonl::read(&jsonl_path);
        *slot = start.elapsed().as_secs_f64() * 1000.0;
        jsonl_rows = read_back.len() as u64;
    }
    let jsonl_read_ms = median3(read_ms);
    let jsonl_size = fs::metadata(&jsonl_path).expect("stat jsonl").len();

    let jsonl_row = FormatRow {
        format: "jsonl",
        size_bytes: Some(jsonl_size),
        write_ms: Some(jsonl_write_ms),
        read_ms: Some(jsonl_read_ms),
        rows: Some(jsonl_rows),
    };

    // --- Parquet write/read (feature-gated) ---
    #[cfg_attr(not(feature = "polars"), allow(unused_variables))]
    let parquet_path = data_dir.join("records.parquet");
    #[cfg(feature = "polars")]
    let parquet_row = {
        let mut df = polars_support::build_dataframe(&records);
        let mut write_ms = [0.0; 3];
        for slot in &mut write_ms {
            if parquet_path.exists() {
                fs::remove_file(&parquet_path).expect("remove parquet before rewrite");
            }
            let start = Instant::now();
            polars_support::write_parquet(&parquet_path, &mut df);
            *slot = start.elapsed().as_secs_f64() * 1000.0;
        }
        let parquet_write_ms = median3(write_ms);

        let mut read_ms = [0.0; 3];
        let mut parquet_rows = 0u64;
        for slot in &mut read_ms {
            let start = Instant::now();
            let read_back = polars_support::read_parquet(&parquet_path);
            *slot = start.elapsed().as_secs_f64() * 1000.0;
            parquet_rows = read_back.height() as u64;
        }
        let parquet_read_ms = median3(read_ms);
        let parquet_size = fs::metadata(&parquet_path).expect("stat parquet").len();

        FormatRow {
            format: "parquet",
            size_bytes: Some(parquet_size),
            write_ms: Some(parquet_write_ms),
            read_ms: Some(parquet_read_ms),
            rows: Some(parquet_rows),
        }
    };
    #[cfg(not(feature = "polars"))]
    let parquet_row = FormatRow {
        format: "parquet",
        size_bytes: None,
        write_ms: None,
        read_ms: None,
        rows: None,
    };

    // --- Aggregation, three ways; A and B timed together per way ---
    let (a_serde, b_serde, serde_ms) = {
        let start = Instant::now();
        let read_back = jsonl::read(&jsonl_path);
        let a = aggregate_a(&read_back);
        let b = aggregate_b(&read_back);
        (a, b, start.elapsed().as_secs_f64() * 1000.0)
    };

    #[cfg(feature = "polars")]
    let (polars_jsonl_ms, polars_jsonl_eq_serde) = {
        let start = Instant::now();
        let a = polars_support::aggregate_jsonl(&jsonl_path, "first_move_orbit");
        let b = polars_support::aggregate_jsonl(&jsonl_path, "move_number");
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        (Some(ms), a == a_serde && b == b_serde)
    };
    #[cfg(not(feature = "polars"))]
    let (polars_jsonl_ms, polars_jsonl_eq_serde): (Option<f64>, bool) = (None, false);

    #[cfg(feature = "polars")]
    let (polars_parquet_ms, polars_parquet_eq_serde) = {
        let start = Instant::now();
        let a = polars_support::aggregate_parquet(&parquet_path, "first_move_orbit");
        let b = polars_support::aggregate_parquet(&parquet_path, "move_number");
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        (Some(ms), a == a_serde && b == b_serde)
    };
    #[cfg(not(feature = "polars"))]
    let (polars_parquet_ms, polars_parquet_eq_serde): (Option<f64>, bool) = (None, false);

    // --- Crate versions, exactly as resolved in the spike's own Cargo.lock ---
    let crate_versions = versions::resolved_versions();

    // --- Compile-time evidence, if measure-compile.ps1 has been run ---
    let compile_path = out_dir.join("benchmarks/record-format-compile.json");
    let compile_json: Option<Value> = fs::read_to_string(&compile_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    // ==================== Markdown ====================
    let mut md = String::new();

    md.push_str("## Header\n\n");
    md.push_str(&md_table_2(["field", "value"], &header.md_rows()));
    md.push('\n');

    md.push_str("## Crate versions\n\n");
    md.push_str(&md_table_2(
        ["crate", "version"],
        &crate_versions
            .iter()
            .map(|(c, v)| (c.as_str(), v.as_str()))
            .collect::<Vec<_>>(),
    ));
    md.push('\n');

    let polars_version_used = crate_versions
        .iter()
        .find(|(c, _)| c == "polars")
        .map(|(_, v)| v.as_str())
        .unwrap_or("not measured");
    md.push_str("## Toolchain note\n\n");
    md.push_str(&format!("- attempted version: {}\n", toolchain_note::ATTEMPTED_VERSION));
    md.push_str(&format!(
        "- failing transitive crate: {} (rust-version = \"{}\") via {}\n",
        toolchain_note::FAILING_CRATE,
        toolchain_note::FAILING_CRATE_RUST_VERSION,
        toolchain_note::VIA
    ));
    md.push_str(&format!(
        "- error class: {} ({})\n",
        toolchain_note::ERROR_CLASS,
        toolchain_note::ERROR_DETAIL
    ));
    md.push_str(&format!("- version actually used: polars {polars_version_used}\n"));
    md.push_str(&format!("- {}\n", toolchain_note::SENTENCE));
    md.push('\n');

    md.push_str("## Dataset\n\n");
    md.push_str(&format!("- games: {}\n", args.games));
    md.push_str(&format!("- records: {record_count}\n"));
    md.push_str(&format!("- master seed: {}\n", args.seed));
    md.push('\n');

    md.push_str("## Write read size\n\n");
    md.push_str("| format | size_bytes | write_ms | read_ms | rows |\n");
    md.push_str("| --- | --- | --- | --- | --- |\n");
    md.push_str(&jsonl_row.md_row());
    md.push_str(&parquet_row.md_row());
    md.push('\n');

    md.push_str("## Aggregation\n\n");
    md.push_str("| way | wall_ms |\n");
    md.push_str("| --- | --- |\n");
    md.push_str(&format!("| polars_jsonl | {} |\n", opt_f64(polars_jsonl_ms)));
    md.push_str(&format!("| polars_parquet | {} |\n", opt_f64(polars_parquet_ms)));
    md.push_str(&format!("| serde_hashmap | {serde_ms:.3} |\n"));
    md.push('\n');
    md.push_str(&format!(
        "- polars_jsonl == serde: {}\n",
        if polars_jsonl_eq_serde { "YES" } else { "NO" }
    ));
    md.push_str(&format!(
        "- polars_parquet == serde: {}\n",
        if polars_parquet_eq_serde { "YES" } else { "NO" }
    ));
    md.push('\n');
    md.push_str("| first_move_orbit | outcome | count |\n");
    md.push_str("| --- | --- | --- |\n");
    for (k1, k2, n) in &a_serde {
        md.push_str(&format!("| {k1} | {k2} | {n} |\n"));
    }
    md.push('\n');

    md.push_str("## Compile time\n\n");
    md.push_str("| variant | build_seconds | dependency_count |\n");
    md.push_str("| --- | --- | --- |\n");
    for variant in ["with_polars", "without_polars"] {
        let row = compile_json.as_ref().and_then(|v| v.get(variant));
        let build_seconds = row.and_then(|r| r.get("build_seconds"));
        let dependency_count = row.and_then(|r| r.get("dependency_count"));
        md.push_str(&format!(
            "| {variant} | {} | {} |\n",
            build_seconds.map_or_else(|| "not measured".to_string(), |v| v.to_string()),
            dependency_count.map_or_else(|| "not measured".to_string(), |v| v.to_string()),
        ));
    }

    fs::write(benchmarks_dir.join("record-format.md"), md).expect("write record-format.md");

    // ==================== JSON ====================
    let json_out = json!({
        "header": header.to_json(),
        "crate_versions": crate_versions.iter().map(|(c, v)| json!({"crate": c, "version": v})).collect::<Vec<_>>(),
        "toolchain_note": {
            "attempted_version": toolchain_note::ATTEMPTED_VERSION,
            "failing_crate": toolchain_note::FAILING_CRATE,
            "failing_crate_rust_version": toolchain_note::FAILING_CRATE_RUST_VERSION,
            "via": toolchain_note::VIA,
            "error_class": toolchain_note::ERROR_CLASS,
            "error_detail": toolchain_note::ERROR_DETAIL,
            "version_used": polars_version_used,
            "note": toolchain_note::SENTENCE,
        },
        "dataset": {
            "games": args.games,
            "records": record_count,
            "master_seed": args.seed,
        },
        "formats": [jsonl_row.json_row(), parquet_row.json_row()],
        "aggregation": {
            "timings": {
                "polars_jsonl": opt_f64_json(polars_jsonl_ms),
                "polars_parquet": opt_f64_json(polars_parquet_ms),
                "serde_hashmap": serde_ms,
            },
            "polars_jsonl_eq_serde": polars_jsonl_eq_serde,
            "polars_parquet_eq_serde": polars_parquet_eq_serde,
            "result_a": a_serde,
            "result_b": b_serde,
        },
        "compile": compile_json,
    });
    fs::write(
        benchmarks_dir.join("record-format.json"),
        serde_json::to_string_pretty(&json_out).expect("serialize record-format.json"),
    )
    .expect("write record-format.json");
}
