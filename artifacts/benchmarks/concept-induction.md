## Header

| field | value |
| --- | --- |
| date | 2026-09-10 |
| git_commit | f0ddefa05d0e22f3938fca68357b3800319c1688 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | cargo build --release; three `cargo test --release --test induction_bench -- --nocapture` runs; pwsh script target/m3/concept-bench.ps1 (reproduced verbatim in § Commands) |

## Scope

Release-profile evidence of concept-induction performance, Milestone 3:

- Full-scale rows come from `discover --config configs/tictactoe-concepts.toml`
  (three timed runs; run-1's documents supply the deterministic counts): per-round
  enumeration/promotion counts from `concepts.json`, the candidates table joined
  from `discover.json` and `heuristics.json`, and the given-vs-discovered overall
  fractions from `vocabulary.json`.
- Full-scale analyze walls are END-TO-END via the `analyze` subcommand on copies of
  run-1 (corpus load + dataset build + induction + file writes), with flags
  matching the fixture's `[induction]` table; the `dataset,concepts` row is the
  induction-inclusive wall, the `dataset` row the induction-free baseline.
- Generate-small rows are the PURE library timings printed by
  `tests/induction_bench.rs` (`induce_concepts` wall and a probe-equivalent single
  cart fit over the generate-small corpus), medians over three runs; round counts
  were asserted identical across the three runs.
- All timing cells vary run to run; every count cell is deterministic. Floats
  render {:.3}; the `.md` tables and `concept-induction.json` are emitted from the
  same variables in one script, so they agree cell for cell.

## Commands

Prerequisites (each its own tool call):

```sh
cargo build --release
cargo test --release --test induction_bench -- --nocapture > target/m3/cb-bench-1.txt 2>&1
cargo test --release --test induction_bench -- --nocapture > target/m3/cb-bench-2.txt 2>&1
cargo test --release --test induction_bench -- --nocapture > target/m3/cb-bench-3.txt 2>&1
```

Then the benchmark script `target/m3/concept-bench.ps1`, run as
`pwsh -NoProfile -ExecutionPolicy Bypass -File target/m3/concept-bench.ps1`:

```powershell
# Step 44-benchmark: release-profile concept-induction benchmark.
# Prerequisites (run before this script):
#   cargo build --release
#   cargo test --release --test induction_bench -- --nocapture > target/m3/cb-bench-1.txt 2>&1   (and -2, -3)
# Writes target/m3/cb/concept-induction.json, target/m3/cb/tables.md, target/m3/cb/header.md.
$ErrorActionPreference = 'Stop'
$Exe = './target/release/strategy-discovery.exe'
$W = 'target/m3/cb'
if (Test-Path $W) { Remove-Item -Recurse -Force $W }
New-Item -ItemType Directory -Force $W | Out-Null

function Get-Median([double[]]$Values) { $s = $Values | Sort-Object; return [double]$s[[int][math]::Floor($s.Count / 2)] }
function F3([double]$X) { return '{0:F3}' -f $X }

# (1) three timed full-scale discover runs.
$DiscoverWalls = @()
for ($k = 1; $k -le 3; $k++) {
    $OutDir = "$W/run-$k"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $out = & $Exe discover --config configs/tictactoe-concepts.toml --out $OutDir 2>"$W/run-$k.stderr.txt"
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "discover run $k exit code $LASTEXITCODE" }
    $DiscoverWalls += [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
    if ($k -eq 1) { ($out -join "`n") | Set-Content -NoNewline -Encoding utf8 "$W/run-1.stdout.txt" }
}
$RunIdLine = (Get-Content "$W/run-1.stdout.txt") | Where-Object { $_ -match '^run_id=' } | Select-Object -First 1
$RunId = ($RunIdLine -split ' ')[0] -replace '^run_id=', ''

# (2) end-to-end analyze walls on copies of run-1 (analyzers rewrite files in place).
Copy-Item -Recurse "$W/run-1" "$W/copy-ds"
Copy-Item -Recurse "$W/run-1" "$W/copy-chain"
$DatasetWalls = @()
$ChainWalls = @()
for ($k = 1; $k -le 3; $k++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $Exe analyze --corpus "$W/copy-ds" --analyzers dataset --induce --withhold-tier2 --induction-rounds 2 2>"$W/ds-$k.stderr.txt" | Out-Null
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "analyze dataset run $k exit code $LASTEXITCODE" }
    $DatasetWalls += [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $Exe analyze --corpus "$W/copy-chain" --analyzers dataset,concepts --induce --withhold-tier2 --induction-rounds 2 2>"$W/chain-$k.stderr.txt" | Out-Null
    $sw.Stop()
    if ($LASTEXITCODE -ne 0) { throw "analyze dataset,concepts run $k exit code $LASTEXITCODE" }
    $ChainWalls += [math]::Round([double]$sw.Elapsed.TotalSeconds, 3)
}

# (3) parse the three induction_bench logs (generate-small corpus, pure library timings).
function Parse-Bench([string]$Path) {
    $r = [ordered]@{ rounds = @() }
    foreach ($line in (Get-Content $Path | Where-Object { $_ -match '^\[bench\] ' })) {
        if ($line -match '^\[bench\] induce-dataset rows=(\d+) columns=(\d+)') {
            $r.rows = [int]$Matches[1]; $r.columns = [int]$Matches[2]
        } elseif ($line -match '^\[bench\] induce ms=([\d.]+) rounds=(\d+) promoted=(\d+)') {
            $r.induce_ms = [double]$Matches[1]; $r.rounds_run = [int]$Matches[2]; $r.promoted = [int]$Matches[3]
        } elseif ($line -match '^\[bench\] induce-round round=(\d+) enumerated=(\d+) deduplicated=(\d+) scored=(\d+) passed=(\d+) shortlisted=(\d+) promoted=(\d+)') {
            $r.rounds += ,@([int]$Matches[1], [int]$Matches[2], [int]$Matches[3], [int]$Matches[4], [int]$Matches[5], [int]$Matches[6], [int]$Matches[7])
        } elseif ($line -match '^\[bench\] probe-fit engine=cart depth=(\d+) ms=([\d.]+) rules=(\d+)') {
            $r.probe_depth = [int]$Matches[1]; $r.probe_ms = [double]$Matches[2]; $r.probe_rules = [int]$Matches[3]
        }
    }
    return $r
}
$B = @(1, 2, 3 | ForEach-Object { Parse-Bench "target/m3/cb-bench-$_.txt" })
foreach ($Bn in $B) {
    if (($Bn.rounds | ConvertTo-Json -Compress) -ne ($B[0].rounds | ConvertTo-Json -Compress)) { throw 'bench round-count instability across runs' }
    if ($Bn.probe_rules -ne $B[0].probe_rules) { throw 'bench probe rules instability across runs' }
}
$SmallRounds = @()
foreach ($row in $B[0].rounds) {
    $SmallRounds += [ordered]@{ round = $row[0]; enumerated = $row[1]; deduplicated = $row[2]; scored = $row[3]; passed = $row[4]; shortlisted = $row[5]; promoted = $row[6] }
}

# (4) full-scale facts from run-1 documents (all deterministic).
$C = Get-Content -Raw "$W/run-1/concepts.json" | ConvertFrom-Json
$FullRounds = @()
foreach ($r in $C.rounds) {
    $FullRounds += [ordered]@{ round = [int]$r.round; columns = [int]$r.columns; enumerated = [int]$r.enumerated; deduplicated = [int]$r.deduplicated; scored = [int]$r.scored; passed = [int]$r.passed; shortlisted = @($r.shortlisted).Count; promoted = @($r.promoted).Count }
}
$D = Get-Content -Raw "$W/run-1/discover.json" | ConvertFrom-Json
$H = Get-Content -Raw "$W/run-1/heuristics.json" | ConvertFrom-Json
$Candidates = @()
foreach ($Cand in $D.candidates) {
    $M = $H.candidates | Where-Object { $_.name -eq $Cand.name } | Select-Object -First 1
    $Candidates += [ordered]@{
        name = $Cand.name; max_depth = [int]$M.max_depth; rules = [int]$M.rules; leaves = [int]$M.leaves
        fallback_rows = [int]$M.fallback_rows
        loss_rate_vs_reference = [double](F3 ([double]$Cand.loss_rate_vs_reference))
        agreement_rate = [double](F3 ([double]$Cand.agreement_rate))
        novelty = [double](F3 ([double]$Cand.novelty))
    }
}
$V = Get-Content -Raw "$W/run-1/vocabulary.json" | ConvertFrom-Json
$Fractions = [ordered]@{
    primitive = [double](F3 ([double]$V.overall.fractions.primitive))
    supplied = [double](F3 ([double]$V.overall.fractions.supplied))
    invented = [double](F3 ([double]$V.overall.fractions.invented))
}

$RustcVersion = (rustc --version).Trim()
$GitCommit = (git rev-parse HEAD).Trim()
$Date = Get-Date -Format 'yyyy-MM-dd'
$CommandField = 'cargo build --release; three `cargo test --release --test induction_bench -- --nocapture` runs; pwsh script target/m3/concept-bench.ps1 (reproduced verbatim in § Commands)'
$Header = [ordered]@{
    date = $Date; git_commit = $GitCommit; rustc = $RustcVersion
    os = 'Windows 11 Pro 10.0.26200'
    cpu = 'AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)'
    ram = '31 GiB'; build_profile = 'release'; command = $CommandField
}
$Result = [ordered]@{
    header = $Header
    full_scale = [ordered]@{
        discover_wall = [ordered]@{ runs = $DiscoverWalls; median_wall_s = Get-Median $DiscoverWalls; run_id = $RunId }
        analyze_walls = [ordered]@{
            dataset_runs_s = $DatasetWalls; dataset_median_s = Get-Median $DatasetWalls
            dataset_concepts_runs_s = $ChainWalls; dataset_concepts_median_s = Get-Median $ChainWalls
        }
        induction_rounds = $FullRounds
        promoted = @($C.promoted).Count
        candidates = $Candidates
        vocabulary_overall_fractions = $Fractions
    }
    generate_small = [ordered]@{
        rows = $B[0].rows; columns = $B[0].columns
        induce_ms_runs = @($B[0].induce_ms, $B[1].induce_ms, $B[2].induce_ms)
        induce_ms_median = Get-Median @($B[0].induce_ms, $B[1].induce_ms, $B[2].induce_ms)
        rounds = $SmallRounds
        probe_fit = [ordered]@{
            engine = 'cart'; depth = $B[0].probe_depth
            ms_runs = @($B[0].probe_ms, $B[1].probe_ms, $B[2].probe_ms)
            ms_median = Get-Median @($B[0].probe_ms, $B[1].probe_ms, $B[2].probe_ms)
            rules = $B[0].probe_rules
        }
    }
}
(($Result | ConvertTo-Json -Depth 10) + "`n") | Set-Content -NoNewline -Encoding utf8 "$W/concept-induction.json"

# (5) header + tables markdown, rendered from the same values.
$Hd = @('## Header', '', '| field | value |', '| --- | --- |')
foreach ($k in $Header.Keys) { $Hd += "| $k | $($Header[$k]) |" }
$Hd += ''
($Hd -join "`n") | Set-Content -NoNewline -Encoding utf8 "$W/header.md"

$Md = @()
$Md += '## Full-scale discover wall (configs/tictactoe-concepts.toml)'
$Md += ''
$Md += '| run | wall_s |'
$Md += '| --- | --- |'
for ($k = 0; $k -lt 3; $k++) { $Md += "| $($k + 1) | $(F3 $DiscoverWalls[$k]) |" }
$Md += ''
$Md += "median_wall_s: $(F3 (Get-Median $DiscoverWalls))"
$Md += ''
$Md += "run_id: $RunId"
$Md += ''
$Md += '## Full-scale analyze walls (end-to-end via analyze, copies of run-1)'
$Md += ''
$Md += '| analyzers | wall_s run 1 | wall_s run 2 | wall_s run 3 | median_s |'
$Md += '| --- | --- | --- | --- | --- |'
$Md += "| dataset | $(F3 $DatasetWalls[0]) | $(F3 $DatasetWalls[1]) | $(F3 $DatasetWalls[2]) | $(F3 (Get-Median $DatasetWalls)) |"
$Md += "| dataset,concepts | $(F3 $ChainWalls[0]) | $(F3 $ChainWalls[1]) | $(F3 $ChainWalls[2]) | $(F3 (Get-Median $ChainWalls)) |"
$Md += ''
$Md += '## Full-scale induction rounds (run-1 concepts.json)'
$Md += ''
$Md += '| round | columns | enumerated | deduplicated | scored | passed | shortlisted | promoted |'
$Md += '| --- | --- | --- | --- | --- | --- | --- | --- |'
foreach ($r in $FullRounds) { $Md += "| $($r.round) | $($r.columns) | $($r.enumerated) | $($r.deduplicated) | $($r.scored) | $($r.passed) | $($r.shortlisted) | $($r.promoted) |" }
$Md += ''
$Md += "promoted concepts: $(@($C.promoted).Count)"
$Md += ''
$Md += '## Candidates (full-scale discover run-1)'
$Md += ''
$Md += '| candidate | max_depth | rules | leaves | fallback_rows | loss_rate_vs_reference | agreement_rate | novelty |'
$Md += '| --- | --- | --- | --- | --- | --- | --- | --- |'
foreach ($Row in $Candidates) { $Md += "| $($Row.name) | $($Row.max_depth) | $($Row.rules) | $($Row.leaves) | $($Row.fallback_rows) | $(F3 $Row.loss_rate_vs_reference) | $(F3 $Row.agreement_rate) | $(F3 $Row.novelty) |" }
$Md += ''
$Md += '## Given-vs-discovered overall fractions (run-1 vocabulary.json)'
$Md += ''
$Md += '| tier | fraction |'
$Md += '| --- | --- |'
$Md += "| primitive | $(F3 $Fractions.primitive) |"
$Md += "| supplied | $(F3 $Fractions.supplied) |"
$Md += "| invented | $(F3 $Fractions.invented) |"
$Md += ''
$Md += '## Generate-small induction bench (pure library, tests/induction_bench.rs)'
$Md += ''
$Md += "rows: $($B[0].rows), columns: $($B[0].columns)"
$Md += ''
$Md += '| measure | ms run 1 | ms run 2 | ms run 3 | median_ms |'
$Md += '| --- | --- | --- | --- | --- |'
$Md += "| induce_concepts | $(F3 $B[0].induce_ms) | $(F3 $B[1].induce_ms) | $(F3 $B[2].induce_ms) | $(F3 (Get-Median @($B[0].induce_ms, $B[1].induce_ms, $B[2].induce_ms))) |"
$Md += "| probe-fit cart depth=$($B[0].probe_depth) (rules=$($B[0].probe_rules)) | $(F3 $B[0].probe_ms) | $(F3 $B[1].probe_ms) | $(F3 $B[2].probe_ms) | $(F3 (Get-Median @($B[0].probe_ms, $B[1].probe_ms, $B[2].probe_ms))) |"
$Md += ''
$Md += '| round | enumerated | deduplicated | scored | passed | shortlisted | promoted |'
$Md += '| --- | --- | --- | --- | --- | --- | --- |'
foreach ($r in $SmallRounds) { $Md += "| $($r.round) | $($r.enumerated) | $($r.deduplicated) | $($r.scored) | $($r.passed) | $($r.shortlisted) | $($r.promoted) |" }
$Md += ''
($Md -join "`n") | Set-Content -NoNewline -Encoding utf8 "$W/tables.md"
Write-Host 'concept-induction benchmark complete.'
```

## Full-scale discover wall (configs/tictactoe-concepts.toml)

| run | wall_s |
| --- | --- |
| 1 | 1.762 |
| 2 | 1.837 |
| 3 | 1.744 |

median_wall_s: 1.762

run_id: bd760ff0ace48705

## Full-scale analyze walls (end-to-end via analyze, copies of run-1)

| analyzers | wall_s run 1 | wall_s run 2 | wall_s run 3 | median_s |
| --- | --- | --- | --- | --- |
| dataset | 0.078 | 0.063 | 0.059 | 0.063 |
| dataset,concepts | 0.276 | 0.260 | 0.260 | 0.260 |

## Full-scale induction rounds (run-1 concepts.json)

| round | columns | enumerated | deduplicated | scored | passed | shortlisted | promoted |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 82 | 4537 | 757 | 3780 | 3749 | 8 | 1 |
| 2 | 83 | 4538 | 775 | 3763 | 3735 | 8 | 1 |

promoted concepts: 2

## Candidates (full-scale discover run-1)

| candidate | max_depth | rules | leaves | fallback_rows | loss_rate_vs_reference | agreement_rate | novelty |
| --- | --- | --- | --- | --- | --- | --- | --- |
| mined-d6-l1 | 6 | 41 | 41 | 0 | 0.425 | 0.781 | 1.000 |
| mined-d8-l1 | 8 | 84 | 85 | 2 | 0.475 | 0.897 | 0.336 |
| mined-d12-l1 | 12 | 149 | 151 | 3 | 0.000 | 0.976 | 0.184 |
| mined-d0-l1 | 0 | 173 | 175 | 3 | 0.000 | 0.993 | 0.031 |

## Given-vs-discovered overall fractions (run-1 vocabulary.json)

| tier | fraction |
| --- | --- |
| primitive | 0.980 |
| supplied | 0.000 |
| invented | 0.020 |

## Generate-small induction bench (pure library, tests/induction_bench.rs)

rows: 190, columns: 82

| measure | ms run 1 | ms run 2 | ms run 3 | median_ms |
| --- | --- | --- | --- | --- |
| induce_concepts | 28.300 | 28.600 | 28.400 | 28.400 |
| probe-fit cart depth=6 (rules=30) | 1.600 | 1.500 | 1.500 | 1.500 |

| round | enumerated | deduplicated | scored | passed | shortlisted | promoted |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 999 | 266 | 733 | 716 | 4 | 1 |
| 2 | 1000 | 274 | 726 | 709 | 4 | 0 |
