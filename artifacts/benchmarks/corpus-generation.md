## Header

| field | value |
| --- | --- |
| date | 2026-08-21 |
| git_commit | 47535735fd212696a7bdaa04b3b50980e7d9c3ab |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/gate-b/bench.ps1 (script reproduced in § Commands; binary target/release/strategy-discovery.exe built by `cargo build --release`) |

## Config

`configs/tictactoe-default.toml` with `games_per_cell` raised from 20 to 200 (7 benchmark-roster strategies → 49 ordered pairings × 200 games = 9800 games; default evaluator, no opening, no random opening plies), written by the script to `target/gate-b/bench/tictactoe-200.toml`:

```toml
schema_version = 1
game = "tictactoe"
seed = 20260821
games_per_cell = 200
```

- run_id: dea7db96340e923d (`config_hash` of the resolved config; identical for every run below)
- cells: 49
- games: 9800
- positions: 75004
- repeats: 3 per configuration (median reported; games/s = 9800 / median wall)

## Commands

```powershell
# Gate B benchmark: 9800-game corpus generation, annotation and analysis timings (release CLI).
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Exe = Join-Path $Root 'target\release\strategy-discovery.exe'
$Work = Join-Path $Root 'target\gate-b\bench'
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Force $Work | Out-Null
$Config = Join-Path $Work 'tictactoe-200.toml'
Set-Content -Path $Config -Encoding ascii -Value @(
    'schema_version = 1',
    'game = "tictactoe"',
    'seed = 20260821',
    'games_per_cell = 200'
)
Write-Host "root=$Root"
Write-Host "exe=$Exe"
Write-Host "config=$Config"

function Invoke-Timed([string]$Label, [string[]]$CliArgs) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = & $Exe @CliArgs
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for: $($CliArgs -join ' ')" }
    Write-Host ("[{0}] wall_s={1:F3} stdout={2}" -f $Label, $sw.Elapsed.TotalSeconds, ($out -join ' | '))
    return [double]$sw.Elapsed.TotalSeconds
}
function Get-Median([double[]]$Values) { $s = $Values | Sort-Object; return [double]$s[1] }
function Format-Runs([double[]]$Values) { return (($Values | ForEach-Object { $_.ToString('F3') }) -join ',') }

$Games = 9800
$Results = [ordered]@{}
foreach ($mode in @(
        @{ name = 'serial'; extra = @('--serial') },
        @{ name = 'threads-4'; extra = @('--threads', '4') },
        @{ name = 'parallel'; extra = @() })) {
    $walls = @()
    foreach ($i in 1..3) {
        $dir = Join-Path $Work ('gen-{0}-{1}' -f $mode.name, $i)
        $walls += Invoke-Timed ('generate {0} run{1}' -f $mode.name, $i) (@('generate', '--config', $Config, '--out', $dir) + $mode.extra)
    }
    $Results[$mode.name] = $walls
}

$Corpus = Join-Path $Work 'gen-parallel-1'
$Stages = [ordered]@{ annotate = @(); analyze = @(); exhaustive = @() }
foreach ($i in 1..3) { $Stages['annotate'] += Invoke-Timed "annotate run$i" @('annotate', '--corpus', $Corpus) }
foreach ($i in 1..3) { $Stages['analyze'] += Invoke-Timed "analyze run$i" @('analyze', '--corpus', $Corpus) }
foreach ($i in 1..3) { $Stages['exhaustive'] += Invoke-Timed "exhaustive run$i" @('annotate', '--exhaustive', '--game', 'tictactoe', '--out', (Join-Path $Work "exhaustive-$i")) }

Write-Host '--- medians ---'
foreach ($k in $Results.Keys) {
    $m = Get-Median $Results[$k]
    Write-Host ('generate {0}: runs={1} median_wall_s={2:F3} games_per_s={3:F1}' -f $k, (Format-Runs $Results[$k]), $m, ($Games / $m))
}
foreach ($k in $Stages.Keys) {
    $m = Get-Median $Stages[$k]
    Write-Host ('{0}: runs={1} median_wall_s={2:F3}' -f $k, (Format-Runs $Stages[$k]), $m)
}
Write-Host '--- files (gen-parallel-1) ---'
Get-ChildItem $Corpus -File | Sort-Object Name | ForEach-Object { Write-Host ('{0} bytes={1}' -f $_.Name, $_.Length) }
Write-Host '--- files (exhaustive-1) ---'
Get-ChildItem (Join-Path $Work 'exhaustive-1') -File | Sort-Object Name | ForEach-Object { Write-Host ('{0} bytes={1}' -f $_.Name, $_.Length) }
Write-Host '--- line counts (gen-parallel-1) ---'
foreach ($f in 'games.jsonl', 'positions.jsonl', 'annotations.jsonl') {
    Write-Host ('{0} lines={1}' -f $f, ([System.IO.File]::ReadLines((Join-Path $Corpus $f)) | Measure-Object).Count)
}
Write-Host '--- analyze with --min-distinct 0.3 (scale note) ---'
& $Exe analyze --corpus $Corpus --min-distinct 0.3 --strict
Write-Host "exit=$LASTEXITCODE"
```

## Generation

| mode | threads | run1_wall_s | run2_wall_s | run3_wall_s | median_wall_s | games_per_s |
| --- | --- | --- | --- | --- | --- | --- |
| serial | 1 | 13.316 | 13.283 | 13.359 | 13.316 | 736.0 |
| threads-4 | 4 | 4.164 | 4.065 | 4.114 | 4.114 | 2381.9 |
| parallel | 16 (rayon global pool) | 2.131 | 2.107 | 2.103 | 2.107 | 4651.8 |

## Output files

| file | bytes | records |
| --- | --- | --- |
| run.json | 13645 | 1 document |
| games.jsonl | 6161570 | 9800 |
| positions.jsonl | 28003228 | 75004 |
| annotations.jsonl | 829583 | 2882 |
| annotate.json | 233 | 1 document |
| summary.json | 9267 | 1 document |

## Annotate and analyze

| stage | run1_wall_s | run2_wall_s | run3_wall_s | median_wall_s | stdout |
| --- | --- | --- | --- | --- | --- |
| annotate | 0.143 | 0.142 | 0.154 | 0.143 | mode=corpus annotated=2882 terminal=0 disagreements=0 |
| analyze | 0.171 | 0.169 | 0.159 | 0.169 | games=9800 distinct_games=3225 canonical_coverage=0.935 decisive_fraction=0.510 diversity_pass=false |
| exhaustive | 0.053 | 0.057 | 0.053 | 0.053 | mode=exhaustive annotated=5478 terminal=958 disagreements=0 |

- exhaustive output: annotations.jsonl 1570396 bytes (5478 records), annotate.json 225 bytes

## corpus_bench (release)

`cargo test --release --test corpus_bench -- --nocapture` (980-game default config):

```text
[bench] corpus games/s serial=733 parallel=3768 games=980 positions=7491 write_ms=260.1
[bench] summary read+aggregate ms=22.8 distinct_games=606
```

## Comparison with Phase 3 spike numbers

| measure | Phase 3 spike (release, same machine) | Phase 5 pipeline (this file) |
| --- | --- | --- |
| JSONL size | 22658488 bytes for 76409 flat records (artifacts/benchmarks/record-format.md § Write read size) | 28003228 bytes for 75004 position records (records carry canonical_state, canonical_transform and legal_actions that the spike's flat record lacked) |
| JSONL write | 51.627 ms (record-format.md § Write read size) | included in the generate wall: parallel median 2.107 s for 9800 games incl. play |
| JSONL read + serde aggregation | 58.468 ms + 71.203 ms (record-format.md § Write read size, § Aggregation row serde_hashmap) | analyze median 0.169 s = read 9800 games + 75004 positions, aggregate, write summary.json |
| depth-9 self-play throughput | 977.2 games/s serial, 5409.0 at 16 threads (artifacts/benchmarks/selfplay-throughput.md § Games per second) | 736.0 serial, 4651.8 parallel global pool — mixed roster (random … perfect), 49 pairings |

Same order of magnitude: YES

## Diversity at 9800 games (scale note)

`analyze` with default thresholds reports `diversity_pass=false` at 9800 games because `distinct_game_fraction` = 3225 / 9800 = 0.329 < 0.5, while `canonical_coverage` 0.935 (715 of 765 canonical positions) and `decisive_fraction` 0.510 both pass. This is expected, not a regression: tic-tac-toe is a finite game, and low-entropy roster cells (e.g. perfect-vs-perfect, which always plays the same forced draw line) repeat identical game records as `games_per_cell` grows, so the distinct-game fraction is not scale-invariant, even as absolute state coverage keeps rising (715 canonical positions reached here vs about 460 at 980 games). The Gate B diversity claim is made at the default `games_per_cell = 20` in `artifacts/reproducibility/corpus-diversity.md`, not at this benchmark's `games_per_cell = 200`. At this larger scale, relaxing the threshold to `--min-distinct 0.3` passes:

```text
games=9800 distinct_games=3225 canonical_coverage=0.935 decisive_fraction=0.510 diversity_pass=true
exit=0
```
