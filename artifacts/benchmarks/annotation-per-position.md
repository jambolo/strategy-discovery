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
| command | cargo run --release --manifest-path spikes/annotate-spike/Cargo.toml --  |

## Per-position search timing

| subset | depth | positions | mean_us | median_us | p99_us | total_ms | none_results |
| --- | --- | --- | --- | --- | --- | --- | --- |
| all_positions | 9 | 5478 | 6.5 | 0.6 | 98.4 | 35.7 | 958 |
| canonical | 9 | 765 | 10.4 | 0.6 | 140.5 | 8.0 | 138 |
| all_positions | 10 | 5478 | 6.9 | 0.6 | 95.4 | 37.8 | 958 |
| canonical | 10 | 765 | 14.6 | 0.6 | 139.8 | 11.2 | 138 |

## Capability findings

- value exposed: NO
- best-move set exposed: NO
- tie enumeration exposed: NO

`game_player::minimax::search` returns only `Option<S::Action>`; the position value, the set of equal-best moves, and tie enumeration live on the private `Response` type and inside the private `search_recursive` function, and ties among equal-value candidates are resolved by stable sort order rather than exposed to callers.

## Exhaustive solver

- positions solved: 5478
- solve_ms: 3.5

| label | all_positions | non_terminal |
| --- | --- | --- |
| X wins | 2936 | 2310 |
| draw | 1068 | 1052 |
| O wins | 1474 | 1158 |

## Cross-check

- depth 9 disagreements: 0
- depth 10 disagreements: 0
