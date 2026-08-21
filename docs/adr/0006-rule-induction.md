# ADR 0006: Rule induction library

## Status

Accepted — 2026-08-21 (Phase 3, Gate A)

## Context

- `artifacts/benchmarks/rule-induction.md`, § Dataset: 5478 rows, 69 features (tier-1 PrimitiveFeatures + tier-2 tic-tac-toe extractor, canonicalized), labels 0=loss/1=draw/2=win for side to move, class counts loss=1574/draw=1068/win=2836, train 4382 / held-out 1096, seed 20260820.
- `artifacts/benchmarks/rule-induction.md`, § Results: linfa-trees 0.8.1 — fit 65.629 ms, train accuracy 0.8003, holdout accuracy 0.7892, deterministic YES, traversable YES via public API `linfa_trees::DecisionTree::root_node`, `TreeNode::is_leaf`/`children`/`split`/`prediction`, `DecisionTree::iter_nodes`/`num_leaves`.
- `artifacts/benchmarks/rule-induction.md`, § Results: smartcore 0.6.5 — fit 20.956 ms, train accuracy 0.9336, holdout accuracy 0.9270, deterministic YES, traversable YES-via-serde only, via `serde_json::to_value(&DecisionTreeClassifier)` exposing `nodes[].{output,split_feature,split_value,true_child,false_child}` and `classes[]` (no public tree-node API).
- `artifacts/benchmarks/rule-induction.md`, § Decision list: linfa-trees tree (max_depth=8, 16 leaves) converted to 16 rules + ELSE, printed in full in that section.
- `artifacts/benchmarks/rule-induction.md`, § Association-mining crate survey: apriori_pattern_miner 0.1.1 (2022-05-16), rust-rule-miner 0.2.2 (2026-01-06), fp-growth 0.1.6 (2021-04-20), dci 0.3.0 (2020-10-19) — survey only, nothing run.
- `docs/component-evaluation.md`, Matrix 2: linfa-trees final_score 4.25 (default, `>= 4.0`); smartcore final_score 3.95 (experimental, `3.0-4.0`); association mining (rust-rule-miner 0.2.2) final_score 2.75 (deferred, `< 3.0`).

## Decision

- Selected default: linfa-trees (final_score 4.25; docs/component-evaluation.md, Matrix 2).

- Experimental alternate: smartcore (final_score 3.95).

- Deferred: association mining (rust-rule-miner 0.2.2) (final_score 2.75).

- linfa-trees scores lower on raw accuracy than smartcore (holdout 0.7892 vs 0.9270) but wins on Reusability/Integration because its public node-traversal API is a supported contract the framework can build a tree-to-decision-list converter against, whereas smartcore's only traversal path is deserializing its private serde layout.

## Consequences

- `linfa`, `linfa-trees` 0.8.1, and `ndarray` 0.16 enter root `Cargo.toml` in Phase 8, not before (`docs/component-evaluation.md`, Matrix 2; `artifacts/benchmarks/rule-induction.json` component_versions).
- The Phase 8 tree-to-decision-list converter (CLAUDE.md rule 5: the framework owns rule forms, games supply vocabulary) targets `linfa_trees::DecisionTree`'s public node API (`root_node`, `TreeNode::is_leaf`/`children`/`split`/`prediction`, `iter_nodes`, `num_leaves`), following the pattern already exercised on the 16-rule decision list in the spike.
- smartcore stays usable only through its serde representation; that shape is not a supported public contract and must not be depended on for production tree traversal without re-validating against future smartcore releases.
- Association mining is revisited in Phase 8: if the linfa-trees decision-list output proves insufficient (e.g. rule counts or coverage too coarse), evaluate a custom itemset miner rather than the surveyed crates, none of which were benchmarked and one of which (rust-rule-miner) is the only actively maintained candidate found.
