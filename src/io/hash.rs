//! FNV-1a 64-bit hashing helpers used for content-addressed config and corpus identifiers.

use std::path::PathBuf;

use serde::Serialize;

use super::IoError;

/// Computes the FNV-1a 64-bit hash of `bytes`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Formats `x` as a zero-padded 16-digit lowercase hex string.
pub fn hex16(x: u64) -> String {
    format!("{x:016x}")
}

/// Hashes the compact JSON serialization of `value`, returning a 16-digit hex digest.
pub fn config_hash<T: Serialize>(value: &T) -> Result<String, IoError> {
    let bytes = serde_json::to_vec(value).map_err(|source| IoError::Json {
        path: PathBuf::from("<memory>"),
        line: 0,
        source,
    })?;
    Ok(hex16(fnv1a64(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_reference_values() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"abc"), 0xe71fa2190541574b);
    }

    #[test]
    fn hex16_pads_to_16_digits() {
        assert_eq!(hex16(0), "0000000000000000");
        assert_eq!(hex16(0xcbf29ce484222325), "cbf29ce484222325");
    }

    #[test]
    fn config_hash_is_fnv_of_compact_json() {
        assert_eq!(config_hash(&serde_json::json!({"a": 1})).unwrap(), "9c3e82dd6fcae8b1");
    }
}
