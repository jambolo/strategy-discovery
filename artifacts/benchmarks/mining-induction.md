## Header

| field | value |
| --- | --- |
| date | 2026-08-24 |
| git_commit | 22238174d5ae503e2934502b2248e0cca8457844 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | `cargo build --release`, three `cargo test --release --test mining_bench -- --nocapture` runs, and the pwsh script `target/m2/mining-induction.ps1` (reproduced verbatim in § Commands) |

## Scope

Gate D release-profile evidence of mining/induction performance, Milestone 2:

- Bench build ms is the pure `build_dataset` call measured by `tests/mining_bench.rs` (the
  `[bench] dataset-build ...` lines), for both the small `generate-small` corpus (holdout 0.0)
  and the exhaustive dataset (627 rows x 70 columns, 5478 annotation records, holdout 0.2 seed
  0).
- Bench mine ms is a per-candidate approximation: the harness's `[bench] mine ...` lines report
  whole-call elapsed divided by 3 candidate depths, not a per-fit measurement.
- CLI walls (`analyze --analyzers dataset` and `analyze --analyzers dataset,mine`) are
  end-to-end via the `analyze` subcommand, including corpus load and file writes, distinct from
  the bench's pure `build_dataset` ms.
- Discover wall is the whole chain `generate -> annotate -> analyze -> report ->
  evaluate/archive` driven by `discover --config configs/tictactoe-discover.toml`.
- Holdout is 0.2 with seed 0 for the exhaustive bench fits and the discover-corpus per-engine
  fits; the full-scale discover config (`configs/tictactoe-discover.toml`) mines its four
  candidates at holdout 0.0.
- `linfa-trees` is documented as potentially non-deterministic across runs when split scores
  tie. cart bench rows were verified identical (rules/leaves/soundness) across the three bench
  runs; only `linfa-trees` run-1 values are reported here, and no stability claim is made for it.

## Commands

```sh
cargo build --release
cargo test --release --test mining_bench -- --nocapture
```

```powershell
# Milestone 2 mining/induction benchmark (release): (1) full-scale discover x3 -> wall_s
# median and candidate table (mined-d6/d8/d12/d0-l1); (2) discover-corpus dataset build
# (analyze --analyzers dataset) x3 wall; (3) per-engine (cart, linfa-trees) holdout 0.2
# fits via `analyze --analyzers dataset,mine` on copies of run-1. Parses the three
# target/m2/mi-bench-<k>.txt files (cargo test --release --test mining_bench -- --nocapture)
# for exhaustive/generate-small bench figures. Writes target/m2/mining-induction.json and
# target/m2/mi/tables.md.
# Prerequisites: `cargo build --release` and three `cargo test --release --test mining_bench
# -- --nocapture > target/m2/mi-bench-<k>.txt 2>&1` runs.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $Root
$Exe = Join-Path $Root 'target\release\strategy-discovery.exe'
$W = 'target/m2/mi'
if (Test-Path $W) { Remove-Item -Recurse -Force $W }
New-Item -ItemType Directory -Force $W | Out-Null

function Get-Median([double[]]$Values) { $s = $Values | Sort-Object; return [double]$s[[int][math]::Floor($s.Count / 2)] }

# (1) three full-scale discover runs into fresh dirs, timed.
$DiscoverWalls = @()
$Run1Stdout = $null
for ($k = 1; $k -le 3; $k++) {
    $OutDir = "$W/run-$k"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = & $Exe discover --config configs/tictactoe-discover.toml --out $OutDir 2>"$W/run-$k.stderr.txt"
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "discover run $k exit code $LASTEXITCODE" }
    $DiscoverWalls += [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
    if ($k -eq 1) {
        $out | Out-File -FilePath "$W/run-1.stdout.txt" -Encoding utf8
        $Run1Stdout = $out
    }
    Write-Host ('[discover run {0}] wall_s={1:F3}' -f $k, $sw.Elapsed.TotalSeconds)
}

$MinedLines = $Run1Stdout | Where-Object { $_ -match 'strategy=mined-' }
$LossZeroLines = $MinedLines | Where-Object { $_ -match 'loss_rate_vs_reference=0\.000' }
if (@($MinedLines).Count -ne 4 -or @($LossZeroLines).Count -lt 1) {
    $Marker = @()
    $Marker += "expected 4 strategy=mined- lines with at least one loss_rate_vs_reference=0.000"
    $Marker += "observed strategy=mined- lines ($(@($MinedLines).Count)):"
    $Marker += $MinedLines
    $Marker -join "`n" | Out-File -FilePath "$W/TARGET-MISS.txt" -Encoding utf8
    Write-Error "target miss, see $W/TARGET-MISS.txt"
    exit 1
}

$RunIdLine = $Run1Stdout | Where-Object { $_ -match '^run_id=' } | Select-Object -First 1
$RunId = if ($RunIdLine) { $RunIdLine -replace '^run_id=', '' } else { '' }

$DiscoverJson = Get-Content -Raw "$W/run-1/discover.json" | ConvertFrom-Json
$HeuristicsJson1 = Get-Content -Raw "$W/run-1/heuristics.json" | ConvertFrom-Json

# (2)/(3) copies of run-1 for analyze (which rewrites dataset/heuristics files in place).
Copy-Item -Recurse "$W/run-1" "$W/copy-ds"
Copy-Item -Recurse "$W/run-1" "$W/copy-cart"
Copy-Item -Recurse "$W/run-1" "$W/copy-linfa"

$DatasetWalls = @()
for ($k = 1; $k -le 3; $k++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $Exe analyze --corpus "$W/copy-ds" --analyzers dataset 2>"$W/dataset-$k.stderr.txt" | Out-Null
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "analyze dataset run $k exit code $LASTEXITCODE" }
    $DatasetWalls += [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
}

function Invoke-EngineFit([string]$Engine, [string]$Copy) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $Exe analyze --corpus $Copy --analyzers dataset,mine --mine-engine $Engine --mine-depths 4,8,0 --mine-min-leaf 1 --mine-seed 0 --mine-holdout 0.2 2>"$W/fit-$Engine.stderr.txt" | Out-Null
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "analyze fit $Engine exit code $LASTEXITCODE" }
    return [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
}

$CartWall = Invoke-EngineFit -Engine 'cart' -Copy "$W/copy-cart"
$LinfaWall = Invoke-EngineFit -Engine 'linfa-trees' -Copy "$W/copy-linfa"

$CartHeuristics = Get-Content -Raw "$W/copy-cart/heuristics.json" | ConvertFrom-Json
$LinfaHeuristics = Get-Content -Raw "$W/copy-linfa/heuristics.json" | ConvertFrom-Json

# Parse the three mining_bench text logs.
function Parse-BenchFile([string]$Path) {
    $lines = Get-Content $Path | Where-Object { $_ -match '^\[bench\] ' }
    $recs = @()
    foreach ($line in $lines) {
        $rec = [ordered]@{}
        $rec.raw = $line
        $rec.source = if ($line -match 'source=exhaustive') { 'exhaustive' } else { 'default' }
        if ($line -match 'dataset-build') {
            $rec.kind = 'dataset-build'
            if ($line -match 'rows=(\d+)') { $rec.rows = [int]$Matches[1] }
            if ($line -match 'columns=(\d+)') { $rec.columns = [int]$Matches[1] }
            if ($line -match 'ms=([\d.]+)') { $rec.ms = [double]$Matches[1] }
        } elseif ($line -match '^\[bench\] mine') {
            $rec.kind = 'mine'
            if ($line -match 'engine=(\S+)') { $rec.engine = $Matches[1] }
            if ($line -match 'depth=(\d+)') { $rec.depth = [int]$Matches[1] }
            if ($line -match 'ms=([\d.]+)') { $rec.ms = [double]$Matches[1] }
            if ($line -match 'rules=(\d+)') { $rec.rules = [int]$Matches[1] }
            if ($line -match 'leaves=(\d+)') { $rec.leaves = [int]$Matches[1] }
            if ($line -match 'soundness=([\d.]+)' -and $line -notmatch 'train_soundness|holdout_soundness') { $rec.soundness = [double]$Matches[1] }
            if ($line -match 'train_soundness=([\d.]+)') { $rec.train_soundness = [double]$Matches[1] }
            if ($line -match 'holdout_soundness=([\d.]+)') { $rec.holdout_soundness = [double]$Matches[1] }
        }
        $recs += [pscustomobject]$rec
    }
    return $recs
}

$Bench = @()
for ($k = 1; $k -le 3; $k++) { $Bench += ,(Parse-BenchFile "target/m2/mi-bench-$k.txt") }

function Find-BenchRec($BenchRun, [string]$Kind, [string]$Source, $Engine = $null, $Depth = $null) {
    $BenchRun | Where-Object {
        $_.kind -eq $Kind -and $_.source -eq $Source -and
        ($null -eq $Engine -or $_.engine -eq $Engine) -and
        ($null -eq $Depth -or $_.depth -eq $Depth)
    } | Select-Object -First 1
}

# Dataset-build rows.
$DbGenSmall = @(1, 2, 3 | ForEach-Object { Find-BenchRec $Bench[$_ - 1] 'dataset-build' 'default' })
$DbExhaustive = @(1, 2, 3 | ForEach-Object { Find-BenchRec $Bench[$_ - 1] 'dataset-build' 'exhaustive' })

$DatasetBuild = @()
$DatasetBuild += [ordered]@{
    source = 'generate-small'
    rows = $DbGenSmall[0].rows
    columns = $DbGenSmall[0].columns
    ms_runs = @($DbGenSmall[0].ms, $DbGenSmall[1].ms, $DbGenSmall[2].ms)
    median_ms = Get-Median @($DbGenSmall[0].ms, $DbGenSmall[1].ms, $DbGenSmall[2].ms)
}
$DatasetBuild += [ordered]@{
    source = 'exhaustive'
    rows = $DbExhaustive[0].rows
    columns = $DbExhaustive[0].columns
    ms_runs = @($DbExhaustive[0].ms, $DbExhaustive[1].ms, $DbExhaustive[2].ms)
    median_ms = Get-Median @($DbExhaustive[0].ms, $DbExhaustive[1].ms, $DbExhaustive[2].ms)
}
$DatasetBuild += [ordered]@{
    source = 'discover-corpus (end-to-end via analyze)'
    rows = $null
    columns = $null
    ms_runs = @([math]::Round($DatasetWalls[0] * 1000.0, 1), [math]::Round($DatasetWalls[1] * 1000.0, 1), [math]::Round($DatasetWalls[2] * 1000.0, 1))
    median_ms = Get-Median @([math]::Round($DatasetWalls[0] * 1000.0, 1), [math]::Round($DatasetWalls[1] * 1000.0, 1), [math]::Round($DatasetWalls[2] * 1000.0, 1))
}

# Exhaustive fits, holdout 0.2 seed 0: cart 4/8/0 then linfa-trees 4/8/0.
$ExhaustiveFits = @()
foreach ($Engine in @('cart', 'linfa-trees')) {
    foreach ($Depth in @(4, 8, 0)) {
        $Recs = @(1, 2, 3 | ForEach-Object { Find-BenchRec $Bench[$_ - 1] 'mine' 'exhaustive' $Engine $Depth })
        if ($Engine -eq 'cart') {
            $r0 = $Recs[0].rules; $l0 = $Recs[0].leaves; $ts0 = $Recs[0].train_soundness; $hs0 = $Recs[0].holdout_soundness
            foreach ($r in $Recs) {
                if ($r.rules -ne $r0 -or $r.leaves -ne $l0 -or $r.train_soundness -ne $ts0 -or $r.holdout_soundness -ne $hs0) {
                    throw "cart bench instability at depth=$Depth : $($r.rules)/$($r.leaves)/$($r.train_soundness)/$($r.holdout_soundness) vs $r0/$l0/$ts0/$hs0"
                }
            }
        }
        $ExhaustiveFits += [ordered]@{
            engine = $Engine
            depth = $Depth
            ms_runs = @($Recs[0].ms, $Recs[1].ms, $Recs[2].ms)
            median_ms = Get-Median @($Recs[0].ms, $Recs[1].ms, $Recs[2].ms)
            rules = $Recs[0].rules
            leaves = $Recs[0].leaves
            train_soundness = $Recs[0].train_soundness
            holdout_soundness = $Recs[0].holdout_soundness
        }
    }
}

# Discover-corpus fits, via analyze copies.
$CorpusFits = @()
foreach ($Cand in $CartHeuristics.candidates) {
    $CorpusFits += [ordered]@{
        engine = 'cart'
        depth = $Cand.max_depth
        rules = $Cand.rules
        leaves = $Cand.leaves
        train_soundness = $Cand.train_soundness
        holdout_soundness = $Cand.holdout_soundness
        fallback_rows = $Cand.fallback_rows
    }
}
foreach ($Cand in $LinfaHeuristics.candidates) {
    $CorpusFits += [ordered]@{
        engine = 'linfa-trees'
        depth = $Cand.max_depth
        rules = $Cand.rules
        leaves = $Cand.leaves
        train_soundness = $Cand.train_soundness
        holdout_soundness = $Cand.holdout_soundness
        fallback_rows = $Cand.fallback_rows
    }
}

# Candidates table, joined discover.json (order) with heuristics.json (run-1).
$Candidates = @()
foreach ($Cand in $DiscoverJson.candidates) {
    $Match = $HeuristicsJson1.candidates | Where-Object { $_.name -eq $Cand.name } | Select-Object -First 1
    $Candidates += [ordered]@{
        name = $Cand.name
        max_depth = $Match.max_depth
        rules = $Match.rules
        leaves = $Match.leaves
        fallback_rows = $Match.fallback_rows
        loss_rate_vs_reference = $Cand.loss_rate_vs_reference
        agreement_rate = $Cand.agreement_rate
        novelty = $Cand.novelty
    }
}

$RustcVersion = (rustc --version).Trim()
$GitCommit = (git rev-parse HEAD).Trim()

$Header = [ordered]@{
    date = '2026-08-24'
    git_commit = $GitCommit
    rustc = $RustcVersion
    os = 'Windows 11 Pro 10.0.26200'
    cpu = 'AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)'
    ram = '31 GiB'
    build_profile = 'release'
    command = 'cargo build --release; three `cargo test --release --test mining_bench -- --nocapture` runs; pwsh script target/m2/mining-induction.ps1 (reproduced verbatim in § Commands)'
}

$Result = [ordered]@{
    header = $Header
    dataset_build = $DatasetBuild
    exhaustive_fits = $ExhaustiveFits
    corpus_fits = $CorpusFits
    corpus_fit_walls = [ordered]@{ cart = $CartWall; 'linfa-trees' = $LinfaWall }
    discover_wall = [ordered]@{
        runs = $DiscoverWalls
        median_wall_s = Get-Median $DiscoverWalls
        run_id = $RunId
    }
    candidates = $Candidates
}

($Result | ConvertTo-Json -Depth 10) + "`n" | Out-File -FilePath 'target/m2/mining-induction.json' -Encoding utf8 -NoNewline

# Render the six .md table/line sections into target/m2/mi/tables.md.
$Md = @()
$Md += '## Dataset build'
$Md += ''
$Md += '| source | rows | columns | ms run 1 | ms run 2 | ms run 3 | median_ms |'
$Md += '| --- | --- | --- | --- | --- | --- | --- |'
foreach ($Row in $DatasetBuild) {
    $Md += "| $($Row.source) | $($Row.rows) | $($Row.columns) | $($Row.ms_runs[0]) | $($Row.ms_runs[1]) | $($Row.ms_runs[2]) | $($Row.median_ms) |"
}
$Md += ''

$Md += '## Exhaustive fits (holdout 0.2, seed 0)'
$Md += ''
$Md += '| engine | depth | fit_ms run 1 | fit_ms run 2 | fit_ms run 3 | median_ms | rules | leaves | train_soundness | holdout_soundness |'
$Md += '| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |'
foreach ($Row in $ExhaustiveFits) {
    $Md += "| $($Row.engine) | $($Row.depth) | $($Row.ms_runs[0]) | $($Row.ms_runs[1]) | $($Row.ms_runs[2]) | $($Row.median_ms) | $($Row.rules) | $($Row.leaves) | $($Row.train_soundness) | $($Row.holdout_soundness) |"
}
$Md += ''

$Md += '## Discover-corpus fits (holdout 0.2, seed 0, via analyze)'
$Md += ''
$Md += '| engine | depth | rules | leaves | train_soundness | holdout_soundness | fallback_rows |'
$Md += '| --- | --- | --- | --- | --- | --- | --- |'
foreach ($Row in $CorpusFits) {
    $Md += "| $($Row.engine) | $($Row.depth) | $($Row.rules) | $($Row.leaves) | $($Row.train_soundness) | $($Row.holdout_soundness) | $($Row.fallback_rows) |"
}
$Md += "end-to-end wall: $CartWall s (cart)"
$Md += "end-to-end wall: $LinfaWall s (linfa-trees)"
$Md += ''

$Md += '## Discover wall'
$Md += ''
$Md += '| run | wall_s |'
$Md += '| --- | --- |'
for ($k = 0; $k -lt 3; $k++) { $Md += "| $($k + 1) | $($DiscoverWalls[$k]) |" }
$Md += "median_wall_s: $(Get-Median $DiscoverWalls)"
$Md += "run_id: $RunId"
$Md += ''

$Md += '## Candidates (full-scale discover run 1)'
$Md += ''
$Md += '| candidate | max_depth | rules | leaves | fallback_rows | loss_rate_vs_reference | agreement_rate | novelty |'
$Md += '| --- | --- | --- | --- | --- | --- | --- | --- |'
foreach ($Row in $Candidates) {
    $Md += "| $($Row.name) | $($Row.max_depth) | $($Row.rules) | $($Row.leaves) | $($Row.fallback_rows) | $($Row.loss_rate_vs_reference) | $($Row.agreement_rate) | $($Row.novelty) |"
}
$Md += ''

$Md -join "`n" | Out-File -FilePath 'target/m2/mi/tables.md' -Encoding utf8

Write-Host 'mining-induction benchmark complete.'
```

## Dataset build

| source | rows | columns | ms run 1 | ms run 2 | ms run 3 | median_ms |
| --- | --- | --- | --- | --- | --- | --- |
| generate-small | 190 | 70 | 5.4 | 5.4 | 5.3 | 5.4 |
| exhaustive | 627 | 70 | 16.5 | 16.7 | 16.5 | 16.5 |
| discover-corpus (end-to-end via analyze) |  |  | 76 | 52 | 53 | 53 |

## Exhaustive fits (holdout 0.2, seed 0)

| engine | depth | fit_ms run 1 | fit_ms run 2 | fit_ms run 3 | median_ms | rules | leaves | train_soundness | holdout_soundness |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| cart | 4 | 1.9 | 1.6 | 1.6 | 1.6 | 10 | 10 | 0.974 | 0.952 |
| cart | 8 | 1.9 | 1.6 | 1.6 | 1.6 | 26 | 28 | 0.986 | 0.960 |
| cart | 0 | 1.9 | 1.6 | 1.6 | 1.6 | 35 | 38 | 0.992 | 0.960 |
| linfa-trees | 4 | 5.7 | 5.7 | 5.6 | 5.7 | 5 | 5 | 0.861 | 0.824 |
| linfa-trees | 8 | 5.7 | 5.7 | 5.6 | 5.7 | 13 | 13 | 0.875 | 0.832 |
| linfa-trees | 0 | 5.7 | 5.7 | 5.6 | 5.7 | 12 | 12 | 0.875 | 0.832 |

## Discover-corpus fits (holdout 0.2, seed 0, via analyze)

| engine | depth | rules | leaves | train_soundness | holdout_soundness | fallback_rows |
| --- | --- | --- | --- | --- | --- | --- |
| cart | 4 | 10 | 10 | 0.964 | 0.973 | 0 |
| cart | 8 | 23 | 23 | 0.982 | 0.964 | 0 |
| cart | 0 | 38 | 41 | 0.989 | 0.937 | 5 |
| linfa-trees | 4 | 6 | 6 | 0.852 | 0.784 | 0 |
| linfa-trees | 8 | 10 | 10 | 0.888 | 0.829 | 0 |
| linfa-trees | 0 | 16 | 16 | 0.901 | 0.838 | 0 |

end-to-end wall: 0.094 s (cart)

end-to-end wall: 0.106 s (linfa-trees)

## Discover wall

| run | wall_s |
| --- | --- |
| 1 | 0.856 |
| 2 | 0.826 |
| 3 | 0.826 |

median_wall_s: 0.826

run_id: bd760ff0ace48705

## Candidates (full-scale discover run 1)

| candidate | max_depth | rules | leaves | fallback_rows | loss_rate_vs_reference | agreement_rate | novelty |
| --- | --- | --- | --- | --- | --- | --- | --- |
| mined-d6-l1 | 6 | 16 | 16 | 0 | 0.050 | 0.970 | 1.000 |
| mined-d8-l1 | 8 | 24 | 25 | 1 | 0.000 | 0.979 | 0.039 |
| mined-d12-l1 | 12 | 38 | 42 | 5 | 0.000 | 0.990 | 0.031 |
| mined-d0-l1 | 0 | 40 | 44 | 5 | 0.000 | 0.990 | 0.004 |
