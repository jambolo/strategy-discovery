## Header

| field | value |
| --- | --- |
| date | 2026-08-21 |
| git_commit | e59c6de486ffca5df18f7933aec816aa04c10c06 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | cargo run --release --manifest-path spikes/rule-induction-spike/Cargo.toml --  |

## Dataset

- rows: 5478
- features: 69
- train rows: 4382
- held-out rows: 1096
- classes: 0=loss 1=draw 2=win (side to move)
- class counts: loss=1574 draw=1068 win=2836
- seed: 20260820

## Results

| library | version | fit_ms | train_accuracy | holdout_accuracy | deterministic | traversable | api_used |
| --- | --- | --- | --- | --- | --- | --- | --- |
| linfa-trees | 0.8.1 | 65.629 | 0.8003 | 0.7892 | YES | YES | linfa_trees::DecisionTree::root_node, TreeNode::is_leaf/children/split/prediction, DecisionTree::iter_nodes/num_leaves |
| smartcore | 0.6.5 | 20.956 | 0.9336 | 0.9270 | YES | YES-via-serde | serde_json::to_value(&DecisionTreeClassifier) exposing nodes[].{output,split_feature,split_value,true_child,false_child}, classes[] |

## Decision list

- source: linfa-trees max_depth=8 leaves=16

```text
1. IF ttt.threats.mine <= 0.500 AND theirs <= 2.500 AND line6.theirs <= 0.500 AND orbit0.mine <= 0.500 AND theirs <= 1.500 AND free <= 7.500 THEN class=win
2. IF ttt.threats.mine <= 0.500 AND theirs <= 2.500 AND line6.theirs <= 0.500 AND orbit0.mine <= 0.500 AND theirs <= 1.500 AND free > 7.500 THEN class=draw
3. IF ttt.threats.mine <= 0.500 AND theirs <= 2.500 AND line6.theirs <= 0.500 AND orbit0.mine <= 0.500 AND theirs > 1.500 THEN class=draw
4. IF ttt.threats.mine <= 0.500 AND theirs <= 2.500 AND line6.theirs <= 0.500 AND orbit0.mine > 0.500 THEN class=win
5. IF ttt.threats.mine <= 0.500 AND theirs <= 2.500 AND line6.theirs > 0.500 THEN class=draw
6. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs <= 2.500 AND ttt.fork_cells <= 1.500 AND orbit2.mine <= 0.500 AND line7.mine <= 1.500 AND ttt.line3.o <= 0.500 THEN class=loss
7. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs <= 2.500 AND ttt.fork_cells <= 1.500 AND orbit2.mine <= 0.500 AND line7.mine <= 1.500 AND ttt.line3.o > 0.500 AND orbit1.mine <= 1.500 THEN class=draw
8. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs <= 2.500 AND ttt.fork_cells <= 1.500 AND orbit2.mine <= 0.500 AND line7.mine <= 1.500 AND ttt.line3.o > 0.500 AND orbit1.mine > 1.500 THEN class=loss
9. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs <= 2.500 AND ttt.fork_cells <= 1.500 AND orbit2.mine <= 0.500 AND line7.mine > 1.500 THEN class=loss
10. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs <= 2.500 AND ttt.fork_cells <= 1.500 AND orbit2.mine > 0.500 THEN class=draw
11. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs <= 2.500 AND ttt.fork_cells > 1.500 THEN class=win
12. IF ttt.threats.mine <= 0.500 AND theirs > 2.500 AND orbit0.theirs > 2.500 THEN class=loss
13. IF ttt.threats.mine > 0.500 AND line2.theirs <= 2.500 AND line5.theirs <= 2.500 AND line3.theirs <= 2.500 THEN class=win
14. IF ttt.threats.mine > 0.500 AND line2.theirs <= 2.500 AND line5.theirs <= 2.500 AND line3.theirs > 2.500 THEN class=loss
15. IF ttt.threats.mine > 0.500 AND line2.theirs <= 2.500 AND line5.theirs > 2.500 THEN class=loss
16. IF ttt.threats.mine > 0.500 AND line2.theirs > 2.500 THEN class=loss
ELSE class=win
```

## Association-mining crate survey

| crate | version | description | updated_at | found_by |
| --- | --- | --- | --- | --- |
| apriori_pattern_miner | 0.1.1 | Implementation of Apriori Pattern Mining algorithm | 2022-05-16 | cargo search apriori --limit 20 |
| rust-rule-miner | 0.2.2 | Automatic rule discovery from historical data using association rule mining, sequential pattern mining | 2026-01-06 | cargo search apriori --limit 20 |
| fp-growth | 0.1.6 | An implementation of the FP-Growth algorithm in pure Rust | 2021-04-20 | cargo search fp-growth --limit 20 |
| dci | 0.3.0 | DCI-Closed, a frequent closed itemset mining algorithm, implemented in Rust | 2020-10-19 | cargo search "frequent itemset" --limit 20 |
