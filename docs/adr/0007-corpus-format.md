# ADR 0007: Corpus file format

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- `artifacts/benchmarks/record-format.md`, § Dataset: 10000 games / 76409 records, master seed 20260820.
- `artifacts/benchmarks/record-format.md`, § Write read size: jsonl 22658488 bytes, write 51.627 ms, read 58.468 ms; parquet 520113 bytes, write 13.404 ms, read 1.054 ms — a 43.6x size gap and a 55x read-time gap in Parquet's favor.
- `artifacts/benchmarks/record-format.md`, § Aggregation: `polars_jsonl == serde: YES`, `polars_parquet == serde: YES` — both formats round-trip to equivalent aggregation results, but the Parquet read/aggregation path is only exercised through polars (polars_parquet 6.949 ms), which is not the selected dataframe layer (ADR 0005).
- `artifacts/benchmarks/record-format.md`, § Compile time: `with_polars` 167.1 s / 919 dependencies vs `without_polars` 11.2 s / 126 dependencies — Parquet reading in this project is only available via polars/arrow, so selecting Parquet would reintroduce the compile-time and dependency cost ADR 0005 declined.
- `docs/plan.md`, § Further Considerations item 3: corpus format Option A = JSONL only for MVP (recommended if polars not selected); Option B = Parquet alongside JSONL (if polars selected).
- `docs/plan.md`, § Phase 5 step 2: position-level record (state, canonical state, legal moves, chosen move, move number, side to move; per game: id, strategy params, seeds, config hash, outcome, length) under a versioned schema (`schema_version`).
- `docs/component-evaluation.md`, Matrix 3: JSONL final_score 4.40 (default, `>= 4.0`); Parquet final_score 3.40 (experimental, `3.0-4.0`).

## Decision

- Selected default: JSONL (final_score 4.40; docs/component-evaluation.md, Matrix 3).

- Experimental alternate: Parquet (final_score 3.40).

- Since ADR 0005 does not select polars, and Parquet reading/writing in this codebase has only been exercised through polars/arrow, choosing JSONL avoids adding a Parquet dependency chain purely for corpus I/O; this follows plan Further Considerations item 3, Option A.

## Consequences

- Phase 5 corpus generation and annotation write JSONL only (`docs/plan.md` Further Considerations item 3, Option A); no Parquet writer ships in the MVP.
- `src/io` implements a streaming `serde_json` writer/reader over the versioned position-level record (plan Phase 5 step 2), one JSON object per line.
- Every record and corpus file carries `schema_version` so downstream Phase 8-9 miners can detect format drift.
- Parquet export is deferred behind a future optional Cargo feature, not built by default.
- Revisit if `polars` (ADR 0005) or a direct `arrow`+`parquet` dependency is promoted to root `Cargo.toml`; until then, the 43.6x size and 55x read-time gap versus Parquet (22658488 vs 520113 bytes; 58.468 ms vs 1.054 ms read) is the accepted cost of staying on plain JSONL with no polars/arrow dependency.
