param(
    [string]$NodeModules = "E:\tactical_game\3dmap\node_modules",
    [string]$BaseUrl = "http://127.0.0.1:8010",
    [string]$CentrifugoUrl = "ws://127.0.0.1:8000/connection/websocket",
    [string]$Channel = "twg-baseline"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $NodeModules)) {
    throw "node_modules not found: $NodeModules"
}

$env:NODE_PATH = $NodeModules
$env:BASE_URL = $BaseUrl
$env:CENTRIFUGO_URL = $CentrifugoUrl
$env:CHANNEL = $Channel

node (Join-Path $PSScriptRoot "capture-websocket.js")
exit $LASTEXITCODE
