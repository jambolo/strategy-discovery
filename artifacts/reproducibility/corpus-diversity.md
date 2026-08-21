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
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/gate-b/diversity.ps1 (script reproduced in § Commands; binary target/release/strategy-discovery.exe built by `cargo build --release`) |

## Configs

- default: `configs/tictactoe-default.toml` (`games_per_cell = 20` → 49 ordered roster pairings × 20 = 980 games) with `seed` replaced by 1, 2 and 3, written by the script to `target/gate-b/diversity/tictactoe-seed{1,2,3}.toml`:

```toml
schema_version = 1
game = "tictactoe"
seed = <1 | 2 | 3>
games_per_cell = 20
```

- negative control: `tests/fixtures/generate-draws.toml` — one strategy `perfect-engine` (minimax depth 9, `tie_break = "engine"`), self-paired, 16 games: every game is the same perfect-play draw (the plan's "corpus of identical perfect-play draws is a failure" case)
- thresholds: `DiversityThresholds::default()` = `min_canonical_coverage = 0.5`, `min_decisive_fraction = 0.2`, `min_distinct_game_fraction = 0.5`; `known_canonical_positions = 765`; `analyze --strict` exits 2 on failure

## Commands

```powershell
# Gate B diversity: default config at seeds 1-3 against the default thresholds, plus the draws negative control (release CLI).
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Exe = Join-Path $Root 'target\release\strategy-discovery.exe'
$Work = Join-Path $Root 'target\gate-b\diversity'
if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Force $Work | Out-Null
Write-Host "root=$Root"
Write-Host "exe=$Exe"

function Invoke-Cli([string[]]$CliArgs) {
    $out = & $Exe @CliArgs
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for: $($CliArgs -join ' ')" }
    Write-Host ('> strategy-discovery {0}' -f ($CliArgs -join ' '))
    Write-Host ('  {0}' -f ($out -join ' | '))
}
function Write-Row([string]$Label, [string]$Dir, [string]$StrictExit) {
    $s = Get-Content -Raw (Join-Path $Dir 'summary.json') | ConvertFrom-Json
    $inv = [cultureinfo]::InvariantCulture
    Write-Host ('ROW {0} run_id={1} games={2} distinct_games={3} distinct_game_fraction={4} distinct_canonical_positions={5} known_canonical_positions={6} canonical_coverage={7} decisive_games={8} decisive_fraction={9} diversity_pass={10} strict_exit={11} failures=[{12}]' -f `
            $Label, $s.run_id, $s.games, $s.distinct_games, $s.distinct_game_fraction.ToString('F4', $inv), $s.distinct_canonical_positions, $s.known_canonical_positions, `
            $s.canonical_coverage.ToString('F4', $inv), $s.decisive_games, $s.decisive_fraction.ToString('F4', $inv), $s.diversity_pass.ToString().ToLower(), $StrictExit, ($s.diversity_failures -join '; '))
    Write-Host ('  thresholds: min_canonical_coverage={0} min_decisive_fraction={1} min_distinct_game_fraction={2}' -f $s.thresholds.min_canonical_coverage, $s.thresholds.min_decisive_fraction, $s.thresholds.min_distinct_game_fraction)
}

foreach ($seed in 1, 2, 3) {
    $cfg = Join-Path $Work "tictactoe-seed$seed.toml"
    Set-Content -Path $cfg -Encoding ascii -Value @('schema_version = 1', 'game = "tictactoe"', "seed = $seed", 'games_per_cell = 20')
    $dir = Join-Path $Work "seed-$seed"
    Invoke-Cli @('generate', '--config', $cfg, '--out', $dir)
    & $Exe analyze --corpus $dir --strict
    $exit = $LASTEXITCODE
    Write-Host "  analyze --strict exit=$exit"
    Write-Row "seed-$seed" $dir $exit
}

$dir = Join-Path $Work 'draws'
Invoke-Cli @('generate', '--config', (Join-Path $Root 'tests\fixtures\generate-draws.toml'), '--out', $dir)
& $Exe analyze --corpus $dir --strict
$exit = $LASTEXITCODE
Write-Host "  analyze --strict exit=$exit"
Write-Row 'draws' $dir $exit
```

## Result

| config | seed | run_id | games | distinct_games | distinct_game_fraction | distinct_canonical_positions | canonical_coverage | decisive_games | decisive_fraction | diversity_pass | strict_exit | diversity_failures | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| default | 1 | a6307a3549cd8da3 | 980 | 579 | 0.5908 | 460 | 0.6013 | 512 | 0.5224 | true | 0 | - | PASS |
| default | 2 | dfb3fa21f15dc3a0 | 980 | 576 | 0.5878 | 468 | 0.6118 | 510 | 0.5204 | true | 0 | - | PASS |
| default | 3 | 4f30afd1b5711779 | 980 | 613 | 0.6255 | 464 | 0.6065 | 501 | 0.5112 | true | 0 | - | PASS |
| generate-draws.toml | 20260821 | 933a94e742497e81 | 16 | 1 | 0.0625 | 10 | 0.0131 | 0 | 0.0000 | false | 2 | canonical_coverage 0.01 < 0.50; decisive_fraction 0.00 < 0.20; distinct_game_fraction 0.06 < 0.50 | FAIL (expected) |

- default config at seeds 1-3 all pass 0.5 / 0.2 / 0.5: YES
- draws negative control fails (diversity_pass false, --strict exit 2): YES
