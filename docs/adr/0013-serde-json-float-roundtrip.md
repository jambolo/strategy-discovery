# ADR 0013: serde_json float_roundtrip feature

## Status

Accepted — 2026-08-22 (Milestone 1 / plan Phase 7)

## Context

- The three D8 serde round-trip properties in `tests/invariants.rs` — bit-exact over
  `FeatureValue::Float(f64)` and `MinimaxConfig.epsilon` via JSON `to_string` -> `from_str` ->
  `assert_eq!`, plus TOML for `StrategySpec` — failed on one-ULP drift. Example:
  `Float(-261601.74642673362)` (bits `c10fef0df8ae944e`) read back as
  `Float(-261601.7464267336)` (bits `c10fef0df8ae944d`).

- The drift is a parser property, not a writer bug. `serde_json::to_string(&v)` is byte-identical
  to Rust's shortest representation `format!("{:?}", v)`, and Rust's own `str::parse::<f64>()` on
  that text returns the original bits exactly. serde_json's default `f64` parser is a documented
  fast approximate parser that is one ULP off for a measurable fraction of values — this is not a
  regression in the crate, it is the documented default trade-off.

- Evidence (standalone crate, 200000 random `f64` values, counting
  `from_str(to_string(v)) != v`):

  | serde_json | features | mismatches / 200000 |
  | --- | --- | --- |
  | 1.0.151 | default | 20737 |
  | 1.0.145 | default | 20737 |
  | 1.0.151 | `float_roundtrip` | 0 |

  The 1.0.145 row rules out the newer `zmij` float codec as the cause: the mismatch rate is
  identical two minor versions back, so this is long-standing default behavior, not a recent
  regression.

- 1.0.151 is the latest published version of `serde_json`, so no version bump helps; enabling the
  `float_roundtrip` feature is the only fix.

## Decision

- `Cargo.toml`'s only `[dependencies]` change this milestone:
  `serde_json = { version = "1.0.151", features = ["float_roundtrip"] }`. Same version, no
  `preserve_order`, no other feature added. The `game-player` git pin is unchanged.

- The fix is enabling the feature, never weakening a property. D8's generators, its
  `assert_eq!` calls and all seven test names in `tests/invariants.rs` are unchanged.

- `tests/invariants.proptest-regressions` is kept as a committed regression guard: its three
  seeds from the original failure are replayed first on every `cargo test --test invariants` run
  and must now pass.

## Consequences

- The feature trades parse speed for bit-exactness: per upstream docs, `float_roundtrip` slows
  only float *parsing*, roughly 2x. The project's persisted float volumes are tiny, so the debug
  `cargo test --workspace` wall-time budgets hold.

- Serialization output is unchanged (the writer was already bit-correct), so every pinned
  `run_id` and existing byte-identity artifact is unaffected by this change.

- The D8 round-trip properties, and the D5/D12 byte-identity claims that depend on exact float
  read-back, now rest on exact `f64` parsing rather than an approximate one.

- Evidence: `tests/invariants.rs`, `tests/invariants.proptest-regressions`.
