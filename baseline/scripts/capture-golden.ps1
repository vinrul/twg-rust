param(
    [string]$BaseUrl = "http://127.0.0.1:8010",
    [string]$OutDir = (Join-Path $PSScriptRoot "..\golden"),
    [string]$Password = "Baseline#2026",
    [switch]$AllowRemote
)

$ErrorActionPreference = "Stop"

$uri = [Uri]$BaseUrl
if (-not $AllowRemote -and $uri.Host -notin @("127.0.0.1", "localhost", "::1")) {
    throw "Refusing to capture from non-local host '$($uri.Host)'. Use -AllowRemote only for an isolated test instance."
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$rawDir = Join-Path $OutDir "raw"
$redactedDir = Join-Path $OutDir "redacted"
New-Item -ItemType Directory -Force -Path $rawDir | Out-Null
New-Item -ItemType Directory -Force -Path $redactedDir | Out-Null

$script:results = New-Object System.Collections.Generic.List[object]

function Write-Utf8NoBom {
    param([string]$Path, [string]$Content)
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Content, $encoding)
}

function Invoke-Capture {
    param(
        [string]$Name,
        [string]$Method,
        [string]$Path,
        [hashtable]$Headers = @{},
        [object]$Body = $null
    )

    $url = $BaseUrl.TrimEnd('/') + $Path
    $params = @{
        Uri = $url
        Method = $Method
        Headers = $Headers
        UseBasicParsing = $true
        TimeoutSec = 60
    }
    if ($null -ne $Body) {
        $params["ContentType"] = "application/json"
        $params["Body"] = ($Body | ConvertTo-Json -Depth 20 -Compress)
    }

    $status = 0
    $content = ""
    try {
        $response = Invoke-WebRequest @params
        $status = [int]$response.StatusCode
        $content = $response.Content
    } catch {
        if ($_.Exception.Response) {
            $status = [int]$_.Exception.Response.StatusCode
            $content = $_.ErrorDetails.Message
            if ([string]::IsNullOrEmpty($content)) {
                $stream = $_.Exception.Response.GetResponseStream()
                if ($stream) {
                    if ($stream.CanSeek) { $stream.Position = 0 }
                    $reader = New-Object System.IO.StreamReader($stream)
                    $content = $reader.ReadToEnd()
                }
            }
        } else {
            $content = $_.Exception.Message
        }
    }

    $safeName = $Name -replace '[^A-Za-z0-9._-]', '_'
    Write-Utf8NoBom -Path (Join-Path $rawDir "$safeName.json") -Content $content

    $redacted = [regex]::Replace(
        $content,
        '("(?:access_token|refresh_token)"\s*:\s*")[^"]*(")',
        '$1<REDACTED>$2'
    )
    Write-Utf8NoBom -Path (Join-Path $redactedDir "$safeName.json") -Content $redacted

    $script:results.Add([PSCustomObject]@{
        Name = $Name
        Method = $Method
        Path = $Path
        Status = $status
        Bytes = ([Text.Encoding]::UTF8.GetByteCount($content))
    })
}

function Login-Capture {
    param([string]$Role, [string]$Username)
    $deviceId = "baseline-$Role-device"
    $loginBody = @{
        username = $Username
        password = $Password
        device_id = $deviceId
    }
    Invoke-Capture -Name "auth-login-$Role" -Method "POST" -Path "/auth/login" -Body $loginBody

    $raw = Get-Content -LiteralPath (Join-Path $rawDir "auth-login-$Role.json") -Raw
    $json = $raw | ConvertFrom-Json
    if (-not $json.data.access_token) {
        throw "Login failed for $Role. Check fixture database and password."
    }

    return @{
        Role = $Role
        Username = $Username
        DeviceId = $deviceId
        AccessToken = $json.data.access_token
        RefreshToken = $json.data.refresh_token
    }
}

$sessions = @()
$sessions += Login-Capture -Role "admin" -Username "baselineadmin"
$sessions += Login-Capture -Role "admin_monitor" -Username "baselinemonitor"
$sessions += Login-Capture -Role "admin_satker" -Username "baselineadminsatker"
$sessions += Login-Capture -Role "user_satker" -Username "baselineusersatker"
$sessions += Login-Capture -Role "user" -Username "baselineuser"

$publicPaths = @(
    "/healthz",
    "/readyz",
    "/public/list/operasis",
    "/public/skenarios/33333333-3333-4333-8333-333333333331",
    "/public/skenarios/get/44444444-4444-4444-8444-444444444441",
    "/public/markers/list/44444444-4444-4444-8444-444444444441",
    "/public/text-markers/list/44444444-4444-4444-8444-444444444441",
    "/public/particles/list/44444444-4444-4444-8444-444444444441",
    "/public/buildings/list/44444444-4444-4444-8444-444444444441",
    "/public/lines/list/44444444-4444-4444-8444-444444444441",
    "/public/alurs/list/44444444-4444-4444-8444-444444444441",
    "/public/anims/list/44444444-4444-4444-8444-444444444441",
    "/public/last_positions/44444444-4444-4444-8444-444444444441",
    "/public/get/home/operasis/33333333-3333-4333-8333-333333333331",
    "/public/game-sessions/snapshot/44444444-4444-4444-8444-444444444441",
    "/public/users/online"
)

foreach ($path in $publicPaths) {
    Invoke-Capture -Name ("public" + ($path -replace '[^A-Za-z0-9]', '_')) -Method "GET" -Path $path
}

$privatePaths = @(
    "/private/identify",
    "/private/operasis?pageSize=10&pageNumber=1&search=",
    "/private/operasis/33333333-3333-4333-8333-333333333331",
    "/private/home/operasis",
    "/private/categories/all",
    "/private/users",
    "/private/satkers",
    "/private/game-sessions/snapshot/44444444-4444-4444-8444-444444444441"
)

foreach ($session in $sessions) {
    $headers = @{
        Authorization = "Bearer $($session.AccessToken)"
        "X-Device-ID" = $session.DeviceId
        Accept = "application/json"
    }
    foreach ($path in $privatePaths) {
        $name = "private-$($session.Role)-" + ($path -replace '[^A-Za-z0-9]', '_')
        Invoke-Capture -Name $name -Method "GET" -Path $path -Headers $headers
    }

    $adminPaths = @(
        "/admin/categories?pageSize=10&pageNumber=1&search=",
        "/admin/users/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaa3/sessions"
    )
    foreach ($path in $adminPaths) {
        $name = "admin-$($session.Role)-" + ($path -replace '[^A-Za-z0-9]', '_')
        Invoke-Capture -Name $name -Method "GET" -Path $path -Headers $headers
    }

    Invoke-Capture -Name "auth-identify-$($session.Role)" -Method "GET" -Path "/private/identify" -Headers $headers
    Invoke-Capture -Name "auth-refresh-$($session.Role)" -Method "POST" -Path "/auth/refresh" -Headers @{ "X-Device-ID" = $session.DeviceId } -Body @{
        refresh_token = $session.RefreshToken
        device_id = $session.DeviceId
    }
}

$meta = [PSCustomObject]@{
    captured_at = (Get-Date).ToUniversalTime().ToString("o")
    base_url = $BaseUrl
    source_commit = "91d364d0cfc5f411c11160bc4e02717be73f3a05"
    total_requests = $script:results.Count
    results = $script:results
}
$meta | ConvertTo-Json -Depth 10 | ForEach-Object {
    Write-Utf8NoBom -Path (Join-Path $OutDir "capture-meta.json") -Content $_
}

$summaryLines = New-Object System.Collections.Generic.List[string]
$summaryLines.Add("# Golden Capture Summary")
$summaryLines.Add("")
$summaryLines.Add("Captured at: $($meta.captured_at)")
$summaryLines.Add("")
$summaryLines.Add("| Request | Method | Path | Status | Bytes |")
$summaryLines.Add("|---|---|---|---:|---:|")
foreach ($item in $script:results) {
    $summaryLines.Add("| $($item.Name) | $($item.Method) | ``$($item.Path)`` | $($item.Status) | $($item.Bytes) |")
}
$summaryText = $summaryLines -join [Environment]::NewLine
Write-Utf8NoBom -Path (Join-Path $OutDir "capture-summary.generated.md") -Content $summaryText

Write-Output "Captured $($script:results.Count) responses"
Write-Output $OutDir
