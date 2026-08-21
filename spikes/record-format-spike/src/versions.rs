//! Exact resolved crate versions, scanned as plain text from the spike's own `Cargo.lock`
//! (avoids depending on cargo's internal lockfile parser).

use std::fs;

const TRACKED: &[&str] = &[
    "polars",
    "polars-core",
    "polars-io",
    "polars-lazy",
    "polars-parquet",
    "polars-arrow",
    "arrow",
    "serde",
    "serde_json",
];

/// Reads `<spike>/Cargo.lock` and returns `(crate, version)` pairs for every name in `TRACKED`
/// that appears in the lockfile, in `TRACKED` order.
pub fn resolved_versions() -> Vec<(String, String)> {
    let text = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock")).expect("read spike Cargo.lock");
    let lines: Vec<&str> = text.lines().collect();

    let mut found: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name = \"") {
            let Some(name) = rest.strip_suffix('"') else { continue };
            if !TRACKED.contains(&name) || found.contains_key(name) {
                continue;
            }
            for follow in &lines[i + 1..] {
                let follow_trimmed = follow.trim();
                if let Some(v) = follow_trimmed.strip_prefix("version = \"") {
                    if let Some(v) = v.strip_suffix('"') {
                        found.insert(name, v.to_string());
                    }
                    break;
                }
                if follow_trimmed.starts_with("[[package]]") {
                    break;
                }
            }
        }
    }

    TRACKED
        .iter()
        .filter_map(|&name| found.get(name).map(|v| (name.to_string(), v.clone())))
        .collect()
}
