# ADR 0003: JSON/TOML metadata persistence

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- No alternative contested; both crates are already root dependencies.

- Crate versions (artifacts/benchmarks/record-format.md § Crate versions): serde 1.0.229, serde_json 1.0.151. `toml` at root `Cargo.toml`: 1.1.4.

- Record/report persistence needs two distinct roles: human-authored run configuration input, and machine-produced run metadata/summaries/reports.

- Corpus record throughput evidence (informing the JSON side, artifacts/benchmarks/record-format.md § Write read size): 10000 games / 76409 position records, JSONL write 51.627 ms / read 58.468 ms via serde.

## Decision

- TOML (`toml` crate) for run configuration input — the format a human edits by hand to launch a pipeline stage.

- JSON (`serde_json`) for run metadata, summaries, and reports — machine-produced, machine-consumed, one-record-per-line where applicable (JSONL).

- Every persisted record/document, in either format, carries a `schema_version` field.

## Consequences

- Phase 4/5 config loaders parse TOML into typed structs via `serde::Deserialize`; pipeline stage outputs (metadata, summaries, reports) serialize via `serde::Serialize` to JSON/JSONL.

- Any schema change to a persisted record bumps `schema_version` and requires a compatibility note for existing corpora.

- No dataframe-format decision is made by this ADR; JSONL vs Parquet for bulk position records is decided separately (see ADR 0004 and `docs/component-evaluation.md`).
