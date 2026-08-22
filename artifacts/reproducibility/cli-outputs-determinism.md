## Header

| field | value |
| --- | --- |
| date | 2026-08-22 |
| git_commit | bb25b90b83af0d040622744f0bcb7dc85560108d |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | debug |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/phase6/cli-determinism.ps1 (script reproduced verbatim in § Commands; binary target/debug/strategy-discovery.exe built by `cargo build`; hashes by Get-FileHash -Algorithm SHA256) |

## Scope

This evidence claims byte-identity between repeat runs of the same command, and between `play --out` and `generate` on the equivalent sweep:

- `play --out` twice.
- `play --out` versus `generate` on the equivalent sweep.
- the `generate -> annotate -> analyze (summary + agreement) -> report` chain twice.
- `play` and `report` stdout twice.
- `pipeline` twice.
- re-verification of the pinned earlier-phase (Phase 5) SHA-256 hashes.

`small-*` and `pipe-*` are expected to differ from each other on `summary.json`, `analyze.json` and `report.md` (3227 vs 3195 bytes for `report.md`), because the `tests/fixtures/experiment-small.toml` experiment config sets its own diversity thresholds (0.25 / 0.2 / 0.5) while the standalone `analyze` invocation uses the defaults (0.5 / 0.2 / 0.5). This is not a determinism failure; determinism is claimed only between the two runs of the same command.

## Commands

```powershell
# Phase 6 CLI determinism: SHA-256 identity of every CLI output across repeat runs, plus
# re-verification of the pinned Phase 5 hashes. Debug binary (`cargo build`).
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $Root
$Exe = Join-Path $Root 'target\debug\strategy-discovery.exe'
$W = 'target/phase6/determinism'
if (Test-Path $W) { Remove-Item -Recurse -Force $W }
New-Item -ItemType Directory -Force $W | Out-Null

function Invoke-Cli([string[]]$CliArgs) {
    $out = & $Exe @CliArgs
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for: $($CliArgs -join ' ')" }
    Write-Host ('> strategy-discovery {0}' -f ($CliArgs -join ' '))
    Write-Host ('  {0}' -f ($out -join ' | '))
    return $out
}

function Get-StringSha([string[]]$Lines) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))
    $sha = [System.Security.Cryptography.SHA256]::Create()
    -join ($sha.ComputeHash($bytes) | ForEach-Object { $_.ToString('x2') })
}

# The sweep whose generate output must equal `play --out` byte for byte.
Set-Content -Path "$W/equivalent.toml" -Encoding ascii -Value @(
    'schema_version = 1',
    'game = "tictactoe"',
    'seed = 7',
    'games_per_cell = 2',
    'evaluators = ["default"]',
    'pairings = [["random", "perfect"]]',
    'random_opening_plies = [0]',
    '',
    '[[strategies]]',
    'name = "random"',
    '[strategies.spec]',
    'kind = "random"',
    '',
    '[[strategies]]',
    'name = "perfect"',
    '[strategies.spec]',
    'kind = "minimax"',
    'depth = 9',
    '',
    '[[openings]]',
    'name = "play"'
)

# (a) play twice, (b) generate on the equivalent sweep
$playOut = @{}
foreach ($i in 1, 2) {
    $playOut[$i] = Invoke-Cli @('play', '--game', 'tictactoe', '--players', 'random,perfect', '--games', '2', '--seed', '7', '--out', "$W/play-$i")
}
Invoke-Cli @('generate', '--config', "$W/equivalent.toml", '--out', "$W/gen-equiv") | Out-Null

# (c)(d) full chain twice on the small fixture
$reportOut = @{}
foreach ($i in 1, 2) {
    Invoke-Cli @('generate', '--config', 'tests/fixtures/generate-small.toml', '--out', "$W/small-$i") | Out-Null
    Invoke-Cli @('annotate', '--corpus', "$W/small-$i") | Out-Null
    Invoke-Cli @('analyze', '--corpus', "$W/small-$i", '--analyzers', 'summary,agreement') | Out-Null
    $reportOut[$i] = Invoke-Cli @('report', '--input', "$W/small-$i", '--out', "$W/small-$i/report.md")
}

# (e) pipeline twice
foreach ($i in 1, 2) {
    Invoke-Cli @('pipeline', '--experiment', 'tests/fixtures/experiment-small.toml', '--out', "$W/pipe-$i") | Out-Null
}

# (f) pinned run_ids from the other two checked-in sweeps
Invoke-Cli @('generate', '--config', 'tests/fixtures/generate-draws.toml', '--out', "$W/draws") | Out-Null
Invoke-Cli @('generate', '--config', 'configs/tictactoe-default.toml', '--out', "$W/default") | Out-Null

Write-Host '--- SHA-256 (Get-FileHash -Algorithm SHA256) ---'
foreach ($d in 'play-1', 'play-2', 'gen-equiv', 'small-1', 'small-2', 'pipe-1', 'pipe-2') {
    Get-ChildItem (Join-Path $Root $d.Insert(0, "$W/")) -File | Sort-Object Name | Get-FileHash -Algorithm SHA256 |
        ForEach-Object { Write-Host ('{0} {1} {2} bytes={3}' -f $d, (Split-Path $_.Path -Leaf), $_.Hash.ToLower(), (Get-Item $_.Path).Length) }
}
Write-Host ('play stdout sha256: run1={0} run2={1}' -f (Get-StringSha $playOut[1]), (Get-StringSha $playOut[2]))
Write-Host ('report stdout sha256: run1={0} run2={1}' -f (Get-StringSha $reportOut[1]), (Get-StringSha $reportOut[2]))

function Test-Same([string[]]$Dirs, [string[]]$Files) {
    foreach ($f in $Files) {
        $hashes = @($Dirs | ForEach-Object { (Get-FileHash -Algorithm SHA256 (Join-Path $Root "$W/$_/$f")).Hash } | Select-Object -Unique)
        if ($hashes.Count -ne 1) { return 'NO' }
    }
    return 'YES'
}
$corpusFiles = @('run.json', 'games.jsonl', 'positions.jsonl')
$chainFiles = @('run.json', 'games.jsonl', 'positions.jsonl', 'annotations.jsonl', 'annotate.json', 'summary.json', 'agreement.json', 'analyze.json', 'report.md')
Write-Host '--- verdicts ---'
Write-Host ('(a) play --out twice, identical: {0}' -f (Test-Same @('play-1', 'play-2') $corpusFiles))
Write-Host ('(b) play --out vs generate on the equivalent sweep, identical: {0}' -f (Test-Same @('play-1', 'gen-equiv') $corpusFiles))
Write-Host ('(c) generate/annotate/analyze/report chain twice, identical: {0}' -f (Test-Same @('small-1', 'small-2') $chainFiles))
Write-Host ('(d) play stdout twice, identical: {0}' -f $(if ((Get-StringSha $playOut[1]) -eq (Get-StringSha $playOut[2])) { 'YES' } else { 'NO' }))
Write-Host ('(e) report stdout twice, identical: {0}' -f $(if ((Get-StringSha $reportOut[1]) -eq (Get-StringSha $reportOut[2])) { 'YES' } else { 'NO' }))
Write-Host ('(f) pipeline twice, identical: {0}' -f (Test-Same @('pipe-1', 'pipe-2') $chainFiles))

Write-Host '--- pinned Phase 5 hashes (expected -> actual) ---'
$pins = [ordered]@{
    'run.json'          = 'a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1'
    'games.jsonl'       = 'd0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d'
    'positions.jsonl'   = 'cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7'
    'annotations.jsonl' = '9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3'
    'annotate.json'     = '65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e'
    'summary.json'      = '5794b37e9fe1f3ee786092160298a9435092c47dc4dc9a2b233d0c49e93d24d5'
}
$allPinned = $true
foreach ($f in $pins.Keys) {
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $Root "$W/small-1/$f")).Hash.ToLower()
    $ok = ($actual -eq $pins[$f])
    if (-not $ok) { $allPinned = $false }
    Write-Host ('{0} pinned={1} actual={2} match={3}' -f $f, $pins[$f], $actual, $(if ($ok) { 'YES' } else { 'NO' }))
}
Write-Host ('all pinned Phase 5 hashes reproduce: {0}' -f $(if ($allPinned) { 'YES' } else { 'NO' }))
```

## Runs

Pipe characters (`|`) inside a command's own stdout (JSONL-adjacent move notation, or the Markdown table syntax embedded in `report`'s stdout) are replaced with `;` in the stdout cell below so they cannot be read as extra table columns; `report`'s stdout is the full rendered report body it prints in addition to writing `--out`.

| run | command | stdout |
| --- | --- | --- |
| play-1 | play --game tictactoe --players random,perfect --games 2 --seed 7 --out target/phase6/determinism/play-1 | game 0:0 seed=0x9d20c9ebd5dcd11a players=random,perfect opening_plies=0 ;  0. X 7 ;  1. O 8 ;  2. X 4 ;  3. O 1 ;  4. X 0 ;  5. O 6 ;  6. X 2 ;  7. O 3 ;  8. X 5 ;   XOX ;   OXX ;   OXO ; outcome=Draw length=9 ;  ; game 0:1 seed=0xd4c1951159fc36d5 players=random,perfect opening_plies=0 ;  0. X 4 ;  1. O 0 ;  2. X 8 ;  3. O 6 ;  4. X 7 ;  5. O 3 ;   O.. ;   OX. ;   OXX ; outcome=Win(O) length=6 ;  ; games=2 wins=X:0,O:1 draws=1 unfinished=0 |
| play-2 | play --game tictactoe --players random,perfect --games 2 --seed 7 --out target/phase6/determinism/play-2 | game 0:0 seed=0x9d20c9ebd5dcd11a players=random,perfect opening_plies=0 ;  0. X 7 ;  1. O 8 ;  2. X 4 ;  3. O 1 ;  4. X 0 ;  5. O 6 ;  6. X 2 ;  7. O 3 ;  8. X 5 ;   XOX ;   OXX ;   OXO ; outcome=Draw length=9 ;  ; game 0:1 seed=0xd4c1951159fc36d5 players=random,perfect opening_plies=0 ;  0. X 4 ;  1. O 0 ;  2. X 8 ;  3. O 6 ;  4. X 7 ;  5. O 3 ;   O.. ;   OX. ;   OXX ; outcome=Win(O) length=6 ;  ; games=2 wins=X:0,O:1 draws=1 unfinished=0 |
| gen-equiv | generate --config target/phase6/determinism/equivalent.toml --out target/phase6/determinism/gen-equiv | run_id=988795b19ac38e3c cells=1 games=2 positions=15 out=target/phase6/determinism/gen-equiv |
| small-1 | generate --config tests/fixtures/generate-small.toml --out target/phase6/determinism/small-1 | run_id=5eafa65f76637df3 cells=36 games=144 positions=1083 out=target/phase6/determinism/small-1 |
| small-1 | annotate --corpus target/phase6/determinism/small-1 | mode=corpus annotated=390 terminal=0 disagreements=0 |
| small-1 | analyze --corpus target/phase6/determinism/small-1 --analyzers summary,agreement | games=144 distinct_games=115 canonical_coverage=0.312 decisive_fraction=0.549 diversity_pass=false |
| small-1 | report --input target/phase6/determinism/small-1 --out target/phase6/determinism/small-1/report.md | # Run 5eafa65f76637df3 ;  ; ; field ; value ; ; ; --- ; --- ; ; ; game ; tictactoe ; ; ; crate_name ; strategy-discovery ; ; ; crate_version ; 0.1.0 ; ; ; seed ; 20260821 ; ; ; games_per_cell ; 4 ; ; ; cells ; 36 ; ; ; games ; 144 ; ; ; positions ; 1083 ; ; ; strategies ; random, depth-2, perfect ; ; ; pairings ; 9 ; ; ; evaluators ; default ; ; ; openings ; none, center ; ; ; random_opening_plies ; 0, 2 ; ;  ; ## Annotation ;  ; ; field ; value ; ; ; --- ; --- ; ; ; mode ; Corpus ; ; ; engine_depth ; 9 ; ; ; evaluator ; default ; ; ; annotated ; 390 ; ; ; terminal ; 0 ; ; ; disagreements ; 0 ; ; ; solver_states ; 5478 ; ;  ; ## Summary ;  ; ; metric ; value ; ; ; --- ; --- ; ; ; run_id ; 5eafa65f76637df3 ; ; ; game ; tictactoe ; ; ; games ; 144 ; ; ; positions ; 1083 ; ; ; distinct_games ; 115 ; ; ; distinct_game_fraction ; 0.799 ; ; ; distinct_positions ; 468 ; ; ; distinct_canonical_positions ; 239 ; ; ; known_canonical_positions ; 765 ; ; ; canonical_coverage ; 0.312 ; ; ; decisive_games ; 79 ; ; ; decisive_fraction ; 0.549 ; ;  ; ### Diversity ;  ; ; metric ; value ; threshold ; pass ; ; ; --- ; --- ; --- ; --- ; ; ; canonical_coverage ; 0.312 ; 0.500 ; no ; ; ; decisive_fraction ; 0.549 ; 0.200 ; yes ; ; ; distinct_game_fraction ; 0.799 ; 0.500 ; yes ; ;  ; Diversity pass: no ;  ; - canonical_coverage 0.31 < 0.50 ;  ; ### Outcomes ;  ; ; outcome ; games ; ; ; --- ; --- ; ; ; win O ; 21 ; ; ; win X ; 58 ; ; ; draw ; 65 ; ; ; unfinished ; 0 ; ; ; total ; 144 ; ;  ; ### By pairing ;  ; ; pairing ; wins ; draws ; unfinished ; total ; ; ; --- ; --- ; --- ; --- ; --- ; ; ; depth-2 vs depth-2 ; X=2 ; 14 ; 0 ; 16 ; ; ; depth-2 vs perfect ; X=4 ; 12 ; 0 ; 16 ; ; ; depth-2 vs random ; X=14 ; 2 ; 0 ; 16 ; ; ; perfect vs depth-2 ; X=6 ; 10 ; 0 ; 16 ; ; ; perfect vs perfect ; X=4 ; 12 ; 0 ; 16 ; ; ; perfect vs random ; X=16 ; 0 ; 0 ; 16 ; ; ; random vs depth-2 ; O=6, X=2 ; 8 ; 0 ; 16 ; ; ; random vs perfect ; O=10, X=1 ; 5 ; 0 ; 16 ; ; ; random vs random ; O=5, X=9 ; 2 ; 0 ; 16 ; ;  ; ### By length ;  ; ; length ; games ; ; ; --- ; --- ; ; ; 5 ; 33 ; ; ; 6 ; 10 ; ; ; 7 ; 20 ; ; ; 8 ; 11 ; ; ; 9 ; 70 ; ;  ; ### By ply ;  ; ; ply ; wins ; draws ; unfinished ; total ; ; ; --- ; --- ; --- ; --- ; --- ; ; ; 0 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 1 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 2 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 3 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 4 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 5 ; O=21, X=25 ; 65 ; 0 ; 111 ; ; ; 6 ; O=11, X=25 ; 65 ; 0 ; 101 ; ; ; 7 ; O=11, X=5 ; 65 ; 0 ; 81 ; ; ; 8 ; X=5 ; 65 ; 0 ; 70 ; ;  ; ### By first-action orbit ;  ; ; orbit ; wins ; draws ; unfinished ; total ; ; ; --- ; --- ; --- ; --- ; --- ; ; ; 0 ; O=4, X=12 ; 4 ; 0 ; 20 ; ; ; 1 ; O=6, X=9 ; 6 ; 0 ; 21 ; ; ; 2 ; O=11, X=37 ; 55 ; 0 ; 103 ; ;  ; ## Agreement ;  ; ; metric ; value ; ; ; --- ; --- ; ; ; run_id ; 5eafa65f76637df3 ; ; ; positions ; 1083 ; ; ; agreeing ; 940 ; ; ; rate ; 0.868 ; ;  ; ### By strategy ;  ; ; strategy ; positions ; agreeing ; rate ; ; ; --- ; --- ; --- ; --- ; ; ; depth-2 ; 312 ; 296 ; 0.949 ; ; ; opening ; 72 ; 72 ; 1.000 ; ; ; perfect ; 296 ; 296 ; 1.000 ; ; ; random ; 259 ; 176 ; 0.680 ; ; ; random-opening ; 144 ; 100 ; 0.694 ; ;  ; ### By ply ;  ; ; ply ; positions ; agreeing ; rate ; ; ; --- ; --- ; --- ; --- ; ; ; 0 ; 144 ; 144 ; 1.000 ; ; ; 1 ; 144 ; 93 ; 0.646 ; ; ; 2 ; 144 ; 130 ; 0.903 ; ; ; 3 ; 144 ; 115 ; 0.799 ; ; ; 4 ; 144 ; 125 ; 0.868 ; ; ; 5 ; 111 ; 101 ; 0.910 ; ; ; 6 ; 101 ; 86 ; 0.851 ; ; ; 7 ; 81 ; 76 ; 0.938 ; ; ; 8 ; 70 ; 70 ; 1.000 ; |
| small-2 | generate --config tests/fixtures/generate-small.toml --out target/phase6/determinism/small-2 | run_id=5eafa65f76637df3 cells=36 games=144 positions=1083 out=target/phase6/determinism/small-2 |
| small-2 | annotate --corpus target/phase6/determinism/small-2 | mode=corpus annotated=390 terminal=0 disagreements=0 |
| small-2 | analyze --corpus target/phase6/determinism/small-2 --analyzers summary,agreement | games=144 distinct_games=115 canonical_coverage=0.312 decisive_fraction=0.549 diversity_pass=false |
| small-2 | report --input target/phase6/determinism/small-2 --out target/phase6/determinism/small-2/report.md | # Run 5eafa65f76637df3 ;  ; ; field ; value ; ; ; --- ; --- ; ; ; game ; tictactoe ; ; ; crate_name ; strategy-discovery ; ; ; crate_version ; 0.1.0 ; ; ; seed ; 20260821 ; ; ; games_per_cell ; 4 ; ; ; cells ; 36 ; ; ; games ; 144 ; ; ; positions ; 1083 ; ; ; strategies ; random, depth-2, perfect ; ; ; pairings ; 9 ; ; ; evaluators ; default ; ; ; openings ; none, center ; ; ; random_opening_plies ; 0, 2 ; ;  ; ## Annotation ;  ; ; field ; value ; ; ; --- ; --- ; ; ; mode ; Corpus ; ; ; engine_depth ; 9 ; ; ; evaluator ; default ; ; ; annotated ; 390 ; ; ; terminal ; 0 ; ; ; disagreements ; 0 ; ; ; solver_states ; 5478 ; ;  ; ## Summary ;  ; ; metric ; value ; ; ; --- ; --- ; ; ; run_id ; 5eafa65f76637df3 ; ; ; game ; tictactoe ; ; ; games ; 144 ; ; ; positions ; 1083 ; ; ; distinct_games ; 115 ; ; ; distinct_game_fraction ; 0.799 ; ; ; distinct_positions ; 468 ; ; ; distinct_canonical_positions ; 239 ; ; ; known_canonical_positions ; 765 ; ; ; canonical_coverage ; 0.312 ; ; ; decisive_games ; 79 ; ; ; decisive_fraction ; 0.549 ; ;  ; ### Diversity ;  ; ; metric ; value ; threshold ; pass ; ; ; --- ; --- ; --- ; --- ; ; ; canonical_coverage ; 0.312 ; 0.500 ; no ; ; ; decisive_fraction ; 0.549 ; 0.200 ; yes ; ; ; distinct_game_fraction ; 0.799 ; 0.500 ; yes ; ;  ; Diversity pass: no ;  ; - canonical_coverage 0.31 < 0.50 ;  ; ### Outcomes ;  ; ; outcome ; games ; ; ; --- ; --- ; ; ; win O ; 21 ; ; ; win X ; 58 ; ; ; draw ; 65 ; ; ; unfinished ; 0 ; ; ; total ; 144 ; ;  ; ### By pairing ;  ; ; pairing ; wins ; draws ; unfinished ; total ; ; ; --- ; --- ; --- ; --- ; --- ; ; ; depth-2 vs depth-2 ; X=2 ; 14 ; 0 ; 16 ; ; ; depth-2 vs perfect ; X=4 ; 12 ; 0 ; 16 ; ; ; depth-2 vs random ; X=14 ; 2 ; 0 ; 16 ; ; ; perfect vs depth-2 ; X=6 ; 10 ; 0 ; 16 ; ; ; perfect vs perfect ; X=4 ; 12 ; 0 ; 16 ; ; ; perfect vs random ; X=16 ; 0 ; 0 ; 16 ; ; ; random vs depth-2 ; O=6, X=2 ; 8 ; 0 ; 16 ; ; ; random vs perfect ; O=10, X=1 ; 5 ; 0 ; 16 ; ; ; random vs random ; O=5, X=9 ; 2 ; 0 ; 16 ; ;  ; ### By length ;  ; ; length ; games ; ; ; --- ; --- ; ; ; 5 ; 33 ; ; ; 6 ; 10 ; ; ; 7 ; 20 ; ; ; 8 ; 11 ; ; ; 9 ; 70 ; ;  ; ### By ply ;  ; ; ply ; wins ; draws ; unfinished ; total ; ; ; --- ; --- ; --- ; --- ; --- ; ; ; 0 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 1 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 2 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 3 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 4 ; O=21, X=58 ; 65 ; 0 ; 144 ; ; ; 5 ; O=21, X=25 ; 65 ; 0 ; 111 ; ; ; 6 ; O=11, X=25 ; 65 ; 0 ; 101 ; ; ; 7 ; O=11, X=5 ; 65 ; 0 ; 81 ; ; ; 8 ; X=5 ; 65 ; 0 ; 70 ; ;  ; ### By first-action orbit ;  ; ; orbit ; wins ; draws ; unfinished ; total ; ; ; --- ; --- ; --- ; --- ; --- ; ; ; 0 ; O=4, X=12 ; 4 ; 0 ; 20 ; ; ; 1 ; O=6, X=9 ; 6 ; 0 ; 21 ; ; ; 2 ; O=11, X=37 ; 55 ; 0 ; 103 ; ;  ; ## Agreement ;  ; ; metric ; value ; ; ; --- ; --- ; ; ; run_id ; 5eafa65f76637df3 ; ; ; positions ; 1083 ; ; ; agreeing ; 940 ; ; ; rate ; 0.868 ; ;  ; ### By strategy ;  ; ; strategy ; positions ; agreeing ; rate ; ; ; --- ; --- ; --- ; --- ; ; ; depth-2 ; 312 ; 296 ; 0.949 ; ; ; opening ; 72 ; 72 ; 1.000 ; ; ; perfect ; 296 ; 296 ; 1.000 ; ; ; random ; 259 ; 176 ; 0.680 ; ; ; random-opening ; 144 ; 100 ; 0.694 ; ;  ; ### By ply ;  ; ; ply ; positions ; agreeing ; rate ; ; ; --- ; --- ; --- ; --- ; ; ; 0 ; 144 ; 144 ; 1.000 ; ; ; 1 ; 144 ; 93 ; 0.646 ; ; ; 2 ; 144 ; 130 ; 0.903 ; ; ; 3 ; 144 ; 115 ; 0.799 ; ; ; 4 ; 144 ; 125 ; 0.868 ; ; ; 5 ; 111 ; 101 ; 0.910 ; ; ; 6 ; 101 ; 86 ; 0.851 ; ; ; 7 ; 81 ; 76 ; 0.938 ; ; ; 8 ; 70 ; 70 ; 1.000 ; |
| pipe-1 | pipeline --experiment tests/fixtures/experiment-small.toml --out target/phase6/determinism/pipe-1 | run_id=5eafa65f76637df3 cells=36 games=144 positions=1083 out=target/phase6/determinism/pipe-1 ; mode=corpus annotated=390 terminal=0 disagreements=0 ; games=144 distinct_games=115 canonical_coverage=0.312 decisive_fraction=0.549 diversity_pass=true ; report=target/phase6/determinism/pipe-1\report.md bytes=3195 ; pipeline=experiment-small out=target/phase6/determinism/pipe-1 stages=generate,annotate,analyze,report |
| pipe-2 | pipeline --experiment tests/fixtures/experiment-small.toml --out target/phase6/determinism/pipe-2 | run_id=5eafa65f76637df3 cells=36 games=144 positions=1083 out=target/phase6/determinism/pipe-2 ; mode=corpus annotated=390 terminal=0 disagreements=0 ; games=144 distinct_games=115 canonical_coverage=0.312 decisive_fraction=0.549 diversity_pass=true ; report=target/phase6/determinism/pipe-2\report.md bytes=3195 ; pipeline=experiment-small out=target/phase6/determinism/pipe-2 stages=generate,annotate,analyze,report |
| draws | generate --config tests/fixtures/generate-draws.toml --out target/phase6/determinism/draws | run_id=933a94e742497e81 cells=1 games=16 positions=144 out=target/phase6/determinism/draws |
| default | generate --config configs/tictactoe-default.toml --out target/phase6/determinism/default | run_id=74bba44805e514a7 cells=49 games=980 positions=7491 out=target/phase6/determinism/default |

## SHA-256

| run | file | bytes | sha256 |
| --- | --- | --- | --- |
| play-1 | games.jsonl | 1171 | 8cf2cf013d88206e04f582d73ba6f3bf2a5945aeb6421e022a127b35068d732c |
| play-1 | positions.jsonl | 5563 | ef155fce71a44a67db408987f45e01d14d18d5263b7990633bdb486caedb7ddb |
| play-1 | run.json | 1152 | 25492c4be51410f9ed92f862094d7e126ae86e56ad684c02d61518e5dd35974c |
| play-2 | games.jsonl | 1171 | 8cf2cf013d88206e04f582d73ba6f3bf2a5945aeb6421e022a127b35068d732c |
| play-2 | positions.jsonl | 5563 | ef155fce71a44a67db408987f45e01d14d18d5263b7990633bdb486caedb7ddb |
| play-2 | run.json | 1152 | 25492c4be51410f9ed92f862094d7e126ae86e56ad684c02d61518e5dd35974c |
| gen-equiv | games.jsonl | 1171 | 8cf2cf013d88206e04f582d73ba6f3bf2a5945aeb6421e022a127b35068d732c |
| gen-equiv | positions.jsonl | 5563 | ef155fce71a44a67db408987f45e01d14d18d5263b7990633bdb486caedb7ddb |
| gen-equiv | run.json | 1152 | 25492c4be51410f9ed92f862094d7e126ae86e56ad684c02d61518e5dd35974c |
| small-1 | agreement.json | 1555 | 556823fd7c6c69bc0a54eb34a48ad6a1621d3ece9c7c5bc9d229fdce109f5b98 |
| small-1 | analyze.json | 413 | 7927c91abd720812fdad1091b05c584018226fc63438ccb7847cee8ff57d679d |
| small-1 | annotate.json | 232 | 65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e |
| small-1 | annotations.jsonl | 113018 | 9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3 |
| small-1 | games.jsonl | 86479 | d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d |
| small-1 | positions.jsonl | 403892 | cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7 |
| small-1 | report.md | 3227 | 314a13bb33f83b6a82ac9f0b05e89f170c7a2a9388f9f7797254b1efe51ff9b2 |
| small-1 | run.json | 8415 | a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1 |
| small-1 | summary.json | 3788 | 5794b37e9fe1f3ee786092160298a9435092c47dc4dc9a2b233d0c49e93d24d5 |
| small-2 | agreement.json | 1555 | 556823fd7c6c69bc0a54eb34a48ad6a1621d3ece9c7c5bc9d229fdce109f5b98 |
| small-2 | analyze.json | 413 | 7927c91abd720812fdad1091b05c584018226fc63438ccb7847cee8ff57d679d |
| small-2 | annotate.json | 232 | 65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e |
| small-2 | annotations.jsonl | 113018 | 9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3 |
| small-2 | games.jsonl | 86479 | d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d |
| small-2 | positions.jsonl | 403892 | cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7 |
| small-2 | report.md | 3227 | 314a13bb33f83b6a82ac9f0b05e89f170c7a2a9388f9f7797254b1efe51ff9b2 |
| small-2 | run.json | 8415 | a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1 |
| small-2 | summary.json | 3788 | 5794b37e9fe1f3ee786092160298a9435092c47dc4dc9a2b233d0c49e93d24d5 |
| pipe-1 | agreement.json | 1555 | 556823fd7c6c69bc0a54eb34a48ad6a1621d3ece9c7c5bc9d229fdce109f5b98 |
| pipe-1 | analyze.json | 413 | 4bfdc3896e0e7eb6d925eee4e53bb7b35eedeac1b7b0935c2de258e2352f8ec6 |
| pipe-1 | annotate.json | 232 | 65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e |
| pipe-1 | annotations.jsonl | 113018 | 9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3 |
| pipe-1 | games.jsonl | 86479 | d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d |
| pipe-1 | positions.jsonl | 403892 | cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7 |
| pipe-1 | report.md | 3195 | a9fab4f8398098b5b9da05af924633c530f11c4ce2eb7064a75bf6ad806bcfda |
| pipe-1 | run.json | 8415 | a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1 |
| pipe-1 | summary.json | 3748 | 2f3fbdd29bfcd3603def448dc5e0795948c5b3b933a89a1b36488d866a1609ba |
| pipe-2 | agreement.json | 1555 | 556823fd7c6c69bc0a54eb34a48ad6a1621d3ece9c7c5bc9d229fdce109f5b98 |
| pipe-2 | analyze.json | 413 | 4bfdc3896e0e7eb6d925eee4e53bb7b35eedeac1b7b0935c2de258e2352f8ec6 |
| pipe-2 | annotate.json | 232 | 65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e |
| pipe-2 | annotations.jsonl | 113018 | 9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3 |
| pipe-2 | games.jsonl | 86479 | d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d |
| pipe-2 | positions.jsonl | 403892 | cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7 |
| pipe-2 | report.md | 3195 | a9fab4f8398098b5b9da05af924633c530f11c4ce2eb7064a75bf6ad806bcfda |
| pipe-2 | run.json | 8415 | a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1 |
| pipe-2 | summary.json | 3748 | 2f3fbdd29bfcd3603def448dc5e0795948c5b3b933a89a1b36488d866a1609ba |
| play stdout | - | - | 3479c8d0990d039c713fc3148cc24f146228aef4d110b86d5878b8f1f7586068 |
| report stdout | - | - | 3dc32d7e6782b6888b765c87a5a58f6f6266d5b813dead909fdd7eb4bb016ae5 |

The `play stdout` and `report stdout` rows are the `Get-StringSha` hash of run 1's captured stdout lines (UTF-8, `\n`-joined); run 2 produced the identical hash in both cases (see `## Result`).

## Result

- (a) play --out twice, identical: YES
- (b) play --out vs generate on the equivalent sweep, identical: YES
- (c) generate/annotate/analyze/report chain twice, identical: YES
- (d) play stdout twice, identical: YES
- (e) report stdout twice, identical: YES
- (f) pipeline twice, identical: YES

| file | pinned | actual | match |
| --- | --- | --- | --- |
| run.json | a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1 | a5307c243808d6636e29bc7c2fd9c0162dba09332b361a9b003000d904565fc1 | YES |
| games.jsonl | d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d | d0233f416fea06618bff626af77ebb683e50577e885ed3f27d483ec4a4ea7a5d | YES |
| positions.jsonl | cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7 | cb8953df19f2b1e81cdcc04559cfcdf9e66a056713cf6a1b5844df69f74290a7 | YES |
| annotations.jsonl | 9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3 | 9ca6ede1a8110c25e0def029a666fb9d51d6770d3f838419d2f5cdb1faaf18a3 | YES |
| annotate.json | 65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e | 65a73ac8bb36316726339c947b149dfa0aa8b6d317147ff5065c4fc77e861e4e | YES |
| summary.json | 5794b37e9fe1f3ee786092160298a9435092c47dc4dc9a2b233d0c49e93d24d5 | 5794b37e9fe1f3ee786092160298a9435092c47dc4dc9a2b233d0c49e93d24d5 | YES |

all pinned Phase 5 hashes reproduce: YES

all identical: YES
