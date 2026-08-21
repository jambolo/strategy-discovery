//! Parquet I/O and polars-lazy aggregation, gated behind the `polars` feature.
//!
//! Record integer fields are `u8`/`u16` in `PositionRecord`, but are widened to `u32` when
//! building the polars `DataFrame` so no `dtype-u8`/`dtype-u16` polars feature is required.

use crate::aggregate::AggResult;
use crate::record::PositionRecord;
use polars::prelude::*;
use std::fs::File;
use std::path::Path;

/// Builds a flat `DataFrame` from `records`, one row per record, small ints widened to `u32`.
pub fn build_dataframe(records: &[PositionRecord]) -> DataFrame {
    let schema_version: Vec<u32> = records.iter().map(|r| r.schema_version).collect();
    let game_id: Vec<u64> = records.iter().map(|r| r.game_id).collect();
    let seed: Vec<u64> = records.iter().map(|r| r.seed).collect();
    let strategy_kind: Vec<&str> = records.iter().map(|r| r.strategy_kind.as_str()).collect();
    let config_hash: Vec<&str> = records.iter().map(|r| r.config_hash.as_str()).collect();
    let move_number: Vec<u32> = records.iter().map(|r| r.move_number as u32).collect();
    let side_to_move: Vec<&str> = records.iter().map(|r| r.side_to_move.as_str()).collect();
    let state: Vec<&str> = records.iter().map(|r| r.state.as_str()).collect();
    let canonical_state: Vec<&str> = records.iter().map(|r| r.canonical_state.as_str()).collect();
    let legal_moves_mask: Vec<u32> = records.iter().map(|r| r.legal_moves_mask as u32).collect();
    let chosen_move: Vec<u32> = records.iter().map(|r| r.chosen_move as u32).collect();
    let first_move_orbit: Vec<u32> = records.iter().map(|r| r.first_move_orbit as u32).collect();
    let outcome: Vec<&str> = records.iter().map(|r| r.outcome.as_str()).collect();
    let game_length: Vec<u32> = records.iter().map(|r| r.game_length as u32).collect();

    let columns = vec![
        Series::new("schema_version".into(), schema_version).into(),
        Series::new("game_id".into(), game_id).into(),
        Series::new("seed".into(), seed).into(),
        Series::new("strategy_kind".into(), strategy_kind).into(),
        Series::new("config_hash".into(), config_hash).into(),
        Series::new("move_number".into(), move_number).into(),
        Series::new("side_to_move".into(), side_to_move).into(),
        Series::new("state".into(), state).into(),
        Series::new("canonical_state".into(), canonical_state).into(),
        Series::new("legal_moves_mask".into(), legal_moves_mask).into(),
        Series::new("chosen_move".into(), chosen_move).into(),
        Series::new("first_move_orbit".into(), first_move_orbit).into(),
        Series::new("outcome".into(), outcome).into(),
        Series::new("game_length".into(), game_length).into(),
    ];
    DataFrame::new_infer_height(columns).expect("build dataframe from position records")
}

/// Writes `df` to `path` as Parquet.
pub fn write_parquet(path: &Path, df: &mut DataFrame) {
    let file = File::create(path).expect("create parquet file");
    ParquetWriter::new(file).finish(df).expect("write parquet");
}

/// Reads a Parquet file back into a `DataFrame`.
pub fn read_parquet(path: &Path) -> DataFrame {
    let file = File::open(path).expect("open parquet file");
    ParquetReader::new(file).finish().expect("read parquet")
}

/// Converts a `group_by([key1, outcome]).agg([len().alias("n")])` result into the canonical,
/// sorted `AggResult` form.
fn agg_result_from_df(df: &DataFrame, key1: &str) -> AggResult {
    let key1_ca = df
        .column(key1)
        .expect("key1 column present")
        .cast(&DataType::Int64)
        .expect("cast key1 to i64");
    let key1_ca = key1_ca.i64().expect("key1 as i64 chunked array");
    let key2_ca = df.column("outcome").expect("outcome column present");
    let key2_ca = key2_ca.str().expect("outcome as str chunked array");
    let n_ca = df
        .column("n")
        .expect("n column present")
        .cast(&DataType::UInt64)
        .expect("cast n to u64");
    let n_ca = n_ca.u64().expect("n as u64 chunked array");

    let mut result: AggResult = key1_ca
        .into_no_null_iter()
        .zip(key2_ca.into_no_null_iter())
        .zip(n_ca.into_no_null_iter())
        .map(|((k1, k2), n)| (k1 as u64, k2.to_string(), n))
        .collect();
    result.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    result
}

fn group_by_query(lf: LazyFrame, key1: &str) -> AggResult {
    let df = lf
        .group_by([col(key1), col("outcome")])
        .agg([len().alias("n")])
        .collect()
        .expect("group_by/agg/collect");
    agg_result_from_df(&df, key1)
}

/// Runs the aggregation via polars-lazy directly over a JSONL file.
pub fn aggregate_jsonl(path: &Path, key1: &str) -> AggResult {
    let lf = LazyJsonLineReader::new(PlRefPath::try_from_path(path).expect("valid jsonl path"))
        .finish()
        .expect("scan jsonl");
    group_by_query(lf, key1)
}

/// Runs the aggregation via polars-lazy directly over a Parquet file.
pub fn aggregate_parquet(path: &Path, key1: &str) -> AggResult {
    let lf = LazyFrame::scan_parquet(
        PlRefPath::try_from_path(path).expect("valid parquet path"),
        Default::default(),
    )
    .expect("scan parquet");
    group_by_query(lf, key1)
}
