#Requires -Version 7
<#
.SYNOPSIS
    Measures clean release build time and dependency count for the record-format-spike crate,
    with and without the `polars` feature.
#>
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path,
    [string]$Out = (Join-Path $RepoRoot "artifacts/benchmarks/record-format-compile.json")
)

$ErrorActionPreference = "Stop"

$SpikeManifest = Join-Path $PSScriptRoot "Cargo.toml"

function Measure-Variant {
    param(
        [string]$Name,
        [string[]]$ExtraArgs
    )

    & cargo clean --manifest-path $SpikeManifest
    if ($LASTEXITCODE -ne 0) {
        throw "cargo clean failed for variant '$Name'"
    }

    $buildArgs = @("build", "--release", "--manifest-path", $SpikeManifest) + $ExtraArgs
    $elapsed = Measure-Command {
        & cargo @buildArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed for variant '$Name'"
        }
    }
    $buildSeconds = [Math]::Round($elapsed.TotalSeconds, 1)

    $treeArgs = @("tree", "--manifest-path", $SpikeManifest, "--edges", "normal") + $ExtraArgs
    $treeOutput = & cargo @treeArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo tree failed for variant '$Name'"
    }
    $dependencyCount = ($treeOutput | Measure-Object -Line).Lines - 1

    [PSCustomObject]@{
        build_seconds     = $buildSeconds
        dependency_count  = $dependencyCount
        command           = "cargo build --release --manifest-path $SpikeManifest $($ExtraArgs -join ' ')".Trim()
    }
}

$withPolars = Measure-Variant -Name "with_polars" -ExtraArgs @()
$withoutPolars = Measure-Variant -Name "without_polars" -ExtraArgs @("--no-default-features")

$cargoVersion = (& cargo --version).Trim()

$result = [PSCustomObject]@{
    with_polars    = $withPolars
    without_polars = $withoutPolars
    cargo_version  = $cargoVersion
}

New-Item -ItemType Directory -Force -Path (Split-Path $Out) | Out-Null
$result | ConvertTo-Json -Depth 5 | Set-Content -Path $Out -Encoding utf8

# Leave the spike binary built (default features) for subsequent runs.
& cargo build --release --manifest-path $SpikeManifest
if ($LASTEXITCODE -ne 0) {
    throw "final default-feature release build failed"
}

Write-Host "Wrote $Out"
