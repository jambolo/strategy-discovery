## Header

| field | value |
| --- | --- |
| date | 2026-08-24 |
| git_commit | 0cfb937df177b04de678c1bc38f07415c95a65a3 |
| rustc | rustc 1.94.1 (e408947bf 2026-03-25) |
| os | Windows 11 Pro 10.0.26200 |
| cpu | AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs) |
| ram | 31 GiB |
| build_profile | debug |
| command | pwsh -NoProfile -ExecutionPolicy Bypass -File target/m2/determinism.ps1 (script reproduced verbatim in § Commands; binary target/debug/strategy-discovery.exe built by `cargo build`; hashes are SHA-256 via Get-FileHash -Algorithm SHA256) |

## Scope

This evidence claims two things about the full-scale `discover` command against `configs/tictactoe-discover.toml`, run on the debug build:

- Repeat-run byte identity: run-2 vs run-1, the same config document, produce all 16 output files byte-identical, and their stdout is byte-equal modulo replacing each run's own out-dir echo with the literal `OUT`.
- Serial-vs-parallel identity under the config_hash-normalized contract: run-serial uses a serial copy of the config that legitimately differs as a document (its `serial` fields differ in value and always serialize), so its `config_hash` legitimately differs from run-1's. Under that contract, 14 of the 16 files are byte-identical outright, and the two files that carry `config_hash` (`discover.json` and `archive/entries.jsonl`) are byte-equal after replacing each run's own config_hash value with the placeholder `CONFIG_HASH`. stdout is byte-equal modulo the out-dir echo, and this comparison includes the `run_id=` and `evaluation=` lines.

`[mine] engine = "cart"` is the deterministic engine on this path.

## Commands

```powershell
param(
    [Parameter(Mandatory = $true)]
    [string]$Stage
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $Root
$Exe = Join-Path $Root 'target\debug\strategy-discovery.exe'
$Det = 'target/m2/det'
$OK = 'identical' + ': YES'

function Invoke-Discover([string]$Config, [string]$OutDir, [string]$StdoutFile) {
    $out = & $Exe discover --config $Config --out $OutDir
    if ($LASTEXITCODE -ne 0) { throw "exit code $LASTEXITCODE for discover --config $Config --out $OutDir" }
    ($out -join "`n") | Set-Content -NoNewline -Encoding utf8 (Join-Path $Root $StdoutFile)
}

if ($Stage -eq 'run1') {
    if (Test-Path $Det) { Remove-Item -Recurse -Force $Det }
    New-Item -ItemType Directory -Force $Det | Out-Null

    $raw = Get-Content -Raw -Path 'configs/tictactoe-discover.toml'
    $raw = $raw.Replace('[generate.sweep]', "[generate]`nserial = true`n`n[generate.sweep]")
    $raw = $raw.Replace('[discover]', "[discover]`nserial = true")
    Set-Content -Path "$Det/discover-serial.toml" -NoNewline -Encoding utf8 -Value $raw

    Invoke-Discover 'configs/tictactoe-discover.toml' "$Det/run-1" "$Det/run-1.stdout.txt"
}
elseif ($Stage -eq 'run2') {
    Invoke-Discover 'configs/tictactoe-discover.toml' "$Det/run-2" "$Det/run-2.stdout.txt"
}
elseif ($Stage -eq 'serial') {
    Invoke-Discover "$Det/discover-serial.toml" "$Det/run-serial" "$Det/run-serial.stdout.txt"
}
elseif ($Stage -eq 'verify') {
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
    $hs = Get-HashMap "$Det/run-serial"

    if ($h1.Keys.Count -ne 16) { throw "MISMATCH: run-1 has $($h1.Keys.Count) files, expected 16" }
    if ($h2.Keys.Count -ne 16) { throw "MISMATCH: run-2 has $($h2.Keys.Count) files, expected 16" }
    if ($hs.Keys.Count -ne 16) { throw "MISMATCH: run-serial has $($hs.Keys.Count) files, expected 16" }

    $keys1 = $h1.Keys | Sort-Object
    $keys2 = $h2.Keys | Sort-Object
    $keysS = $hs.Keys | Sort-Object
    if (($keys1 -join ',') -ne ($keys2 -join ',')) { throw "MISMATCH: run-1 vs run-2 relative-path sets differ" }
    if (($keys1 -join ',') -ne ($keysS -join ',')) { throw "MISMATCH: run-1 vs run-serial relative-path sets differ" }

    $tableLines = @('| file | run-1 | run-2 | run-serial |', '| --- | --- | --- | --- |')
    foreach ($k in $keys1) {
        $tableLines += "| $k | $($h1[$k]) | $($h2[$k]) | $($hs[$k]) |"
    }
    ($tableLines -join "`n") | Set-Content -NoNewline -Encoding utf8 (Join-Path $Root "$Det/runs-table.md")

    foreach ($k in $keys1) {
        if ($h1[$k] -ne $h2[$k]) { throw "MISMATCH: run-2 vs run-1 differs on $k" }
    }

    $diff = @()
    foreach ($k in $keys1) {
        if ($h1[$k] -ne $hs[$k]) { $diff += $k }
    }
    $diffSorted = ($diff | Sort-Object) -join ','
    $expected = ('archive/entries.jsonl', 'discover.json' | Sort-Object) -join ','
    if ($diffSorted -ne $expected) { throw "MISMATCH: run-serial vs run-1 differing set is [$diffSorted], expected [$expected]" }

    $raw1Discover = Get-Content -Raw -Path (Join-Path $Root "$Det/run-1/discover.json")
    $rawSDiscover = Get-Content -Raw -Path (Join-Path $Root "$Det/run-serial/discover.json")
    $m1 = [regex]::Match($raw1Discover, '"config_hash": "([0-9a-f]{16})"')
    $mS = [regex]::Match($rawSDiscover, '"config_hash": "([0-9a-f]{16})"')
    if (-not $m1.Success) { throw "MISMATCH: config_hash not found in run-1/discover.json" }
    if (-not $mS.Success) { throw "MISMATCH: config_hash not found in run-serial/discover.json" }
    $parallelHash = $m1.Groups[1].Value
    $serialHash = $mS.Groups[1].Value
    ("parallel=$parallelHash", "serial=$serialHash") -join "`n" | Set-Content -NoNewline -Encoding utf8 (Join-Path $Root "$Det/config-hash.txt")

    $raw1Entries = Get-Content -Raw -Path (Join-Path $Root "$Det/run-1/archive/entries.jsonl")
    $rawSEntries = Get-Content -Raw -Path (Join-Path $Root "$Det/run-serial/archive/entries.jsonl")

    $c1Discover = [regex]::Matches($raw1Discover, [regex]::Escape($parallelHash)).Count
    $c1Entries = [regex]::Matches($raw1Entries, [regex]::Escape($parallelHash)).Count
    $cSDiscover = [regex]::Matches($rawSDiscover, [regex]::Escape($serialHash)).Count
    $cSEntries = [regex]::Matches($rawSEntries, [regex]::Escape($serialHash)).Count
    if ($c1Discover -ne 1) { throw "MISMATCH: parallel config_hash appears $c1Discover times in run-1/discover.json, expected 1" }
    if ($c1Entries -ne 4) { throw "MISMATCH: parallel config_hash appears $c1Entries times in run-1/archive/entries.jsonl, expected 4" }
    if ($cSDiscover -ne 1) { throw "MISMATCH: serial config_hash appears $cSDiscover times in run-serial/discover.json, expected 1" }
    if ($cSEntries -ne 4) { throw "MISMATCH: serial config_hash appears $cSEntries times in run-serial/archive/entries.jsonl, expected 4" }

    $norm1Discover = $raw1Discover.Replace($parallelHash, 'CONFIG_HASH')
    $normSDiscover = $rawSDiscover.Replace($serialHash, 'CONFIG_HASH')
    if ($norm1Discover -cne $normSDiscover) { throw "MISMATCH: discover.json not identical after CONFIG_HASH normalization" }

    $norm1Entries = $raw1Entries.Replace($parallelHash, 'CONFIG_HASH')
    $normSEntries = $rawSEntries.Replace($serialHash, 'CONFIG_HASH')
    if ($norm1Entries -cne $normSEntries) { throw "MISMATCH: archive/entries.jsonl not identical after CONFIG_HASH normalization" }

    $out1 = Get-Content -Raw -Path (Join-Path $Root "$Det/run-1.stdout.txt")
    $out2 = Get-Content -Raw -Path (Join-Path $Root "$Det/run-2.stdout.txt")
    $outS = Get-Content -Raw -Path (Join-Path $Root "$Det/run-serial.stdout.txt")

    $dir1 = "$Det/run-1"
    $dir2 = "$Det/run-2"
    $dirS = "$Det/run-serial"

    $norm1Out = $out1.Replace($dir1, 'OUT')
    $norm2Out = $out2.Replace($dir2, 'OUT')
    $normSOut = $outS.Replace($dirS, 'OUT')

    if ($norm1Out -cne $norm2Out) { throw "MISMATCH: run-2 stdout differs from run-1 stdout modulo out=" }
    if ($norm1Out -cne $normSOut) { throw "MISMATCH: run-serial stdout differs from run-1 stdout modulo out=" }

    $verdictLines = @(
        '- run-2 vs run-1 files (16/16 byte-identical), ' + $OK,
        '- run-2 vs run-1 stdout modulo out=, ' + $OK,
        '- run-serial vs run-1 files (14/16 byte-identical outright; discover.json and archive/entries.jsonl byte-identical after config_hash normalization), ' + $OK,
        '- run-serial vs run-1 stdout modulo out= (run_id and evaluation included), ' + $OK
    )
    ($verdictLines -join "`n") | Set-Content -NoNewline -Encoding utf8 (Join-Path $Root "$Det/verdicts.md")
    $verdictLines | ForEach-Object { Write-Host $_ }
}
else {
    throw "unknown stage: $Stage"
}
```

## Runs

| file | run-1 | run-2 | run-serial |
| --- | --- | --- | --- |
| agreement.json | 90a603c4703df0fb40fd052e6defaf42920b54d33366e2afbc8a332b718cbe98 | 90a603c4703df0fb40fd052e6defaf42920b54d33366e2afbc8a332b718cbe98 | 90a603c4703df0fb40fd052e6defaf42920b54d33366e2afbc8a332b718cbe98 |
| analyze.json | 2527b623c6aa559aef583ee72cbad4b57c53fac0412e2d2992effce3b924e731 | 2527b623c6aa559aef583ee72cbad4b57c53fac0412e2d2992effce3b924e731 | 2527b623c6aa559aef583ee72cbad4b57c53fac0412e2d2992effce3b924e731 |
| annotate.json | 29a8e93735e9f02651b654123de367ce57a8976f6e025f9a94a06b1a50d1bd36 | 29a8e93735e9f02651b654123de367ce57a8976f6e025f9a94a06b1a50d1bd36 | 29a8e93735e9f02651b654123de367ce57a8976f6e025f9a94a06b1a50d1bd36 |
| annotations.jsonl | 4b348498499568f648d07cc069c85af9ca9dae0d05a5b6a5e33a1544c1f8a36c | 4b348498499568f648d07cc069c85af9ca9dae0d05a5b6a5e33a1544c1f8a36c | 4b348498499568f648d07cc069c85af9ca9dae0d05a5b6a5e33a1544c1f8a36c |
| archive/archive.json | fd58a8375bab0a1ce1b0fda3a3dc56cfaa5f06d3c871c3e81e80df0a1624718b | fd58a8375bab0a1ce1b0fda3a3dc56cfaa5f06d3c871c3e81e80df0a1624718b | fd58a8375bab0a1ce1b0fda3a3dc56cfaa5f06d3c871c3e81e80df0a1624718b |
| archive/entries.jsonl | 6cd286fa7bb5dc37010c5039d3c4dfbbb873dfac781d5c82bd3b1db665261b8e | 6cd286fa7bb5dc37010c5039d3c4dfbbb873dfac781d5c82bd3b1db665261b8e | cb24159a7a990fca5e054617815c08424da187d588e4db44b4987b9b7f835baf |
| dataset.json | ae9f3977f6f40a5364d331927036ee6e60e38644f8416d8cc29f2b02a04089ed | ae9f3977f6f40a5364d331927036ee6e60e38644f8416d8cc29f2b02a04089ed | ae9f3977f6f40a5364d331927036ee6e60e38644f8416d8cc29f2b02a04089ed |
| dataset.jsonl | 1e3b702965cb918eafe272b891889209b3625328aea899f8d4fbff0e311bc92e | 1e3b702965cb918eafe272b891889209b3625328aea899f8d4fbff0e311bc92e | 1e3b702965cb918eafe272b891889209b3625328aea899f8d4fbff0e311bc92e |
| discover.json | bc5fb5f6e22fe4b5d3195a2e9c7478f68bcb257dfcca2443c61448fa9f6fa6a6 | bc5fb5f6e22fe4b5d3195a2e9c7478f68bcb257dfcca2443c61448fa9f6fa6a6 | dd20992f177227c4f80e081564cd0ea0911cedb1e86988fd8d25db35bcf55ba0 |
| evaluation.json | 2627ba05bac3a05fac1bdd8158a0e78a3f0ce21ef92322796f8d3856429fd0a7 | 2627ba05bac3a05fac1bdd8158a0e78a3f0ce21ef92322796f8d3856429fd0a7 | 2627ba05bac3a05fac1bdd8158a0e78a3f0ce21ef92322796f8d3856429fd0a7 |
| games.jsonl | 40cb19a4952a5ad1fda8e1f4be1879b153e5c26b376d97a66d19fc92b7bf475f | 40cb19a4952a5ad1fda8e1f4be1879b153e5c26b376d97a66d19fc92b7bf475f | 40cb19a4952a5ad1fda8e1f4be1879b153e5c26b376d97a66d19fc92b7bf475f |
| heuristics.json | 3a2d0c00a44b3653b46f8f3f1e93500eadf4ab7ef0c4d3838c5794800c6dcd66 | 3a2d0c00a44b3653b46f8f3f1e93500eadf4ab7ef0c4d3838c5794800c6dcd66 | 3a2d0c00a44b3653b46f8f3f1e93500eadf4ab7ef0c4d3838c5794800c6dcd66 |
| positions.jsonl | cc7ea2e590b06ea67d5bb26bae472419ac6e5b664733fff3987a65980c77edba | cc7ea2e590b06ea67d5bb26bae472419ac6e5b664733fff3987a65980c77edba | cc7ea2e590b06ea67d5bb26bae472419ac6e5b664733fff3987a65980c77edba |
| report.md | 33a472a251b8ee636c601be50502cbac38a0fba0a901e299dcd78562186b2a3b | 33a472a251b8ee636c601be50502cbac38a0fba0a901e299dcd78562186b2a3b | 33a472a251b8ee636c601be50502cbac38a0fba0a901e299dcd78562186b2a3b |
| run.json | d0108d9b64fcf08524200a850d6507cc15487bf4fcdf8741dc229200db7f007b | d0108d9b64fcf08524200a850d6507cc15487bf4fcdf8741dc229200db7f007b | d0108d9b64fcf08524200a850d6507cc15487bf4fcdf8741dc229200db7f007b |
| summary.json | a8b7c47b8fc8201f38c2154bd3ed296c0348a7152cd08c65f76473d76d5f22e2 | a8b7c47b8fc8201f38c2154bd3ed296c0348a7152cd08c65f76473d76d5f22e2 | a8b7c47b8fc8201f38c2154bd3ed296c0348a7152cd08c65f76473d76d5f22e2 |

- run-2 vs run-1 files (16/16 byte-identical), identical: YES
- run-2 vs run-1 stdout modulo out=, identical: YES
- run-serial vs run-1 files (14/16 byte-identical outright; discover.json and archive/entries.jsonl byte-identical after config_hash normalization), identical: YES
- run-serial vs run-1 stdout modulo out= (run_id and evaluation included), identical: YES

### config_hash

- parallel config_hash: cbcaffd750bb707b
- serial config_hash: c3c8b37144634b33
- sole differing field: config_hash (once in discover.json, four times in archive/entries.jsonl)
- reason: the serial copy is a different config document — its serial fields differ in value and always serialize (#[serde(default)] affects deserialization only), so its config_hash legitimately differs; config_hash hashes only the loaded config document and is frozen by the project constraints.
