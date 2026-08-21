//! JSONL read/write for `PositionRecord`: one `serde_json` object per line.

use crate::record::PositionRecord;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

/// Writes `records` as one JSON object per line.
pub fn write(path: &Path, records: &[PositionRecord]) {
    let file = File::create(path).expect("create jsonl file");
    let mut writer = BufWriter::new(file);
    for record in records {
        serde_json::to_writer(&mut writer, record).expect("serialize record");
        writer.write_all(b"\n").expect("write newline");
    }
    writer.flush().expect("flush jsonl writer");
}

/// Reads every line back into a `PositionRecord`.
pub fn read(path: &Path) -> Vec<PositionRecord> {
    let file = File::open(path).expect("open jsonl file");
    let reader = BufReader::new(file);
    reader
        .lines()
        .map(|line| {
            let line = line.expect("read jsonl line");
            serde_json::from_str(&line).expect("deserialize record")
        })
        .collect()
}
