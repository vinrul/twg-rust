param(
    [ValidateSet("auto", "podman", "docker")]
    [string]$Engine = "auto",
    [string]$Image = "docker.io/centrifugo/centrifugo:v6.2.5",
    [string]$Config = (Join-Path $PSScriptRoot "..\fixtures\centrifugo.json")
)

$ErrorActionPreference = "Stop"

$containerName = "twg-baseline-centrifugo"
$port = 8000

function Resolve-ContainerEngine {
    param([string]$Requested)
    if ($Requested -eq "auto") {
        if (Get-Command podman -ErrorAction SilentlyContinue) { return "podman" }
        if (Get-Command docker -ErrorAction SilentlyContinue) { return "docker" }
        throw "Neither Podman nor Docker CLI was found."
    }
    if (-not (Get-Command $Requested -ErrorAction SilentlyContinue)) {
        throw "Container engine not found: $Requested"
    }
    return $Requested
}

$engine = Resolve-ContainerEngine -Requested $Engine
$configPath = Resolve-Path $Config

& $engine info *> $null
if ($LASTEXITCODE -ne 0) {
    throw "$engine daemon/engine is not reachable."
}

& $engine image exists $Image *> $null
if ($LASTEXITCODE -ne 0) {
    throw "Image '$Image' is not available. Pull it first: $engine pull $Image"
}

& $engine rm -f $containerName *> $null

Write-Output "Creating Centrifugo container via $engine..."
& $engine create `
    --name $containerName `
    -p "${port}:8000" `
    $Image centrifugo --config /centrifugo/config.json | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create Centrifugo container."
}

& $engine cp $configPath "${containerName}:/centrifugo/config.json"
if ($LASTEXITCODE -ne 0) {
    throw "Failed to copy Centrifugo config."
}

& $engine start $containerName | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Failed to start Centrifugo container."
}

$ready = $false
for ($i = 0; $i -lt 45; $i++) {
    try {
        $response = Invoke-WebRequest -UseBasicParsing -Uri "http://127.0.0.1:$port/health" -TimeoutSec 2
        if ($response.StatusCode -eq 200) {
            $ready = $true
            break
        }
    } catch {
    }
    Start-Sleep -Seconds 1
}

if (-not $ready) {
    & $engine logs $containerName 2>&1 | Select-Object -Last 80
    throw "Centrifugo did not become ready."
}

Write-Output "Centrifugo ready."
Write-Output "Container: $containerName"
Write-Output "HTTP API: http://127.0.0.1:$port/api"
Write-Output "WebSocket: ws://127.0.0.1:$port/connection/websocket"
Write-Output "API key: baseline-only-not-secret (local test only)"
