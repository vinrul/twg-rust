param(
    [string]$BaseUrl = "http://127.0.0.1:8010",
    [int]$Samples = 30,
    [string]$OutFile = (Join-Path $PSScriptRoot "..\golden\server-latency.generated.md")
)

$ErrorActionPreference = "Stop"

$endpoints = @(
    @{ Name = "healthz"; Path = "/healthz" },
    @{ Name = "readyz"; Path = "/readyz" },
    @{ Name = "public-operasis"; Path = "/public/list/operasis" },
    @{ Name = "public-markers"; Path = "/public/markers/list/44444444-4444-4444-8444-444444444441" }
)

$results = New-Object System.Collections.Generic.List[object]

foreach ($endpoint in $endpoints) {
    $times = New-Object System.Collections.Generic.List[double]
    $url = $BaseUrl.TrimEnd('/') + $endpoint.Path
    for ($i = 0; $i -lt $Samples; $i++) {
        $value = & curl.exe -sS -o NUL -w "%{time_total}" $url
        if ($LASTEXITCODE -ne 0) {
            throw "Request failed: $url"
        }
        $times.Add([double]::Parse($value, [Globalization.CultureInfo]::InvariantCulture))
    }
    $sorted = @($times | Sort-Object)
    $avg = ($times | Measure-Object -Average).Average * 1000
    $min = $sorted[0] * 1000
    $max = $sorted[$sorted.Count - 1] * 1000
    $p50 = $sorted[[math]::Floor($sorted.Count * 0.50)] * 1000
    $p95 = $sorted[[math]::Min($sorted.Count - 1, [math]::Floor($sorted.Count * 0.95))] * 1000
    $results.Add([PSCustomObject]@{
        Endpoint = $endpoint.Name
        Path = $endpoint.Path
        Samples = $Samples
        AvgMs = [math]::Round($avg, 2)
        MinMs = [math]::Round($min, 2)
        P50Ms = [math]::Round($p50, 2)
        P95Ms = [math]::Round($p95, 2)
        MaxMs = [math]::Round($max, 2)
    })
}

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Baseline Latency Server Lokal")
$lines.Add("")
$lines.Add("Diukur pada $(Get-Date -Format o) terhadap $BaseUrl.")
$lines.Add("Bukan benchmark produksi; loopback, satu proses, fixture lokal.")
$lines.Add("")
$lines.Add("| Endpoint | Samples | Avg ms | Min ms | P50 ms | P95 ms | Max ms |")
$lines.Add("|---|---:|---:|---:|---:|---:|---:|")
foreach ($row in $results) {
    $lines.Add("| $($row.Endpoint) | $($row.Samples) | $($row.AvgMs) | $($row.MinMs) | $($row.P50Ms) | $($row.P95Ms) | $($row.MaxMs) |")
}
$lines.Add("")
$lines | Set-Content -LiteralPath $OutFile -Encoding UTF8

$results | Format-Table -AutoSize
Write-Output $OutFile
