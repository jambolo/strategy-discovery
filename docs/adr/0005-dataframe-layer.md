# ADR 0005: Dataframe/query layer

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- `artifacts/benchmarks/record-format.md`, § Dataset: 10000 games / 76409 records, master seed 20260820.
- `artifacts/benchmarks/record-format.md`, § Write read size: jsonl 22658488 bytes, write 51.627 ms, read 58.468 ms; parquet 520113 bytes, write 13.404 ms, read 1.054 ms.
- `artifacts/benchmarks/record-format.md`, § Aggregation: outcome-by-first-move-orbit aggregation — polars_jsonl 31.880 ms, polars_parquet 6.949 ms, serde_hashmap 71.203 ms; `polars_jsonl == serde: YES`; `polars_parquet == serde: YES` (plain `serde` + `HashMap` aggregation matches polars output exactly on both source formats).
- `artifacts/benchmarks/record-format.md`, § Compile time: `with_polars` 167.1 s / 919 dependencies vs `without_polars` 11.2 s / 126 dependencies (clean release build of the spike).
- `artifacts/benchmarks/record-format.md`, § Toolchain note: polars 0.55.2 fails on rustc 1.94.1 — polars-io 0.55.2 unconditionally enables polars-utils/sysinfo, pulling in sysinfo 0.39.x (rust-version 1.95), producing E0658 on the unstable `cfg_select!` macro; the benchmark used polars 0.53.0 instead.
- `docs/component-evaluation.md`, Matrix 1: serde + custom aggregation final_score 4.30 (default, `>= 4.0`); polars final_score 3.70 (experimental, `3.0-4.0`).

## Decision

- Selected default: serde + custom aggregation (final_score 4.30; docs/component-evaluation.md, Matrix 1).

- Experimental alternate: polars (final_score 3.70).

- serde-based aggregation matches polars' aggregation output exactly at current corpus scale (76409 records) while avoiding the compile-time and dependency-count cost, and avoiding a toolchain-MSRV conflict observed on rustc 1.94.1.

## Consequences

- `polars` does not enter root `Cargo.toml` for Phase 3-7; the analyze stage in `src/discovery`/`src/io` operates as plain Rust over deserialized `serde` records.
- Deterministic ordering in aggregation output uses `BTreeMap` rather than `HashMap`, since seeded reproducibility (CLAUDE.md, cross-cutting rule 3) requires stable iteration order — the spike's `HashMap` equality check does not itself guarantee output ordering.
- `polars` may still be used inside disposable spikes (as in `spikes/record-format-spike`) or reconsidered later as an optional, non-default Cargo feature; it is not wired into the default build.
- Revisit condition: polars' effective MSRV tracks a newer rustc than the project's pinned stable toolchain (evidenced by the 0.55.2 failure), and the with-polars compile-time budget (167.1 s / 919 deps) is disproportionate to plumbing needs at this scale — revisit if project MSRV advances past polars' requirement or if corpus scale outgrows serde-based aggregation performance (71.203 ms at 76409 records).
