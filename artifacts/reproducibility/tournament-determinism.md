## Header

| field | value |
| --- | --- |
| date | 2026-08-23 |
| git_commit | 5b5bcc8141d29ef58242646e166333129db044ef |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | debug |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/m1/tournament-determinism.ps1 (script reproduced verbatim in § Commands; binary target/debug/strategy-discovery.exe built by `cargo build`; hashes by Get-FileHash -Algorithm SHA256) |

## Scope

This evidence claims byte-identity of every `evaluate` output across repeat runs, scheduling modes, and a rerun against an existing archive, for the full command under test: `evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out <dir>` (default `--games 20`, `--seed 0`, roster `ttt-benchmark-v1`, 3 strategies x 7 opponents x 2 seats x 20 games = 280 games per strategy). stdout is compared with the ` out=<dir>` suffix of the final line removed, since it echoes the user-supplied `--out` flag and is the only token allowed to differ between runs into different directories. The exhaustive tic-tac-toe annotation used as the agreement input is produced by the script itself and is not checked in. The five claims:

- (a) `evaluate` twice into fresh `--out` directories produces identical `evaluation.json`, `archive/archive.json`, and `archive/entries.jsonl`.
- (b) stdout is identical across those runs.
- (c) `--serial` vs `--threads 4` vs the default rayon pool produce identical files and stdout.
- (d) a third run into run-1's EXISTING `--out` appends nothing (entries.jsonl line count unchanged; all three files and stdout identical).
- (e) `-v` produces identical files and stdout.

## Commands

```powershell
# Milestone 1 tournament determinism: SHA-256 identity of every `evaluate` output (evaluation.json,
# archive/archive.json, archive/entries.jsonl, stdout) across repeat runs, scheduling modes, a rerun
# against the existing archive, and -v. Debug binary (`cargo build`). Scratch under target/m1/.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $Root
$Exe = Join-Path $Root 'target\debug\strategy-discovery.exe'
$W = 'target/m1/determinism'
if (Test-Path $W) { Remove-Item -Recurse -Force $W }
New-Item -ItemType Directory -Force $W | Out-Null

function Invoke-Cli([string[]]$CliArgs) {
    $out = & $Exe @CliArgs 2> "$W/stderr.txt"
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for: $($CliArgs -join ' ')" }
    Write-Host ('> strategy-discovery {0}' -f ($CliArgs -join ' '))
    foreach ($line in $out) { Write-Host ('  {0}' -f $line) }
    return $out
}

# stdout hash with the `out=<dir>` echo of the user-supplied --out flag removed (it is the only
# token allowed to differ between runs into different directories).
function Get-StdoutSha([string[]]$Lines) {
    $norm = $Lines | ForEach-Object { $_ -replace ' out=.*$', '' }
    $bytes = [System.Text.Encoding]::UTF8.GetBytes(($norm -join "`n"))
    $sha = [System.Security.Cryptography.SHA256]::Create()
    -join ($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') })
}

$Files = @('evaluation.json', 'archive/archive.json', 'archive/entries.jsonl')
function Get-FileShas([string]$Run) {
    $h = [ordered]@{}
    foreach ($f in $Files) { $h[$f] = (Get-FileHash -Algorithm SHA256 (Join-Path $Root "$W/$Run/$f")).Hash.ToLower() }
    return $h
}
function Test-Same([string[]]$Runs) {
    foreach ($f in $Files) {
        $hashes = @($Runs | ForEach-Object { (Get-FileHash -Algorithm SHA256 (Join-Path $Root "$W/$_/$f")).Hash } | Select-Object -Unique)
        if ($hashes.Count -ne 1) { return 'NO' }
    }
    return 'YES'
}

# Exhaustive annotation: the agreement input (never checked in; produced here).
Invoke-Cli @('annotate', '--exhaustive', '--game', 'tictactoe', '--out', "$W/exhaustive") | Out-Null

$Base = @('evaluate', '--game', 'tictactoe', '--strategies', 'tests/fixtures/strategies-eval.toml', '--annotations', "$W/exhaustive")
$Stdout = @{}
# (a)(b) the same command twice into fresh directories
$Stdout['run-1'] = Invoke-Cli ($Base + @('--out', "$W/run-1"))
$Stdout['run-2'] = Invoke-Cli ($Base + @('--out', "$W/run-2"))
# (c) scheduling modes into fresh directories
$Stdout['serial'] = Invoke-Cli ($Base + @('--out', "$W/serial", '--serial'))
$Stdout['threads-4'] = Invoke-Cli ($Base + @('--out', "$W/threads-4", '--threads', '4'))
# (d) a third run into run-1's directory: the archive already holds every entry
$EntriesBefore = ([System.IO.File]::ReadLines((Join-Path $Root "$W/run-1/archive/entries.jsonl")) | Measure-Object).Count
$ShaBefore = Get-FileShas 'run-1'
$Stdout['run-1-again'] = Invoke-Cli ($Base + @('--out', "$W/run-1"))
$EntriesAfter = ([System.IO.File]::ReadLines((Join-Path $Root "$W/run-1/archive/entries.jsonl")) | Measure-Object).Count
$ShaAfter = Get-FileShas 'run-1'
# (e) -v into a fresh directory (logs go to stderr; stdout must not change)
$Stdout['verbose'] = Invoke-Cli (@('-v') + $Base + @('--out', "$W/verbose"))

Write-Host '--- SHA-256 (Get-FileHash -Algorithm SHA256) ---'
foreach ($r in 'run-1', 'run-2', 'serial', 'threads-4', 'verbose') {
    foreach ($f in $Files) {
        $p = Join-Path $Root "$W/$r/$f"
        Write-Host ('{0} {1} bytes={2} {3}' -f $r, $f, (Get-Item $p).Length, (Get-FileHash -Algorithm SHA256 $p).Hash.ToLower())
    }
}
Write-Host '--- stdout SHA-256 (out= echo removed) ---'
foreach ($k in 'run-1', 'run-2', 'serial', 'threads-4', 'run-1-again', 'verbose') { Write-Host ('{0} {1}' -f $k, (Get-StdoutSha $Stdout[$k])) }
Write-Host ('run-1 entries.jsonl lines before={0} after={1}' -f $EntriesBefore, $EntriesAfter)

$SameRerun = 'YES'
foreach ($f in $Files) { if ($ShaBefore[$f] -ne $ShaAfter[$f]) { $SameRerun = 'NO' } }
if ($EntriesBefore -ne $EntriesAfter) { $SameRerun = 'NO' }
if ((Get-StdoutSha $Stdout['run-1-again']) -ne (Get-StdoutSha $Stdout['run-1'])) { $SameRerun = 'NO' }
function Test-SameStdout([string[]]$Keys) {
    $u = @($Keys | ForEach-Object { Get-StdoutSha $Stdout[$_] } | Select-Object -Unique)
    if ($u.Count -eq 1) { 'YES' } else { 'NO' }
}
Write-Host '--- verdicts ---'
Write-Host ('(a) evaluate twice into fresh --out directories, files identical: {0}' -f (Test-Same @('run-1', 'run-2')))
Write-Host ('(b) evaluate twice, stdout identical: {0}' -f (Test-SameStdout @('run-1', 'run-2')))
Write-Host ('(c) --serial vs --threads 4 vs default pool, files and stdout identical: {0}' -f $(if ((Test-Same @('run-1', 'serial', 'threads-4')) -eq 'YES' -and (Test-SameStdout @('run-1', 'serial', 'threads-4')) -eq 'YES') { 'YES' } else { 'NO' }))
Write-Host ('(d) third run against the existing archive appends nothing, files and stdout identical: {0}' -f $SameRerun)
Write-Host ('(e) -v run, files and stdout identical: {0}' -f $(if ((Test-Same @('run-1', 'verbose')) -eq 'YES' -and (Test-SameStdout @('run-1', 'verbose')) -eq 'YES') { 'YES' } else { 'NO' }))
```

## Runs

`run-1 entries.jsonl lines before=3 after=3`, confirming that the third run into run-1's existing `--out` appended no new archive entries.

| run | command | stdout |
| --- | --- | --- |
| exhaustive | annotate --exhaustive --game tictactoe --out target/m1/determinism/exhaustive | mode=exhaustive annotated=5478 terminal=958 disagreements=0 |
| run-1 | evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out target/m1/determinism/run-1 | strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000 ; strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535 ; strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051 ; evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/determinism/run-1 |
| run-2 | evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out target/m1/determinism/run-2 | strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000 ; strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535 ; strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051 ; evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/determinism/run-2 |
| serial | evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out target/m1/determinism/serial --serial | strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000 ; strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535 ; strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051 ; evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/determinism/serial |
| threads-4 | evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out target/m1/determinism/threads-4 --threads 4 | strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000 ; strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535 ; strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051 ; evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/determinism/threads-4 |
| run-1-again | evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out target/m1/determinism/run-1 | strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000 ; strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535 ; strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051 ; evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/determinism/run-1 |
| verbose | -v evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/determinism/exhaustive --out target/m1/determinism/verbose | strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000 ; strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535 ; strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051 ; evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/determinism/verbose |

## SHA-256

| run | file | bytes | sha256 |
| --- | --- | --- | --- |
| run-1 | evaluation.json | 38924 | 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a |
| run-1 | archive/archive.json | 429 | 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a |
| run-1 | archive/entries.jsonl | 15448 | 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8 |
| run-2 | evaluation.json | 38924 | 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a |
| run-2 | archive/archive.json | 429 | 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a |
| run-2 | archive/entries.jsonl | 15448 | 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8 |
| serial | evaluation.json | 38924 | 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a |
| serial | archive/archive.json | 429 | 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a |
| serial | archive/entries.jsonl | 15448 | 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8 |
| threads-4 | evaluation.json | 38924 | 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a |
| threads-4 | archive/archive.json | 429 | 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a |
| threads-4 | archive/entries.jsonl | 15448 | 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8 |
| verbose | evaluation.json | 38924 | 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a |
| verbose | archive/archive.json | 429 | 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a |
| verbose | archive/entries.jsonl | 15448 | 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8 |

| run | stdout sha256 |
| --- | --- |
| run-1 | 0f9a17a480545f555d3b0ff7791f162be48486e000029b65ab23f1234a3843ab |
| run-2 | 0f9a17a480545f555d3b0ff7791f162be48486e000029b65ab23f1234a3843ab |
| serial | 0f9a17a480545f555d3b0ff7791f162be48486e000029b65ab23f1234a3843ab |
| threads-4 | 0f9a17a480545f555d3b0ff7791f162be48486e000029b65ab23f1234a3843ab |
| run-1-again | 0f9a17a480545f555d3b0ff7791f162be48486e000029b65ab23f1234a3843ab |
| verbose | 0f9a17a480545f555d3b0ff7791f162be48486e000029b65ab23f1234a3843ab |

## Result

- (a) evaluate twice into fresh --out directories, files identical: YES
- (b) evaluate twice, stdout identical: YES
- (c) --serial vs --threads 4 vs default pool, files and stdout identical: YES
- (d) third run against the existing archive appends nothing, files and stdout identical: YES
- (e) -v run, files and stdout identical: YES

all identical: YES
