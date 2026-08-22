## Header

| field | value |
| --- | --- |
| date | 2026-08-23 |
| git_commit | fc3d6cdd01614bfd27c9e5f861d81b8347990a55 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/m1/tournament-throughput.ps1 (script reproduced verbatim in § Commands; binary target/release/strategy-discovery.exe built by `cargo build --release`; bench binary built by `cargo test --release --test tournament_bench --no-run`) |

## Scope

Gate C release-profile evidence of harness throughput, gathered by the embedded PowerShell script in four measurements, each repeated three times with medians reported:

- `tests/tournament_bench.rs` — `perfect` versus the full `ttt-benchmark-v1` roster (7 opponents x 2 seats x 20 games/pairing = 280 games, depth 9) in five scheduling modes (serial, pool, threads 2/4/8), plus one agreement pass over corpus annotations (390 positions).
- `evaluate --strategies tests/fixtures/strategies-eval.toml --games 100` (3 strategies x 14 pairings x 100 games = 4200 games, no annotations) in six scheduling modes (`--serial`, `--threads 1/2/4/8`, default pool of 16 logical CPUs).
- Agreement over the exhaustive annotation (5478 records, 4520 non-terminal). `--games 0 --annotations DIR` isolates the agreement pass from game play: 3 strategies x 4520 positions = 13560 positions, wall time includes process start and annotation load.
- The canonical command, `--games 100 --annotations DIR`, on the default pool.

The bench's games/s is lower than the CLI's because the bench plays `perfect` against every opponent including `perfect` (full depth-9 searches on every move) while `strategies-eval.toml` also contains the cheap `random` and `depth-3` strategies. The `.json` twin carries the same numbers plus every individual run.

## Commands

```sh
cargo build --release
cargo test --release --test tournament_bench --no-run
```

```powershell
# Milestone 1 tournament throughput (release): (1) tests/tournament_bench.rs x3 -> games/s, us/move and
# speedup per scheduling mode (perfect vs the full ttt-benchmark-v1 roster, 20 games/pairing, depth 9);
# (2) `evaluate` wall time for tests/fixtures/strategies-eval.toml (3 strategies x 14 pairings x
# --games 100 = 4200 games) per scheduling mode, x3; (3) agreement positions/s over the exhaustive
# annotation (`--games 0 --annotations`, 3 x 4520 = 13560 positions, x3); (4) the canonical command
# (--games 100 --annotations, default pool) x3. Medians of 3. Writes the .json twin of the artifact.
# Prerequisites: `cargo build --release` and `cargo test --release --test tournament_bench --no-run`.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $Root
$Exe = Join-Path $Root 'target\release\strategy-discovery.exe'
$W = 'target/m1/throughput'
if (Test-Path $W) { Remove-Item -Recurse -Force $W }
New-Item -ItemType Directory -Force $W | Out-Null
$Json = 'artifacts/benchmarks/tournament-throughput.json'

function Get-Median([double[]]$Values) { $s = $Values | Sort-Object; return [double]$s[[int][math]::Floor($s.Count / 2)] }
function Invoke-Timed([string]$Label, [string[]]$CliArgs) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = & $Exe @CliArgs 2> "$W/stderr.txt"
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for: $($CliArgs -join ' ')" }
    Write-Host ('[{0}] wall_s={1:F3} last_line={2}' -f $Label, $sw.Elapsed.TotalSeconds, @($out)[-1])
    return [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
}

# (1) tests/tournament_bench.rs, three runs, parsed per mode.
$BenchModes = [ordered]@{}
foreach ($i in 1..3) {
    $lines = & cargo test --release --test tournament_bench -- --nocapture 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE -ne 0) { throw "tournament_bench run $i failed" }
    $lines | Where-Object { $_ -like '`[bench`]*' } | ForEach-Object { Write-Host ('run{0} {1}' -f $i, $_) }
    foreach ($l in ($lines | Where-Object { $_ -like '`[bench`] tournament*' })) {
        if ($l -match 'mode=(\S+) threads=(\S+) games=(\d+) plies=(\d+) games/s=([\d.]+) us/move=([\d.]+) speedup=([\d.]+)x') {
            $key = '{0}/{1}' -f $Matches[1], $Matches[2]
            if (-not $BenchModes.Contains($key)) {
                $BenchModes[$key] = [ordered]@{ mode = $Matches[1]; threads = $Matches[2]; games = [int]$Matches[3]; plies = [int]$Matches[4]; games_per_s = @(); us_per_move = @(); speedup = @() }
            }
            $BenchModes[$key].games_per_s += [double]$Matches[5]
            $BenchModes[$key].us_per_move += [double]$Matches[6]
            $BenchModes[$key].speedup += [double]$Matches[7]
        }
    }
    $agree = $lines | Where-Object { $_ -like '`[bench`] agreement*' } | Select-Object -First 1
    if ($agree -match 'positions/s=(\d+) positions=(\d+) rate=([\d.]+)') {
        if (-not $BenchModes.Contains('agreement')) { $BenchModes['agreement'] = [ordered]@{ positions = [int]$Matches[2]; rate = [double]$Matches[3]; positions_per_s = @() } }
        $BenchModes['agreement'].positions_per_s += [double]$Matches[1]
    }
}

# (2) evaluate wall per scheduling mode, no annotations: 3 strategies x 14 pairings x 100 games.
$Games = 4200
$Base = @('evaluate', '--game', 'tictactoe', '--strategies', 'tests/fixtures/strategies-eval.toml', '--games', '100')
$CliModes = @(
    @{ name = 'serial'; threads = 1; extra = @('--serial') },
    @{ name = 'threads-1'; threads = 1; extra = @('--threads', '1') },
    @{ name = 'threads-2'; threads = 2; extra = @('--threads', '2') },
    @{ name = 'threads-4'; threads = 4; extra = @('--threads', '4') },
    @{ name = 'threads-8'; threads = 8; extra = @('--threads', '8') },
    @{ name = 'pool'; threads = 16; extra = @() })
$Cli = @()
foreach ($m in $CliModes) {
    $walls = @()
    foreach ($i in 1..3) { $walls += Invoke-Timed ('evaluate {0} run{1}' -f $m.name, $i) ($Base + @('--out', "$W/cli-$($m.name)-$i") + $m.extra) }
    $med = Get-Median $walls
    $Cli += [ordered]@{ mode = $m.name; threads = $m.threads; games = $Games; wall_s = $walls; median_wall_s = $med; games_per_s = [math]::Round($Games / $med, 1) }
}

# (3) agreement over the exhaustive annotation: --games 0 isolates the agreement pass (+ load + startup).
Invoke-Timed 'annotate exhaustive' @('annotate', '--exhaustive', '--game', 'tictactoe', '--out', "$W/exhaustive") | Out-Null
$AgreeBase = @('evaluate', '--game', 'tictactoe', '--strategies', 'tests/fixtures/strategies-eval.toml', '--annotations', "$W/exhaustive")
$AgreeWalls = @()
foreach ($i in 1..3) { $AgreeWalls += Invoke-Timed ('agreement run{0}' -f $i) ($AgreeBase + @('--games', '0', '--out', "$W/agree-$i")) }
$AgreeMed = Get-Median $AgreeWalls
$Positions = 3 * 4520

# (4) the canonical command: --games 100 with annotations, default pool.
$FullWalls = @()
foreach ($i in 1..3) { $FullWalls += Invoke-Timed ('evaluate full run{0}' -f $i) ($AgreeBase + @('--games', '100', '--out', "$W/full-$i")) }
$FullMed = Get-Median $FullWalls

Write-Host '--- tournament_bench medians (3 runs) ---'
$Bench = @()
foreach ($k in $BenchModes.Keys) {
    if ($k -eq 'agreement') { continue }
    $b = $BenchModes[$k]
    $row = [ordered]@{ mode = $b.mode; threads = $b.threads; games = $b.games; plies = $b.plies; games_per_s = $b.games_per_s; median_games_per_s = (Get-Median $b.games_per_s); us_per_move = $b.us_per_move; median_us_per_move = (Get-Median $b.us_per_move); speedup = $b.speedup; median_speedup = (Get-Median $b.speedup) }
    $Bench += $row
    Write-Host ('bench mode={0} threads={1} games={2} plies={3} games/s={4} median_games/s={5} us/move={6} median_us/move={7} speedup={8} median_speedup={9}' -f $row.mode, $row.threads, $row.games, $row.plies, (($row.games_per_s | ForEach-Object { $_.ToString('0.0') }) -join ','), $row.median_games_per_s.ToString('0.0'), (($row.us_per_move | ForEach-Object { $_.ToString('0.0') }) -join ','), $row.median_us_per_move.ToString('0.0'), (($row.speedup | ForEach-Object { $_.ToString('0.00') }) -join ','), $row.median_speedup.ToString('0.00'))
}
$BenchAgree = $BenchModes['agreement']
Write-Host ('bench agreement corpus positions={0} rate={1} positions/s={2} median_positions/s={3}' -f $BenchAgree.positions, $BenchAgree.rate.ToString('0.000'), ($BenchAgree.positions_per_s -join ','), (Get-Median $BenchAgree.positions_per_s))
Write-Host '--- evaluate CLI medians (3 runs) ---'
foreach ($c in $Cli) { Write-Host ('cli mode={0} threads={1} games={2} wall_s={3} median_wall_s={4:F3} games/s={5}' -f $c.mode, $c.threads, $c.games, (($c.wall_s | ForEach-Object { $_.ToString('F3') }) -join ','), $c.median_wall_s, $c.games_per_s.ToString('0.0')) }
Write-Host ('agreement exhaustive positions={0} wall_s={1} median_wall_s={2:F3} positions/s={3}' -f $Positions, (($AgreeWalls | ForEach-Object { $_.ToString('F3') }) -join ','), $AgreeMed, [math]::Round($Positions / $AgreeMed, 0))
Write-Host ('evaluate full (--games 100 --annotations, pool) wall_s={0} median_wall_s={1:F3}' -f (($FullWalls | ForEach-Object { $_.ToString('F3') }) -join ','), $FullMed)

$Doc = [ordered]@{
    header = [ordered]@{
        date = (Get-Date -Format 'yyyy-MM-dd')
        git_commit = (& git rev-parse HEAD).Trim()
        rustc = (& rustc --version).Trim()
        os = 'Windows 11 Pro 10.0.26200'
        cpu = 'AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)'
        ram = '31 GiB'
        build_profile = 'release'
        command = 'pwsh -NoProfile -ExecutionPolicy Bypass -File target/m1/tournament-throughput.ps1'
    }
    config = [ordered]@{ roster = 'ttt-benchmark-v1'; opponents = 7; seats = 2; pairings = 14; strategies_file = 'tests/fixtures/strategies-eval.toml'; strategies = 3; repeats = 3; bench_games_per_pairing = 20; cli_games_per_pairing = 100; cli_games = $Games; exhaustive_positions = 4520 }
    tournament_bench = $Bench
    tournament_bench_agreement_corpus = [ordered]@{ positions = $BenchAgree.positions; rate = $BenchAgree.rate; positions_per_s = $BenchAgree.positions_per_s; median_positions_per_s = (Get-Median $BenchAgree.positions_per_s) }
    evaluate_cli = $Cli
    agreement_exhaustive = [ordered]@{ strategies = 3; positions = $Positions; wall_s = $AgreeWalls; median_wall_s = $AgreeMed; positions_per_s = [math]::Round($Positions / $AgreeMed, 0) }
    evaluate_full = [ordered]@{ games = $Games; annotations = 'exhaustive'; wall_s = $FullWalls; median_wall_s = $FullMed }
}
[System.IO.File]::WriteAllText((Join-Path $Root $Json), (($Doc | ConvertTo-Json -Depth 6) -replace "`r`n", "`n") + "`n")
Write-Host "json=$Json"
```

## Tournament bench

| mode | threads | games | plies | run1_games_per_s | run2_games_per_s | run3_games_per_s | median_games_per_s | median_us_per_move | median_speedup |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| serial | 1 | 280 | 2338 | 273.7 | 259.0 | 268.1 | 268.1 | 446.6 | 1.00 |
| pool | global | 280 | 2338 | 1507.1 | 1560.6 | 1635.3 | 1560.6 | 76.7 | 6.03 |
| threads | 2 | 280 | 2338 | 504.6 | 499.7 | 495.4 | 499.7 | 239.7 | 1.85 |
| threads | 4 | 280 | 2338 | 935.5 | 894.3 | 923.6 | 923.6 | 129.7 | 3.44 |
| threads | 8 | 280 | 2338 | 1378.3 | 1406.8 | 1400.1 | 1400.1 | 85.5 | 5.22 |

Agreement bench (perfect, corpus annotations): positions=390 rate=1 positions/s=45001,41649,42668 median=42668.

## Evaluate CLI

| mode | threads | games | run1_wall_s | run2_wall_s | run3_wall_s | median_wall_s | games_per_s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| serial | 1 | 4200 | 7.166 | 6.991 | 6.945 | 6.991 | 600.8 |
| threads-1 | 1 | 4200 | 6.607 | 6.641 | 6.648 | 6.641 | 632.4 |
| threads-2 | 2 | 4200 | 3.976 | 3.842 | 3.860 | 3.860 | 1088.1 |
| threads-4 | 4 | 4200 | 2.122 | 2.143 | 2.143 | 2.143 | 1959.9 |
| threads-8 | 8 | 4200 | 1.255 | 1.265 | 1.353 | 1.265 | 3320.2 |
| pool | 16 | 4200 | 1.127 | 1.098 | 1.112 | 1.112 | 3777.0 |

## Agreement

| input | strategies | positions | run1_wall_s | run2_wall_s | run3_wall_s | median_wall_s | positions_per_s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| exhaustive (--games 0) | 3 | 13560 | 0.053 | 0.053 | 0.080 | 0.053 | 255849 |

## Evaluate wall

| cmd | run1_wall_s | run2_wall_s | run3_wall_s | median_wall_s |
| --- | --- | --- | --- | --- |
| evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/throughput/exhaustive --games 100 --out DIR | 1.153 | 1.101 | 1.115 | 1.115 |

## Comparison with Phase 5 corpus generation

| measure | Phase 5 (artifacts/benchmarks/corpus-generation.md § Generation) | Milestone 1 evaluate CLI (this file) |
| --- | --- | --- |
| serial games/s | 736.0 | 600.8 |
| parallel games/s | 4651.8 | 3777.0 |

Same order of magnitude: YES
