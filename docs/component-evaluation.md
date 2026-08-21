# Component Evaluation

## Scope and method

This document scores the three contested categories of the Phase 3 analysis stack — dataframe/query layer, rule induction, and corpus file format — by weighted criteria against evidence produced by four spike crates; settled plumbing (the `game-player` minimax adapter, the self-play experiment runner, JSON/TOML metadata persistence, the summary aggregator) is instead recorded as short ADR notes in `docs/adr/`. All evidence was captured at git commit `e59c6de486ffca5df18f7933aec816aa04c10c06`, rustc 1.94.1 (e408947bf 2026-03-25), Windows 11 Pro 10.0.26200, AMD Ryzen 7 7800X3D (16 logical CPUs), 31 GiB RAM, release builds.

| # | criterion | weight |
| --- | --- | --- |
| 1 | Reusability across games | 0.30 |
| 2 | Determinism and reproducibility under seeds | 0.20 |
| 3 | Performance on target workloads | 0.20 |
| 4 | Integration complexity in Rust codebase | 0.15 |
| 5 | Observability and debuggability | 0.10 |
| 6 | Maintenance risk | 0.05 |

`final_score = sum(score_i * weight_i)`, scores are integers 1-5.

Thresholds: `>= 4.0` selected as default; `3.0 <= score < 4.0` optional/experimental; `< 3.0` reject/defer.

Citation convention: `<artifact path> § <H2 section>, <row/field> (<number>)`, e.g. `artifacts/benchmarks/record-format.md § Aggregation, row polars_jsonl (31.880 ms)`.

## Evidence artifacts

| artifact | produced by | used in |
| --- | --- | --- |
| artifacts/benchmarks/record-format.md | spikes/record-format-spike | Matrix 1, Matrix 3 |
| artifacts/benchmarks/record-format.json | spikes/record-format-spike | Matrix 1, Matrix 3 (raw data backing record-format.md) |
| artifacts/benchmarks/record-format-compile.json | spikes/record-format-spike | Matrix 1 (Integration complexity), Matrix 3 (Integration complexity) |
| artifacts/benchmarks/rule-induction.md | spikes/rule-induction-spike | Matrix 2 |
| artifacts/benchmarks/rule-induction.json | spikes/rule-induction-spike | Matrix 2 (raw data backing rule-induction.md) |
| artifacts/benchmarks/annotation-per-position.md | spikes/annotate-spike | Gate A row 2 |
| artifacts/benchmarks/selfplay-throughput.md | spikes/throughput-spike | Gate A row 3 |
| artifacts/reproducibility/annotation-determinism.md | spikes/annotate-spike | Gate A row 3 |
| artifacts/reproducibility/selfplay-serial-vs-parallel.md | spikes/throughput-spike | Gate A row 3 |

## Matrix 1 - Dataframe/query layer

| criterion | weight | polars | evidence | serde + custom aggregation | evidence |
| --- | --- | --- | --- | --- | --- |
| Reusability across games | 0.30 | 4 | artifacts/benchmarks/record-format.md § Aggregation — same query ran over JSONL and Parquet; schema-driven queries generic over record layout, nested per-game feature columns need explicit flattening | 4 | artifacts/benchmarks/record-format.md § Aggregation, row serde_hashmap — aggregation written once against the game-agnostic record type |
| Determinism and reproducibility under seeds | 0.20 | 4 | artifacts/benchmarks/record-format.md § Aggregation, `polars_jsonl == serde: YES`, `polars_parquet == serde: YES` — results equal serde only after explicit sort; group-by order not stable without sort | 5 | artifacts/benchmarks/record-format.md § Aggregation, `polars_jsonl == serde: YES`, `polars_parquet == serde: YES` — plain deterministic Rust, BTreeMap ordering |
| Performance on target workloads | 0.20 | 5 | artifacts/benchmarks/record-format.md § Aggregation, rows polars_jsonl (31.880 ms), polars_parquet (6.949 ms) over 76,409 records | 3 | artifacts/benchmarks/record-format.md § Aggregation, row serde_hashmap (71.203 ms) — 2.2x slower than polars-JSONL, 10x slower than polars-Parquet |
| Integration complexity in Rust codebase | 0.15 | 2 | artifacts/benchmarks/record-format.md § Compile time, row with_polars (167.1 s / 919 deps) vs without_polars (11.2 s / 126 deps); § Toolchain note — polars 0.55.2 fails on rustc 1.94.1 via sysinfo 0.39 rust-version 1.95, forced 0.53.0 | 5 | artifacts/benchmarks/record-format.md § Crate versions (serde 1.0.229 / serde_json 1.0.151 already root deps), § Compile time row without_polars — zero new crates |
| Observability and debuggability | 0.10 | 3 | artifacts/benchmarks/record-format.md § Aggregation — lazy plan opaque; errors surface at collect; equality had to be verified against serde | 5 | artifacts/benchmarks/record-format.md § Aggregation, row serde_hashmap — plain structs, debugger-steppable |
| Maintenance risk | 0.05 | 2 | artifacts/benchmarks/record-format.md § Toolchain note — MSRV outran project toolchain within two minor versions | 5 | artifacts/benchmarks/record-format.md § Crate versions — stable 1.x |
| final_score | 1.00 | 3.70 | 1.20+0.80+1.00+0.30+0.30+0.10 | 4.30 | 1.20+1.00+0.60+0.75+0.50+0.25 |

Tier: polars 3.70 → experimental; serde + custom aggregation 4.30 → default.

## Matrix 2 - Rule induction

| criterion | weight | linfa-trees | evidence | smartcore | evidence | association mining (rust-rule-miner 0.2.2) | evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Reusability across games | 0.30 | 4 | artifacts/benchmarks/rule-induction.md § Dataset, features 69; § Results, api_used — ndarray matrix from any game's FeatureVector, tree walk game-agnostic | 4 | artifacts/benchmarks/rule-induction.md § Results — same input shape | 3 | artifacts/benchmarks/rule-induction.md § Association-mining crate survey — itemset mining over boolean features generic in principle, runtime not measured |
| Determinism and reproducibility under seeds | 0.20 | 5 | artifacts/benchmarks/rule-induction.md § Results, row linfa-trees, deterministic YES | 5 | artifacts/benchmarks/rule-induction.md § Results, row smartcore, deterministic YES | 3 | artifacts/benchmarks/rule-induction.md § Association-mining crate survey — not measured; conservative |
| Performance on target workloads | 0.20 | 4 | artifacts/benchmarks/rule-induction.md § Results, fit_ms 65.629, holdout_accuracy 0.7892; § Decision list max_depth=8 | 5 | artifacts/benchmarks/rule-induction.md § Results, fit_ms 20.956, holdout_accuracy 0.9270 | 2 | artifacts/benchmarks/rule-induction.md § Association-mining crate survey — not measured; conservative floor |
| Integration complexity in Rust codebase | 0.15 | 4 | artifacts/benchmarks/rule-induction.md § Results, api_used row linfa-trees — public typed traversal `DecisionTree::root_node`, `TreeNode::is_leaf/children/split/prediction`, `DecisionTree::iter_nodes/num_leaves`; needs ndarray 0.16.1 in root | 2 | artifacts/benchmarks/rule-induction.md § Results, traversable YES-via-serde, api_used row smartcore — traversal ONLY via `serde_json::to_value(&DecisionTreeClassifier)` reading `nodes[].{output,split_feature,split_value,true_child,false_child}`, private layout, undocumented contract | 3 | artifacts/benchmarks/rule-induction.md § Association-mining crate survey — small 0.x crates, API not exercised |
| Observability and debuggability | 0.10 | 5 | artifacts/benchmarks/rule-induction.md § Decision list — 16-rule decision list printed from the fitted tree via public API | 3 | artifacts/benchmarks/rule-induction.md § Results, row smartcore — structure visible only after serialization | 3 | artifacts/benchmarks/rule-induction.md § Association-mining crate survey — not measured |
| Maintenance risk | 0.05 | 3 | artifacts/benchmarks/rule-induction.md § Results, version 0.8.1 — slow cadence | 3 | artifacts/benchmarks/rule-induction.md § Results, version 0.6.5 | 2 | artifacts/benchmarks/rule-induction.md § Association-mining crate survey — 3 of 4 crates last updated 2020-2022; only rust-rule-miner 0.2.2 updated 2026-01-06 |
| final_score | 1.00 | 4.25 | 1.20+1.00+0.80+0.60+0.50+0.15 | 3.95 | 1.20+1.00+1.00+0.30+0.30+0.15 | 2.75 | 0.90+0.60+0.40+0.45+0.30+0.10 |

Tier: linfa-trees 4.25 → default; smartcore 3.95 → experimental; association mining (rust-rule-miner 0.2.2) 2.75 → deferred.

## Matrix 3 - Corpus file format

| criterion | weight | JSONL | evidence | Parquet | evidence |
| --- | --- | --- | --- | --- | --- |
| Reusability across games | 0.30 | 5 | artifacts/benchmarks/record-format.md § Write read size, row jsonl — schema-free, self-describing lines, any record serializes via serde | 4 | artifacts/benchmarks/record-format.md § Write read size, row parquet; § Crate versions — columnar schema per record shape, nested structs need arrow conversion |
| Determinism and reproducibility under seeds | 0.20 | 5 | artifacts/benchmarks/record-format.md § Write read size, row jsonl (rows 76409); § Aggregation, `polars_jsonl == serde: YES` — text, line-ordered; repeat-write byte identity not separately measured | 3 | artifacts/benchmarks/record-format.md § Write read size, row parquet; § Aggregation, `polars_parquet == serde: YES` — binary layout depends on writer version/compression, repeat-write byte identity not measured, content round-trip confirmed |
| Performance on target workloads | 0.20 | 2 | artifacts/benchmarks/record-format.md § Write read size, row jsonl (22,658,488 bytes, write 51.627 ms, read 58.468 ms) | 5 | artifacts/benchmarks/record-format.md § Write read size, row parquet (520,113 bytes — 43.6x smaller, write 13.404 ms, read 1.054 ms — 55x faster read) |
| Integration complexity in Rust codebase | 0.15 | 5 | artifacts/benchmarks/record-format.md § Crate versions — serde_json already in root | 2 | artifacts/benchmarks/record-format.md § Compile time, § Crate versions — only exercised reader/writer is polars 0.53.0 (919 deps, 167.1 s build); arrow+parquet path not measured |
| Observability and debuggability | 0.10 | 5 | artifacts/benchmarks/record-format.md § Write read size, row jsonl — human-readable, grep/diff-able | 2 | artifacts/benchmarks/record-format.md § Write read size, row parquet — binary, needs tooling |
| Maintenance risk | 0.05 | 5 | artifacts/benchmarks/record-format.md § Crate versions — serde_json 1.0.151 | 2 | artifacts/benchmarks/record-format.md § Toolchain note — tied to polars MSRV churn |
| final_score | 1.00 | 4.40 | 1.50+1.00+0.40+0.75+0.50+0.25 | 3.40 | 1.20+0.60+1.00+0.30+0.20+0.10 |

Tier: JSONL 4.40 → default; Parquet 3.40 → experimental.

## Score summary

| category | candidate | final_score | tier |
| --- | --- | --- | --- |
| Dataframe/query layer | serde + custom aggregation | 4.30 | default |
| Dataframe/query layer | polars | 3.70 | experimental |
| Rule induction | linfa-trees | 4.25 | default |
| Rule induction | smartcore | 3.95 | experimental |
| Rule induction | association mining (rust-rule-miner 0.2.2) | 2.75 | deferred |
| Corpus file format | JSONL | 4.40 | default |
| Corpus file format | Parquet | 3.40 | experimental |

## Selected defaults

| category | component | final_score | ADR |
| --- | --- | --- | --- |
| Dataframe/query layer | serde + custom aggregation | 4.30 | docs/adr/0005-dataframe-layer.md |
| Rule induction | linfa-trees | 4.25 | docs/adr/0006-rule-induction.md |
| Corpus file format | JSONL | 4.40 | docs/adr/0007-corpus-format.md |

## Experimental alternates

| category | component | final_score | tier | condition to revisit |
| --- | --- | --- | --- | --- |
| Dataframe/query layer | polars | 3.70 | experimental | revisit if polars MSRV tracks stable and root build budget allows |
| Rule induction | smartcore | 3.95 | experimental | revisit if a public tree-traversal API ships |
| Corpus file format | Parquet | 3.40 | experimental | revisit if polars or arrow+parquet is promoted |
| Rule induction | association mining (rust-rule-miner 0.2.2) | 2.75 | deferred | Phase 8: custom itemset miner if tree rules insufficient |

## Consequence for Phase 5

- corpus format JSONL only (plan "Further Considerations" item 3, Option A)

- no `polars`/`parquet`/`arrow` in root `Cargo.toml`

- `linfa` + `linfa-trees` + `ndarray` enter root in Phase 8, not before

- annotation uses the framework-side exhaustive solver (see Gate A row 2)

## Not measured

- arrow+parquet (serde_arrow) path

- association-mining crate runtime/determinism

- polars 0.55.x performance (did not compile on rustc 1.94.1)

- Parquet and JSONL repeat-write byte identity

- cross-machine numbers

## Gate A

| # | plan Phase 3 acceptance check | result | evidence |
| --- | --- | --- | --- |
| 1 | `docs/component-evaluation.md` exists with completed scores for the analysis stack and ADR notes for settled components | PASS | this file + docs/adr/0001-game-player-minimax-adapter.md, docs/adr/0002-self-play-experiment-runner.md, docs/adr/0003-json-toml-metadata-persistence.md, docs/adr/0004-summary-aggregator.md |
| 2 | per-position annotation capability of `game-player` confirmed or fallback documented | PASS | artifacts/benchmarks/annotation-per-position.md § Capability findings (value exposed NO, best-move set exposed NO, tie enumeration exposed NO), fallback exhaustive solver 0 disagreements at depth 9 and 10, docs/adr/0001-game-player-minimax-adapter.md |
| 3 | at least one throughput benchmark and one reproducibility report under artifacts | PASS | artifacts/benchmarks/selfplay-throughput.md, artifacts/reproducibility/selfplay-serial-vs-parallel.md, artifacts/reproducibility/annotation-determinism.md |
| 4 | Gate A: analysis-stack selection completed | PASS | `## Selected defaults` above |

Gate A verdict: PASS
