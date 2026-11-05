param(
    [string]$ServerSource = "E:\tactical_game\server",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

$runtimeDir = Join-Path $PSScriptRoot "..\.runtime"
$binary = Join-Path $runtimeDir "twg-baseline-server.exe"
$configSource = Join-Path $PSScriptRoot "..\fixtures\app.env.baseline"
$configTarget = Join-Path $runtimeDir "app.env"

New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $runtimeDir "uploads") | Out-Null

if (-not $SkipBuild) {
    Write-Output "Building baseline server without modifying source..."
    & go -C $ServerSource build -o $binary .
    if ($LASTEXITCODE -ne 0) {
        throw "Go build failed."
    }
}

if (-not (Test-Path -LiteralPath $binary)) {
    throw "Binary not found: $binary"
}

Copy-Item -LiteralPath $configSource -Destination $configTarget -Force

Write-Output "Starting baseline server at http://127.0.0.1:8010"
Write-Output "Using fixture database from baseline/fixtures/app.env.baseline"
Push-Location $runtimeDir
try {
    & $binary
} finally {
    Pop-Location
}
