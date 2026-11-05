param(
    [ValidateSet("auto", "podman", "docker")]
    [string]$Engine = "auto",
    [string]$Image = "postgres:18.4-alpine",
    [switch]$Reset,
    [switch]$SkipImageCheck
)

$ErrorActionPreference = "Stop"

$fixturesDir = Resolve-Path (Join-Path $PSScriptRoot "..\fixtures")
$migrationsDir = Join-Path $fixturesDir "migrations"
$containerName = "twg-baseline-postgres"
$volumeName = "twg_baseline_pgdata"
$port = 55432

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

& $engine info *> $null
if ($LASTEXITCODE -ne 0) {
    throw "$engine daemon/engine is not reachable. Start it, then run this script again."
}

if (-not $SkipImageCheck) {
    & $engine image exists $Image *> $null
    if ($LASTEXITCODE -ne 0) {
        throw "Image '$Image' is not available. Pull it first: $engine pull $Image"
    }
}

if ($Reset) {
    & $engine rm -f $containerName *> $null
    & $engine volume rm -f $volumeName *> $null
} else {
    & $engine rm -f $containerName *> $null
}

& $engine volume exists $volumeName *> $null
if ($LASTEXITCODE -ne 0) {
    & $engine volume create $volumeName | Out-Null
}

Write-Output "Starting $Image on port $port via $engine..."
& $engine run -d `
    --name $containerName `
    -e POSTGRES_DB=twg_baseline `
    -e POSTGRES_USER=twg_baseline `
    -e POSTGRES_PASSWORD=twg_baseline_local `
    -p "${port}:5432" `
    -v "${volumeName}:/var/lib/postgresql" `
    $Image | Out-Null

if ($LASTEXITCODE -ne 0) {
    throw "Failed to start PostgreSQL container."
}

$ready = $false
for ($i = 0; $i -lt 90; $i++) {
    & $engine exec $containerName pg_isready -U twg_baseline -d twg_baseline *> $null
    if ($LASTEXITCODE -eq 0) {
        $ready = $true
        break
    }
    Start-Sleep -Seconds 1
}

if (-not $ready) {
    throw "PostgreSQL did not become ready in time."
}

& $engine exec $containerName mkdir -p /tmp/twg-fixtures/migrations
if ($LASTEXITCODE -ne 0) { throw "Failed to create fixture directory in container." }

& $engine cp (Join-Path $fixturesDir "bootstrap.sql") "${containerName}:/tmp/twg-fixtures/bootstrap.sql"
if ($LASTEXITCODE -ne 0) { throw "Failed to copy bootstrap.sql." }

& $engine cp $migrationsDir "${containerName}:/tmp/twg-fixtures/"
if ($LASTEXITCODE -ne 0) { throw "Failed to copy migrations." }

& $engine cp (Join-Path $fixturesDir "seed.sql") "${containerName}:/tmp/twg-fixtures/seed.sql"
if ($LASTEXITCODE -ne 0) { throw "Failed to copy seed.sql." }

& $engine exec $containerName psql -v ON_ERROR_STOP=1 -U twg_baseline -d twg_baseline -f /tmp/twg-fixtures/bootstrap.sql
if ($LASTEXITCODE -ne 0) { throw "Bootstrap SQL failed." }

$migrations = Get-ChildItem -LiteralPath $migrationsDir -File -Filter "*.up.sql" | Sort-Object Name
foreach ($migration in $migrations) {
    Write-Output "Applying $($migration.Name)"
    $containerPath = "/tmp/twg-fixtures/migrations/$($migration.Name)"
    & $engine exec $containerName psql -v ON_ERROR_STOP=1 -U twg_baseline -d twg_baseline -f $containerPath
    if ($LASTEXITCODE -ne 0) {
        throw "Migration failed: $($migration.Name)"
    }
}

& $engine exec $containerName psql -v ON_ERROR_STOP=1 -U twg_baseline -d twg_baseline -f /tmp/twg-fixtures/seed.sql
if ($LASTEXITCODE -ne 0) { throw "Seed SQL failed." }

& $engine exec $containerName psql -U twg_baseline -d twg_baseline -c "SELECT 'users' AS table_name, count(*) FROM users UNION ALL SELECT 'satkers', count(*) FROM satkers UNION ALL SELECT 'operasis', count(*) FROM operasis UNION ALL SELECT 'skenarios', count(*) FROM skenarios UNION ALL SELECT 'markers', count(*) FROM markers UNION ALL SELECT 'anims', count(*) FROM anims;"

Write-Output ""
Write-Output "Baseline database ready via $engine."
Write-Output "Container: $containerName"
Write-Output "DB_SOURCE=postgresql://twg_baseline:twg_baseline_local@127.0.0.1:55432/twg_baseline?sslmode=disable"
Write-Output "Next: scripts/run-baseline-server.ps1"
