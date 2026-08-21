## Header

| field | value |
| --- | --- |
| date | 2026-08-21 |
| git_commit | e2ac5174da1b75f0899c428db074bff53ab28ca0 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | release |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/gate-b/determinism.ps1 (script reproduced in § Commands; binary target/release/strategy-discovery.exe built by `cargo build --release`; hashes by Get-FileHash -Algorithm SHA256) |

## Config

`configs/tictactoe-default.toml` with `games_per_cell` raised from 20 to 200 (49 ordered roster pairings × 200 games = 9800 games), written by the script to `target/gate-b/determinism/tictactoe-200.toml`:

```toml
schema_version = 1
game = "tictactoe"
seed = 20260821
games_per_cell = 200
```

- run_id: dea7db96340e923d (hash of the resolved config; identical for every generate run below)

## Commands

```powershell
# Gate B determinism: SHA-256 identity of every pipeline output across repeat runs (release CLI).
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Exe = Join-Path $Root 'target\release\strategy-discovery.exe'
$Work = Join-Path $Root 'target\gate-b\determinism'
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

function Invoke-Cli([string[]]$CliArgs) {
    $out = & $Exe @CliArgs
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for: $($CliArgs -join ' ')" }
    Write-Host ('> strategy-discovery {0}' -f ($CliArgs -join ' '))
    Write-Host ('  {0}' -f ($out -join ' | '))
}

# (a) same seed twice, default parallel execution (rayon global pool)
Invoke-Cli @('generate', '--config', $Config, '--out', (Join-Path $Work 'same-seed-a'))
Invoke-Cli @('generate', '--config', $Config, '--out', (Join-Path $Work 'same-seed-b'))
# (b) --serial vs --threads 4 vs default
Invoke-Cli @('generate', '--config', $Config, '--out', (Join-Path $Work 'serial'), '--serial')
Invoke-Cli @('generate', '--config', $Config, '--out', (Join-Path $Work 'threads-4'), '--threads', '4')
# (c) annotate twice on two copies of the same corpus, (e) analyze twice on the same copies
foreach ($i in 1, 2) {
    $d = Join-Path $Work "annotate-$i"
    New-Item -ItemType Directory -Force $d | Out-Null
    foreach ($f in 'run.json', 'games.jsonl', 'positions.jsonl') { Copy-Item (Join-Path $Work "same-seed-a\$f") (Join-Path $d $f) }
    Invoke-Cli @('annotate', '--corpus', $d)
    Invoke-Cli @('analyze', '--corpus', $d)
}
# (d) exhaustive annotation twice
foreach ($i in 1, 2) { Invoke-Cli @('annotate', '--exhaustive', '--game', 'tictactoe', '--out', (Join-Path $Work "exhaustive-$i")) }

Write-Host '--- SHA-256 (Get-FileHash -Algorithm SHA256) ---'
foreach ($d in 'same-seed-a', 'same-seed-b', 'serial', 'threads-4', 'annotate-1', 'annotate-2', 'exhaustive-1', 'exhaustive-2') {
    Get-ChildItem (Join-Path $Work $d) -File | Sort-Object Name | Get-FileHash -Algorithm SHA256 |
        ForEach-Object { Write-Host ('{0} {1} {2} bytes={3}' -f $d, (Split-Path $_.Path -Leaf), $_.Hash.ToLower(), (Get-Item $_.Path).Length) }
}

function Test-Same([string[]]$Dirs, [string[]]$Files) {
    $ok = $true
    foreach ($f in $Files) {
        $hashes = @($Dirs | ForEach-Object { (Get-FileHash -Algorithm SHA256 (Join-Path $Work "$_\$f")).Hash } | Select-Object -Unique)
        if ($hashes.Count -ne 1) { $ok = $false }
    }
    if ($ok) { return 'YES' } else { return 'NO' }
}
Write-Host '--- verdicts ---'
Write-Host ('(a) same-seed identical: {0}' -f (Test-Same @('same-seed-a', 'same-seed-b') @('run.json', 'games.jsonl', 'positions.jsonl')))
Write-Host ('(b) serial-vs-threads-4-vs-parallel identical: {0}' -f (Test-Same @('serial', 'threads-4', 'same-seed-a') @('run.json', 'games.jsonl', 'positions.jsonl')))
Write-Host ('(c) annotate x2 identical: {0}' -f (Test-Same @('annotate-1', 'annotate-2') @('annotations.jsonl', 'annotate.json')))
Write-Host ('(d) exhaustive x2 identical: {0}' -f (Test-Same @('exhaustive-1', 'exhaustive-2') @('annotations.jsonl', 'annotate.json')))
Write-Host ('(e) analyze x2 identical: {0}' -f (Test-Same @('annotate-1', 'annotate-2') @('summary.json')))
```

## Runs

| run | command | stdout |
| --- | --- | --- |
| same-seed-a | generate --config tictactoe-200.toml --out same-seed-a | run_id=dea7db96340e923d cells=49 games=9800 positions=75004 out=C:\Users\John\Projects\worktrees\phase5-corpus-21-determinism\target\gate-b\determinism\same-seed-a |
| same-seed-b | generate --config tictactoe-200.toml --out same-seed-b | run_id=dea7db96340e923d cells=49 games=9800 positions=75004 out=C:\Users\John\Projects\worktrees\phase5-corpus-21-determinism\target\gate-b\determinism\same-seed-b |
| serial | generate --config tictactoe-200.toml --out serial --serial | run_id=dea7db96340e923d cells=49 games=9800 positions=75004 out=C:\Users\John\Projects\worktrees\phase5-corpus-21-determinism\target\gate-b\determinism\serial |
| threads-4 | generate --config tictactoe-200.toml --out threads-4 --threads 4 | run_id=dea7db96340e923d cells=49 games=9800 positions=75004 out=C:\Users\John\Projects\worktrees\phase5-corpus-21-determinism\target\gate-b\determinism\threads-4 |
| annotate-1 | annotate --corpus annotate-1 (copy of same-seed-a) | mode=corpus annotated=2882 terminal=0 disagreements=0 |
| analyze-1 | analyze --corpus annotate-1 | games=9800 distinct_games=3225 canonical_coverage=0.935 decisive_fraction=0.510 diversity_pass=false |
| annotate-2 | annotate --corpus annotate-2 (copy of same-seed-a) | mode=corpus annotated=2882 terminal=0 disagreements=0 |
| analyze-2 | analyze --corpus annotate-2 | games=9800 distinct_games=3225 canonical_coverage=0.935 decisive_fraction=0.510 diversity_pass=false |
| exhaustive-1 | annotate --exhaustive --game tictactoe --out exhaustive-1 | mode=exhaustive annotated=5478 terminal=958 disagreements=0 |
| exhaustive-2 | annotate --exhaustive --game tictactoe --out exhaustive-2 | mode=exhaustive annotated=5478 terminal=958 disagreements=0 |

## SHA-256

| run | file | bytes | sha256 |
| --- | --- | --- | --- |
| same-seed-a | games.jsonl | 6161570 | 83fccbbb0958c10a86afb3bc0c671dbee5d9f2ea5b4ce35da20b49859b0c1ab6 |
| same-seed-a | positions.jsonl | 28003228 | ff3eac9480bdd976ca324e92b070c791e9f54e9c02689e6598311848bdfa782e |
| same-seed-a | run.json | 13645 | 9e2e9da7f7e6a99f247afac803fb284e19a8e73cc651e5da40b59bc4d42a65ee |
| same-seed-b | games.jsonl | 6161570 | 83fccbbb0958c10a86afb3bc0c671dbee5d9f2ea5b4ce35da20b49859b0c1ab6 |
| same-seed-b | positions.jsonl | 28003228 | ff3eac9480bdd976ca324e92b070c791e9f54e9c02689e6598311848bdfa782e |
| same-seed-b | run.json | 13645 | 9e2e9da7f7e6a99f247afac803fb284e19a8e73cc651e5da40b59bc4d42a65ee |
| serial | games.jsonl | 6161570 | 83fccbbb0958c10a86afb3bc0c671dbee5d9f2ea5b4ce35da20b49859b0c1ab6 |
| serial | positions.jsonl | 28003228 | ff3eac9480bdd976ca324e92b070c791e9f54e9c02689e6598311848bdfa782e |
| serial | run.json | 13645 | 9e2e9da7f7e6a99f247afac803fb284e19a8e73cc651e5da40b59bc4d42a65ee |
| threads-4 | games.jsonl | 6161570 | 83fccbbb0958c10a86afb3bc0c671dbee5d9f2ea5b4ce35da20b49859b0c1ab6 |
| threads-4 | positions.jsonl | 28003228 | ff3eac9480bdd976ca324e92b070c791e9f54e9c02689e6598311848bdfa782e |
| threads-4 | run.json | 13645 | 9e2e9da7f7e6a99f247afac803fb284e19a8e73cc651e5da40b59bc4d42a65ee |
| annotate-1 | annotate.json | 233 | ce205808b39dac17600b1f9f991d0d7ec3ee85ee07b877ca0167276ddad40b3c |
| annotate-1 | annotations.jsonl | 829583 | 5ff171e17b83acdb8774b4ab31a57c555b11f40e0f29476521dc484b1e2fdadd |
| annotate-1 | games.jsonl | 6161570 | 83fccbbb0958c10a86afb3bc0c671dbee5d9f2ea5b4ce35da20b49859b0c1ab6 |
| annotate-1 | positions.jsonl | 28003228 | ff3eac9480bdd976ca324e92b070c791e9f54e9c02689e6598311848bdfa782e |
| annotate-1 | run.json | 13645 | 9e2e9da7f7e6a99f247afac803fb284e19a8e73cc651e5da40b59bc4d42a65ee |
| annotate-1 | summary.json | 9267 | af4fba323c56bdfdcee9765bbeabf82a56f0c668be382034d7b82d396aebb1aa |
| annotate-2 | annotate.json | 233 | ce205808b39dac17600b1f9f991d0d7ec3ee85ee07b877ca0167276ddad40b3c |
| annotate-2 | annotations.jsonl | 829583 | 5ff171e17b83acdb8774b4ab31a57c555b11f40e0f29476521dc484b1e2fdadd |
| annotate-2 | games.jsonl | 6161570 | 83fccbbb0958c10a86afb3bc0c671dbee5d9f2ea5b4ce35da20b49859b0c1ab6 |
| annotate-2 | positions.jsonl | 28003228 | ff3eac9480bdd976ca324e92b070c791e9f54e9c02689e6598311848bdfa782e |
| annotate-2 | run.json | 13645 | 9e2e9da7f7e6a99f247afac803fb284e19a8e73cc651e5da40b59bc4d42a65ee |
| annotate-2 | summary.json | 9267 | af4fba323c56bdfdcee9765bbeabf82a56f0c668be382034d7b82d396aebb1aa |
| exhaustive-1 | annotate.json | 225 | 1356da2e7f00a16a443085cc32edbc12ae0435afcb9e28bc49732ef54384f353 |
| exhaustive-1 | annotations.jsonl | 1570396 | 777c843e3d246dbb1f99316b784ab0d9b30c40b5dee77645a2d1f7a2192e32f1 |
| exhaustive-2 | annotate.json | 225 | 1356da2e7f00a16a443085cc32edbc12ae0435afcb9e28bc49732ef54384f353 |
| exhaustive-2 | annotations.jsonl | 1570396 | 777c843e3d246dbb1f99316b784ab0d9b30c40b5dee77645a2d1f7a2192e32f1 |

## Result

- (a) same seed twice — same-seed-a vs same-seed-b: run.json, games.jsonl, positions.jsonl — identical: YES
- (b) --serial vs --threads 4 vs default pool — serial, threads-4, same-seed-a: run.json, games.jsonl, positions.jsonl — identical: YES
- (c) annotate twice — annotate-1 vs annotate-2: annotations.jsonl, annotate.json — identical: YES
- (d) annotate --exhaustive twice — exhaustive-1 vs exhaustive-2: annotations.jsonl, annotate.json — identical: YES
- (e) analyze twice — annotate-1 vs annotate-2: summary.json — identical: YES
- all identical: YES
