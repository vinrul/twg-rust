param(
    [ValidateSet("auto", "podman", "docker")]
    [string]$Engine = "auto"
)

$ErrorActionPreference = "Stop"

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

foreach ($name in @("twg-baseline-centrifugo", "twg-baseline-postgres")) {
    & $engine container exists $name *> $null
    if ($LASTEXITCODE -eq 0) {
        & $engine stop $name | Out-Null
        Write-Output "Stopped $name"
    } else {
        Write-Output "Not found: $name"
    }
}
