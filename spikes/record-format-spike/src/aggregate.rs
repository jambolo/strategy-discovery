//! Aggregation A (`first_move_orbit`, `outcome`) and B (`move_number`, `outcome`), computed
//! identically by the serde+HashMap path and (feature-gated) by polars over JSONL and Parquet.
//! Canonical result form: `Vec<(key1, key2, count)>` sorted ascending by `(key1, key2)`.

use crate::record::PositionRecord;
use std::collections::HashMap;

pub type AggResult = Vec<(u64, String, u64)>;

fn aggregate_by<F>(records: &[PositionRecord], key1: F) -> AggResult
where
    F: Fn(&PositionRecord) -> u64,
{
    let mut counts: HashMap<(u64, String), u64> = HashMap::new();
    for record in records {
        *counts.entry((key1(record), record.outcome.clone())).or_insert(0) += 1;
    }
    let mut result: AggResult = counts.into_iter().map(|((k1, k2), n)| (k1, k2, n)).collect();
    result.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    result
}

/// A: count grouped by `(first_move_orbit, outcome)`.
pub fn aggregate_a(records: &[PositionRecord]) -> AggResult {
    aggregate_by(records, |r| r.first_move_orbit as u64)
}

/// B: count grouped by `(move_number, outcome)`.
pub fn aggregate_b(records: &[PositionRecord]) -> AggResult {
    aggregate_by(records, |r| r.move_number as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selfplay::generate;

    #[test]
    fn aggregate_a_and_b_totals_match_record_count() {
        let records = generate(30, 7);
        let a = aggregate_a(&records);
        let b = aggregate_b(&records);
        assert_eq!(a.iter().map(|(_, _, n)| n).sum::<u64>(), records.len() as u64);
        assert_eq!(b.iter().map(|(_, _, n)| n).sum::<u64>(), records.len() as u64);
    }

    #[test]
    fn aggregate_a_is_sorted_ascending_by_key1_then_key2() {
        let records = generate(30, 7);
        let a = aggregate_a(&records);
        for w in a.windows(2) {
            assert!((w[0].0, &w[0].1) <= (w[1].0, &w[1].1));
        }
    }
}
