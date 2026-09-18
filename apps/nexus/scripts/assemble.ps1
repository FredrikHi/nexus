<#
.SYNOPSIS
    Builds everything and lays it out the way a release ships:

        nexus.exe
        bin\integration-api.exe
        web\          the built web app
        auth\         index.cjs, migrate.cjs and their source maps
        pgsql\        PostgreSQL 18

    That folder is what `nexus install` runs from, what the service tests in CI
    use, and what the release zip will contain.

.EXAMPLE
    .\apps\nexus\scripts\assemble.ps1 -Destination C:\Nexus
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [string]$Destination
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$Destination = [IO.Path]::GetFullPath($Destination)

# Native commands do not throw on failure in PowerShell; this checks their exit
# code instead. And the reverse problem: cargo and npm write progress to stderr,
# which Windows PowerShell under "Stop" treats as a failure, so the preference
# is relaxed while they run.
function Invoke-Step([string]$What, [string]$Directory, [scriptblock]$Commands) {
    Write-Host "==> $What" -ForegroundColor Cyan
    Push-Location $Directory
    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Commands
        if ($LASTEXITCODE -ne 0) { throw "$What failed (exit code $LASTEXITCODE)" }
    } finally {
        $ErrorActionPreference = $previous
        Pop-Location
    }
}

# Replaces an earlier layout, but refuses to empty a folder that is not one: a
# mistyped -Destination must not cost anyone their files.
if (Test-Path $Destination) {
    $existing = @(Get-ChildItem -Force $Destination)
    if ($existing.Count -gt 0 -and -not (Test-Path (Join-Path $Destination "nexus.exe"))) {
        throw "$Destination is not empty and is not an earlier Nexus layout. Choose an empty or new folder."
    }
    Remove-Item -Recurse -Force $Destination
}
New-Item -ItemType Directory -Force -Path $Destination | Out-Null

Invoke-Step "nexus.exe" (Join-Path $repo "apps\nexus") { cargo build --release --locked }

$env:SQLX_OFFLINE = "true"   # compile the API's queries against the committed cache, not a live database
Invoke-Step "API" (Join-Path $repo "apps\api") { cargo build --release --locked }

Invoke-Step "web app" (Join-Path $repo "apps\web") { npm ci --no-audit --no-fund; if ($LASTEXITCODE -eq 0) { npm run build } }

Invoke-Step "auth bundle" (Join-Path $repo "apps\auth") { npm ci --no-audit --no-fund; if ($LASTEXITCODE -eq 0) { npm run bundle } }

# Cached in apps\nexus\pgsql between runs; copied, not downloaded, per layout.
Write-Host "==> PostgreSQL" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "fetch-postgres.ps1")
$pgsql = Join-Path $repo "apps\nexus\pgsql"

Write-Host "==> Laying out $Destination" -ForegroundColor Cyan
Copy-Item (Join-Path $repo "apps\nexus\target\release\nexus.exe") $Destination
New-Item -ItemType Directory -Path (Join-Path $Destination "bin") | Out-Null
Copy-Item (Join-Path $repo "apps\api\target\release\integration-api.exe") (Join-Path $Destination "bin")
Copy-Item -Recurse (Join-Path $repo "apps\web\dist") (Join-Path $Destination "web")
New-Item -ItemType Directory -Path (Join-Path $Destination "auth") | Out-Null
Copy-Item (Join-Path $repo "apps\auth\dist\*") (Join-Path $Destination "auth")
Copy-Item -Recurse $pgsql (Join-Path $Destination "pgsql")

$size = (Get-ChildItem $Destination -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("Assembled {0} ({1:N0} MB)" -f $Destination, $size) -ForegroundColor Green
