param(
    [string]$AssetsRoot = "E:\tactical_game\3dmap\public",
    [string]$OutDir = (Join-Path $PSScriptRoot "..\assets")
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $AssetsRoot)) {
    throw "Assets root not found: $AssetsRoot"
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$files = Get-ChildItem -LiteralPath $AssetsRoot -Recurse -File | ForEach-Object {
    $relative = $_.FullName.Substring($AssetsRoot.Length).TrimStart('\', '/')
    $draco = $false
    $meshopt = $false

    if ($_.Extension -in @(".glb", ".gltf")) {
        $bytes = [System.IO.File]::ReadAllBytes($_.FullName)
        $text = [System.Text.Encoding]::UTF8.GetString($bytes)
        $draco = $text -match "KHR_draco_mesh_compression"
        $meshopt = $text -match "EXT_meshopt_compression"
    }

    [PSCustomObject]@{
        Path = $relative.Replace('\', '/')
        Extension = $_.Extension.ToLowerInvariant()
        Bytes = $_.Length
        KiB = [math]::Round($_.Length / 1024, 1)
        Draco = $draco
        Meshopt = $meshopt
    }
}

$csvPath = Join-Path $OutDir "assets.generated.csv"
$summaryPath = Join-Path $OutDir "assets-summary.generated.md"
$files | Sort-Object Path | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8

$totalFiles = @($files).Count
$totalBytes = ($files | Measure-Object -Property Bytes -Sum).Sum
$gltf = @($files | Where-Object { $_.Extension -in @(".glb", ".gltf") })
$dracoCount = @($gltf | Where-Object { $_.Draco }).Count
$meshoptCount = @($gltf | Where-Object { $_.Meshopt }).Count

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Inventaris Aset 3D (Generated)")
$lines.Add("")
$lines.Add("Sumber: ``E:\tactical_game\3dmap\public``")
$lines.Add("")
$lines.Add("- Total file: **$totalFiles**")
$lines.Add("- Total ukuran: **$([math]::Round($totalBytes / 1MB, 2)) MiB**")
$lines.Add("- File GLTF/GLB: **$($gltf.Count)**")
$lines.Add("- GLTF/GLB dengan Draco: **$dracoCount**")
$lines.Add("- GLTF/GLB dengan meshopt: **$meshoptCount**")
$lines.Add("")
$lines.Add("## Distribusi ekstensi")
$lines.Add("")
$lines.Add("| Ekstensi | Jumlah | Ukuran (MiB) |")
$lines.Add("|---|---:|---:|")
$files | Group-Object Extension | Sort-Object Count -Descending | ForEach-Object {
    $sum = ($_.Group | Measure-Object -Property Bytes -Sum).Sum
    $ext = if ($_.Name) { $_.Name } else { "(tanpa ekstensi)" }
    $lines.Add("| $ext | $($_.Count) | $([math]::Round($sum / 1MB, 2)) |")
}
$lines.Add("")
$lines.Add("## 20 aset terbesar")
$lines.Add("")
$lines.Add("| Path | MiB | Draco | Meshopt |")
$lines.Add("|---|---:|---|---|")
$files | Sort-Object Bytes -Descending | Select-Object -First 20 | ForEach-Object {
    $lines.Add("| ``$($_.Path)`` | $([math]::Round($_.Bytes / 1MB, 2)) | $($_.Draco) | $($_.Meshopt) |")
}
$lines.Add("")
$lines | Set-Content -LiteralPath $summaryPath -Encoding UTF8

Write-Output "Inventoried $totalFiles assets"
Write-Output $csvPath
Write-Output $summaryPath
