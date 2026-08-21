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
| command | cargo run --release --manifest-path spikes/throughput-spike/Cargo.toml -- --out C:\Users\John\Projects\strategy-discovery\spikes\throughput-spike/../../artifacts --games 200 --repeats 3 --seed 20260820 |

## Result

- depth 2 threads 1: identical: YES serial 0x9460637c851e883e parallel 0x9460637c851e883e
- depth 2 threads 2: identical: YES serial 0x9460637c851e883e parallel 0x9460637c851e883e
- depth 2 threads 4: identical: YES serial 0x9460637c851e883e parallel 0x9460637c851e883e
- depth 2 threads 8: identical: YES serial 0x9460637c851e883e parallel 0x9460637c851e883e
- depth 2 threads 16: identical: YES serial 0x9460637c851e883e parallel 0x9460637c851e883e
- depth 4 threads 1: identical: YES serial 0x8b6130e4d9cb1cc6 parallel 0x8b6130e4d9cb1cc6
- depth 4 threads 2: identical: YES serial 0x8b6130e4d9cb1cc6 parallel 0x8b6130e4d9cb1cc6
- depth 4 threads 4: identical: YES serial 0x8b6130e4d9cb1cc6 parallel 0x8b6130e4d9cb1cc6
- depth 4 threads 8: identical: YES serial 0x8b6130e4d9cb1cc6 parallel 0x8b6130e4d9cb1cc6
- depth 4 threads 16: identical: YES serial 0x8b6130e4d9cb1cc6 parallel 0x8b6130e4d9cb1cc6
- depth 9 threads 1: identical: YES serial 0x2cabf5ad8b16976e parallel 0x2cabf5ad8b16976e
- depth 9 threads 2: identical: YES serial 0x2cabf5ad8b16976e parallel 0x2cabf5ad8b16976e
- depth 9 threads 4: identical: YES serial 0x2cabf5ad8b16976e parallel 0x2cabf5ad8b16976e
- depth 9 threads 8: identical: YES serial 0x2cabf5ad8b16976e parallel 0x2cabf5ad8b16976e
- depth 9 threads 16: identical: YES serial 0x2cabf5ad8b16976e parallel 0x2cabf5ad8b16976e
- all identical: YES
