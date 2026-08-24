## Header

| field | value |
| --- | --- |
| date | 2026-09-10 |
| git_commit | 173d6da917d34debc6c069e7a947971d61412fa6 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | debug |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/m3/determinism.ps1 (script reproduced verbatim in § Commands; binary target/debug/strategy-discovery.exe built by `cargo build`; hashes are SHA-256 via Get-FileHash -Algorithm SHA256) |

## Scope

This evidence claims one thing about the full-scale induction-enabled `discover`
command against `configs/tictactoe-concepts.toml` (tier-2 withheld, extended
tier-1, two promotion rounds), run on the debug build: repeat-run byte identity.
Run-2 vs run-1, the same config document, produce all 18 output files
byte-identical — `concepts.json` and `vocabulary.json` included — and their stdout
is byte-equal modulo replacing each run's own out-dir echo with the literal `OUT`.
There is no serial variant: the Milestone 3 DoD demands repeat-run identity only,
and `[mine] engine = "cart"` plus the cart-only induction probe are the
deterministic engines on this path.

## Commands

```powershell
# Step 43-determinism: two full-scale debug discover runs, SHA-256 table, verdicts.
# Writes target/m3/det/runs-table.md, target/m3/det/verdicts.md, target/m3/det/header-fields.txt.
# Throws on any mismatch; the two verdict strings are built by concatenation so this
# script never contains them literally.
$ErrorActionPreference = 'Stop'
$Det = 'target/m3/det'
if (Test-Path $Det) { Remove-Item -Recurse -Force $Det }
New-Item -ItemType Directory -Force $Det | Out-Null
$OK = 'identical' + ': YES'

function Invoke-Discover([string]$OutDir, [string]$StdoutFile) {
    $out = & ./target/debug/strategy-discovery.exe discover --config configs/tictactoe-concepts.toml --out $OutDir 2>"$StdoutFile.stderr.txt"
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for discover --out $OutDir" }
    ($out -join "`n") | Set-Content -NoNewline -Encoding utf8 $StdoutFile
}

Invoke-Discover "$Det/run-1" "$Det/run-1.stdout.txt"
Invoke-Discover "$Det/run-2" "$Det/run-2.stdout.txt"

function Get-HashMap([string]$Dir) {
    $map = @{}
    Get-ChildItem -Path $Dir -Recurse -File | ForEach-Object {
        $rel = $_.FullName.Substring((Resolve-Path $Dir).Path.Length + 1).Replace('\', '/')
        $map[$rel] = (Get-FileHash -Algorithm SHA256 $_.FullName).Hash.ToLower()
    }
    return $map
}

$h1 = Get-HashMap "$Det/run-1"
$h2 = Get-HashMap "$Det/run-2"
if ($h1.Keys.Count -ne 18) { throw "MISMATCH: run-1 has $($h1.Keys.Count) files, expected 18" }
if ($h2.Keys.Count -ne 18) { throw "MISMATCH: run-2 has $($h2.Keys.Count) files, expected 18" }
$keys1 = $h1.Keys | Sort-Object
$keys2 = $h2.Keys | Sort-Object
if (($keys1 -join ',') -ne ($keys2 -join ',')) { throw 'MISMATCH: run-1 vs run-2 relative-path sets differ' }

$tableLines = @('| file | run-1 | run-2 |', '| --- | --- | --- |')
foreach ($k in $keys1) { $tableLines += "| $k | $($h1[$k]) | $($h2[$k]) |" }
($tableLines -join "`n") | Set-Content -NoNewline -Encoding utf8 "$Det/runs-table.md"

foreach ($k in $keys1) {
    if ($h1[$k] -ne $h2[$k]) { throw "MISMATCH: run-2 vs run-1 differs on $k" }
}

$out1 = (Get-Content -Raw "$Det/run-1.stdout.txt").Replace("$Det/run-1", 'OUT')
$out2 = (Get-Content -Raw "$Det/run-2.stdout.txt").Replace("$Det/run-2", 'OUT')
if ($out1 -cne $out2) { throw 'MISMATCH: run-2 stdout differs from run-1 stdout modulo out=' }

$verdictLines = @(
    '- run-2 vs run-1 files (18/18 byte-identical), ' + $OK,
    '- run-2 vs run-1 stdout modulo out=, ' + $OK
)
($verdictLines -join "`n") | Set-Content -NoNewline -Encoding utf8 "$Det/verdicts.md"
$verdictLines | ForEach-Object { Write-Host $_ }

$gitCommit = (git rev-parse HEAD).Trim()
$date = Get-Date -Format 'yyyy-MM-dd'
("date=$date", "git_commit=$gitCommit") -join "`n" | Set-Content -NoNewline -Encoding utf8 "$Det/header-fields.txt"
Write-Host 'determinism evidence complete.'
```

## Runs

| file | run-1 | run-2 |
| --- | --- | --- |
| agreement.json | 90a603c4703df0fb40fd052e6defaf42920b54d33366e2afbc8a332b718cbe98 | 90a603c4703df0fb40fd052e6defaf42920b54d33366e2afbc8a332b718cbe98 |
| analyze.json | 55a63df9119aac7e2e5502a32991d052c64c50ff4bc8fca5d2d6f08f0264ca73 | 55a63df9119aac7e2e5502a32991d052c64c50ff4bc8fca5d2d6f08f0264ca73 |
| annotate.json | 29a8e93735e9f02651b654123de367ce57a8976f6e025f9a94a06b1a50d1bd36 | 29a8e93735e9f02651b654123de367ce57a8976f6e025f9a94a06b1a50d1bd36 |
| annotations.jsonl | 4b348498499568f648d07cc069c85af9ca9dae0d05a5b6a5e33a1544c1f8a36c | 4b348498499568f648d07cc069c85af9ca9dae0d05a5b6a5e33a1544c1f8a36c |
| archive/archive.json | cd3957e306706fe44094429392df4ffc2a89b58b65a825464c2b3bd5cbeabbdb | cd3957e306706fe44094429392df4ffc2a89b58b65a825464c2b3bd5cbeabbdb |
| archive/entries.jsonl | f27efba85d9a2679bbc67ff0348bff83ee904cdb84a6dcc55784520a41781947 | f27efba85d9a2679bbc67ff0348bff83ee904cdb84a6dcc55784520a41781947 |
| concepts.json | 541c5ea59ea43739f8fc8da969499c5cc3d8193d3f1d694f0150b6ac52bb71ac | 541c5ea59ea43739f8fc8da969499c5cc3d8193d3f1d694f0150b6ac52bb71ac |
| dataset.json | 40580182e3168ecc7e79af36eb750598b5e534e7ac0527acf37640ad7e7606ef | 40580182e3168ecc7e79af36eb750598b5e534e7ac0527acf37640ad7e7606ef |
| dataset.jsonl | 133c33eabf2defbf9931a6bfb2823caf3195af17af09b423c02dd3a9ecc3abba | 133c33eabf2defbf9931a6bfb2823caf3195af17af09b423c02dd3a9ecc3abba |
| discover.json | 7888de4b46888a5de6a4ef8c9a6fe505cecbda0ce431ab51d94a84ed6ae6b16f | 7888de4b46888a5de6a4ef8c9a6fe505cecbda0ce431ab51d94a84ed6ae6b16f |
| evaluation.json | 0e9c3423b5ef240316f6f0567fbb5a52897c78c46f562bba99e766ec63fdc153 | 0e9c3423b5ef240316f6f0567fbb5a52897c78c46f562bba99e766ec63fdc153 |
| games.jsonl | 40cb19a4952a5ad1fda8e1f4be1879b153e5c26b376d97a66d19fc92b7bf475f | 40cb19a4952a5ad1fda8e1f4be1879b153e5c26b376d97a66d19fc92b7bf475f |
| heuristics.json | c52ba9bf7d150837fdc11a0d383293fea3292f98853aac22abe0db017aad13a6 | c52ba9bf7d150837fdc11a0d383293fea3292f98853aac22abe0db017aad13a6 |
| positions.jsonl | cc7ea2e590b06ea67d5bb26bae472419ac6e5b664733fff3987a65980c77edba | cc7ea2e590b06ea67d5bb26bae472419ac6e5b664733fff3987a65980c77edba |
| report.md | e78d8a1509e25a088608a3cbfa67328d67b095af0ea404e6b7df18684a2b0765 | e78d8a1509e25a088608a3cbfa67328d67b095af0ea404e6b7df18684a2b0765 |
| run.json | d0108d9b64fcf08524200a850d6507cc15487bf4fcdf8741dc229200db7f007b | d0108d9b64fcf08524200a850d6507cc15487bf4fcdf8741dc229200db7f007b |
| summary.json | a8b7c47b8fc8201f38c2154bd3ed296c0348a7152cd08c65f76473d76d5f22e2 | a8b7c47b8fc8201f38c2154bd3ed296c0348a7152cd08c65f76473d76d5f22e2 |
| vocabulary.json | e1fca819b5c7760945346186bbab2556a771aefc3e21e0c84b5b2408905a00fe | e1fca819b5c7760945346186bbab2556a771aefc3e21e0c84b5b2408905a00fe |

- run-2 vs run-1 files (18/18 byte-identical), identical: YES
- run-2 vs run-1 stdout modulo out=, identical: YES
