param(
    [string]$GoldenDir = (Join-Path $PSScriptRoot "..\golden")
)

$ErrorActionPreference = "Stop"

$metaPath = Join-Path $GoldenDir "capture-meta.json"
if (-not (Test-Path -LiteralPath $metaPath)) {
    throw "capture-meta.json not found. Run capture-golden.ps1 first."
}

$meta = Get-Content -LiteralPath $metaPath -Raw | ConvertFrom-Json
$results = $meta.results
$failures = New-Object System.Collections.Generic.List[string]

foreach ($item in $results) {
    if ([int]$item.Status -ge 500) {
        $failures.Add("5xx: $($item.Name) -> $($item.Status)")
    }
}

function Expect-Status {
    param([string]$Pattern, [int]$Status)
    $matches = @($results | Where-Object { $_.Name -like $Pattern })
    if ($matches.Count -eq 0) {
        $failures.Add("Missing capture: $Pattern")
        return
    }
    foreach ($match in $matches) {
        if ([int]$match.Status -ne $Status) {
            $failures.Add("Expected $Status, got $($match.Status): $($match.Name)")
        }
    }
}

foreach ($name in @(
    "auth-login-admin",
    "auth-login-admin_monitor",
    "auth-login-admin_satker",
    "auth-login-user_satker",
    "auth-login-user"
)) {
    Expect-Status -Pattern $name -Status 201
}

foreach ($role in @("admin", "admin_monitor", "admin_satker", "user_satker", "user")) {
    Expect-Status -Pattern "auth-refresh-$role" -Status 200
    Expect-Status -Pattern "auth-identify-$role" -Status 200
}

Expect-Status -Pattern "public_healthz" -Status 200
Expect-Status -Pattern "public_readyz" -Status 200
Expect-Status -Pattern "public_public_users_online" -Status 200

Expect-Status -Pattern "admin-admin-_admin_*" -Status 200
Expect-Status -Pattern "admin-admin_monitor-_admin_*" -Status 200
Expect-Status -Pattern "admin-admin_satker-_admin_*" -Status 200
Expect-Status -Pattern "admin-user_satker-_admin_*" -Status 403
Expect-Status -Pattern "admin-user-_admin_*" -Status 403

Expect-Status -Pattern "private-user-_private_satkers" -Status 403
Expect-Status -Pattern "private-user-_private_operasis_33333333_3333_4333_8333_333333333331" -Status 403
Expect-Status -Pattern "private-user_satker-_private_satkers" -Status 200
Expect-Status -Pattern "private-admin-_private_satkers" -Status 200

$jsonFiles = Get-ChildItem -LiteralPath (Join-Path $GoldenDir "redacted") -Filter "*.json" -File
foreach ($file in $jsonFiles) {
    $content = Get-Content -LiteralPath $file.FullName -Raw
    try {
        $null = $content | ConvertFrom-Json
    } catch {
        $failures.Add("Invalid JSON: $($file.Name)")
    }
    if ($content -match '"access_token"' -and $content -notmatch 'REDACTED') {
        $failures.Add("Unredacted access_token: $($file.Name)")
    }
    if ($content -match '"refresh_token"' -and $content -notmatch 'REDACTED') {
        $failures.Add("Unredacted refresh_token: $($file.Name)")
    }
}

$summaryPath = Join-Path $GoldenDir "verification.generated.md"
$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("# Golden Verification")
$lines.Add("")
$lines.Add("Total capture: **$($results.Count)**")
$lines.Add("Redacted JSON files: **$($jsonFiles.Count)**")
$lines.Add("Failures: **$($failures.Count)**")
$lines.Add("")
if ($failures.Count -eq 0) {
    $lines.Add("Status: **PASS**")
} else {
    $lines.Add("Status: **FAIL**")
    $lines.Add("")
    foreach ($failure in $failures) {
        $lines.Add("- $failure")
    }
}
$lines | Set-Content -LiteralPath $summaryPath -Encoding UTF8

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { Write-Error $_ }
    exit 1
}

Write-Output "Golden verification PASS ($($results.Count) captures, $($jsonFiles.Count) JSON files)"
Write-Output $summaryPath
