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
| command | cargo run --release --manifest-path spikes/record-format-spike/Cargo.toml --  |

## Crate versions

| crate | version |
| --- | --- |
| polars | 0.53.0 |
| polars-core | 0.53.0 |
| polars-io | 0.53.0 |
| polars-lazy | 0.53.0 |
| polars-parquet | 0.53.0 |
| polars-arrow | 0.53.0 |
| serde | 1.0.229 |
| serde_json | 1.0.151 |

## Toolchain note

- attempted version: polars 0.55.2
- failing transitive crate: sysinfo 0.39.x (rust-version = "1.95") via polars-io 0.55.2 unconditionally enabling polars-utils/sysinfo (sysinfo ^0.39)
- error class: E0658 (cfg_select! unstable on rustc 1.94.1)
- version actually used: polars 0.53.0
- polars' effective MSRV moves faster than this project's toolchain; this is maintenance-risk evidence for the evaluation.

## Dataset

- games: 10000
- records: 76409
- master seed: 20260820

## Write read size

| format | size_bytes | write_ms | read_ms | rows |
| --- | --- | --- | --- | --- |
| jsonl | 22658488 | 51.627 | 58.468 | 76409 |
| parquet | 520113 | 13.404 | 1.054 | 76409 |

## Aggregation

| way | wall_ms |
| --- | --- |
| polars_jsonl | 31.880 |
| polars_parquet | 6.949 |
| serde_hashmap | 71.203 |

- polars_jsonl == serde: YES
- polars_parquet == serde: YES

| first_move_orbit | outcome | count |
| --- | --- | --- |
| 0 | O | 8110 |
| 0 | X | 20100 |
| 0 | draw | 5130 |
| 1 | O | 11394 |
| 1 | X | 18456 |
| 1 | draw | 5328 |
| 2 | O | 1614 |
| 2 | X | 5296 |
| 2 | draw | 981 |

## Compile time

| variant | build_seconds | dependency_count |
| --- | --- | --- |
| with_polars | 167.1 | 919 |
| without_polars | 11.2 | 126 |
