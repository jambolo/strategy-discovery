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

## Settings

- games per configuration: 200
- repeats: 3 (median reported)
- master seed: 20260820
- depths: 2, 4, 9
- first ply: seeded uniform random; later plies: minimax search both sides

## Games per second

| depth | serial | threads_1 | threads_2 | threads_4 | threads_8 | threads_16 |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | 32071.3 | 30839.0 | 50641.9 | 87569.5 | 102443.3 | 116083.3 |
| 4 | 5496.7 | 5451.8 | 9811.5 | 17575.6 | 24259.2 | 29666.1 |
| 9 | 977.2 | 961.5 | 1717.6 | 3164.9 | 4518.2 | 5409.0 |

## Speedup vs serial

| depth | serial | threads_1 | threads_2 | threads_4 | threads_8 | threads_16 |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | 1.00 | 0.96 | 1.58 | 2.73 | 3.19 | 3.62 |
| 4 | 1.00 | 0.99 | 1.78 | 3.20 | 4.41 | 5.40 |
| 9 | 1.00 | 0.98 | 1.76 | 3.24 | 4.62 | 5.54 |

## Nodes per second

nodes = `ResponseGenerator::generate` calls.

| depth | serial | threads_1 | threads_2 | threads_4 | threads_8 | threads_16 |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | 1358541 | 1306339 | 2145190 | 3709444 | 4339497 | 4917291 |
| 4 | 1794858 | 1780190 | 3203805 | 5739055 | 7921473 | 9687023 |
| 9 | 4054533 | 3989709 | 7126975 | 13132217 | 18747346 | 22443375 |

## Outcomes

| depth | plies_total | x_wins | o_wins | draws |
| --- | --- | --- | --- | --- |
| 2 | 1718 | 41 | 0 | 159 |
| 4 | 1800 | 0 | 0 | 200 |
| 9 | 1800 | 0 | 0 | 200 |
