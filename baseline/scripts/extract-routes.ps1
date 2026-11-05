param(
    [string]$RoutesFile = "E:\tactical_game\server\api\routes.go",
    [string]$OutDir = (Join-Path $PSScriptRoot "..\api")
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $RoutesFile)) {
    throw "Routes file not found: $RoutesFile"
}

$prefixes = @{
    "router" = ""
}

$access = @{
    "router" = "public"
    "authGroup" = "public"
    "authProtected" = "bearer"
    "publicGroup" = "public"
    "adminGroup" = "bearer+session+role(admin|admin_satker|admin_monitor)"
    "privateGroup" = "bearer+session"
    "snapshotWriteGroup" = "bearer+session+role(admin|admin_satker)"
}

$rows = New-Object System.Collections.Generic.List[object]
$lineNo = 0

foreach ($line in Get-Content -LiteralPath $RoutesFile) {
    $lineNo++

    if ($line -match '^\s*(\w+)\s*:=\s*router\.Group\("([^"]+)"\)') {
        $prefixes[$matches[1]] = $matches[2]
        continue
    }

    if ($line -match '^\s*(\w+)\s*:=\s*(\w+)\.Use\(') {
        $target = $matches[2]
        if ($prefixes.ContainsKey($target)) {
            $prefixes[$matches[1]] = $prefixes[$target]
        }
        continue
    }

    if ($line -match '^\s*(\w+)\.(GET|POST|PUT|PATCH|DELETE)\("([^"]+)",\s*(.+?)\)\s*$') {
        $group = $matches[1]
        $method = $matches[2]
        $path = $matches[3]
        $handler = $matches[4].Trim()
        $prefix = ""
        if ($prefixes.ContainsKey($group)) {
            $prefix = $prefixes[$group]
        }
        $fullPath = ($prefix + $path) -replace '//+', '/'
        $rowAccess = "unknown"
        if ($access.ContainsKey($group)) {
            $rowAccess = $access[$group]
        }
        $rows.Add([PSCustomObject]@{
            Method = $method
            Path = $fullPath
            Access = $rowAccess
            Handler = $handler
            Source = "server/api/routes.go:$lineNo"
        })
    }
}

$rows.Add([PSCustomObject]@{
    Method = "GET"
    Path = "/uploads/*"
    Access = "public-static"
    Handler = "router.Static"
    Source = "server/api/routes.go:12"
})

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$csvPath = Join-Path $OutDir "routes.generated.csv"
$mdPath = Join-Path $OutDir "routes.generated.md"
$rows | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Inventaris Route Backend (Generated)")
$lines.Add("")
$lines.Add("Dihasilkan dari ``server/api/routes.go``. Jangan edit manual; jalankan ulang ``scripts/extract-routes.ps1``.")
$lines.Add("")
$lines.Add("Total route: **$($rows.Count)**")
$lines.Add("")
$lines.Add("| Method | Path | Access | Handler | Source |")
$lines.Add("|---|---|---|---|---|")
foreach ($row in $rows) {
    $lines.Add("| $($row.Method) | ``$($row.Path)`` | $($row.Access) | ``$($row.Handler)`` | $($row.Source) |")
}
$lines.Add("")
$lines | Set-Content -LiteralPath $mdPath -Encoding UTF8

Write-Output "Generated $($rows.Count) routes"
Write-Output $csvPath
Write-Output $mdPath
