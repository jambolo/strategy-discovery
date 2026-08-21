# ADR 0004: Summary aggregator

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- This is the first analyzer in the pluggable analyzer registry (`generate -> annotate -> analyze -> report`).

- Dataframe-layer default selected in `docs/component-evaluation.md` is `serde + custom aggregation`, not polars, for the record-ingestion/aggregation path.

- Aggregation benchmark, 10000 games / 76409 position records (artifacts/benchmarks/record-format.md § Aggregation): `serde_hashmap` wall time 71.203 ms; results identical to polars over both formats — `polars_jsonl == serde: YES`, `polars_parquet == serde: YES`.

- polars build cost is disproportionate to the aggregation it buys here (artifacts/benchmarks/record-format.md § Compile time): with_polars 167.1 s / 919 deps vs without_polars 11.2 s / 126 deps; polars 0.55.2 additionally failed to build on this toolchain (rustc 1.94.1) via a transitive `sysinfo` MSRV mismatch, so 0.53.0 was used (artifacts/benchmarks/record-format.md § Toolchain note).

## Decision

- Summary aggregator = serde deserialization of JSONL position records + hand-rolled aggregation, keyed with `BTreeMap` for deterministic output order.

- Relies on the `serde + custom aggregation` default chosen in `docs/component-evaluation.md`.

- Outputs: outcome distribution by first-move orbit, by move number, by strategy pairing.

## Consequences

- Phase 8 mining consumes this aggregator's output as its first analyzer; later analyzers register into the same pluggable registry without pipeline changes.

- `BTreeMap` keying is required wherever aggregator output order affects downstream diffing or reporting, to keep output byte-reproducible.

- If a future aggregation exceeds what hand-rolled `HashMap`/`BTreeMap` code can do efficiently, re-open the dataframe-layer question in `docs/component-evaluation.md` rather than silently adding polars here.
