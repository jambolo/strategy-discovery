//! Provisional per-ply position record (plan Phase 5 shape). Kept flat: scalar columns only,
//! so a JSONL dump maps directly onto a flat polars frame with no nesting.

use serde::{Deserialize, Serialize};

/// FNV-1a 64-bit hash.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// One record per ply: the state BEFORE `chosen_move` is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionRecord {
    pub schema_version: u32,
    pub game_id: u64,
    pub seed: u64,
    pub strategy_kind: String,
    pub config_hash: String,
    pub move_number: u8,
    pub side_to_move: String,
    pub state: String,
    pub canonical_state: String,
    pub legal_moves_mask: u16,
    pub chosen_move: u8,
    pub first_move_orbit: u8,
    pub outcome: String,
    pub game_length: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_64_of_empty_string_is_the_offset_basis() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
    }
}
