<#
.SYNOPSIS
    Downloads the PostgreSQL build nexus.exe runs, into apps/nexus/pgsql.

    Used for local development and by the release build, so both get exactly the
    same binaries. The checksum is pinned here rather than read from next to the
    download: a hash fetched from the same place as the file only proves the
    transfer finished, not that the file is the one we tested against.

.EXAMPLE
    .\apps\nexus\scripts\fetch-postgres.ps1
    $env:PG_DIR = (Resolve-Path .\apps\nexus\pgsql)
#>
[CmdletBinding()]
param(
    [string]$Destination = (Join-Path $PSScriptRoot "..\pgsql")
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"   # Invoke-WebRequest is many times slower with it on 5.1

# Built by theseus-rs from the official sources, the same builds the
# postgresql_embedded crate uses. Bump all three together.
$Version = "18.6.0"
$Asset = "postgresql-$Version-x86_64-pc-windows-msvc.zip"
$Sha256 = "dd4bcd50b38bf5b9f8a6e0d6736bd4412a79a066639cb855479d4d58b1dd74ac"

$Destination = [IO.Path]::GetFullPath($Destination)
$marker = Join-Path $Destination "NEXUS_POSTGRES_VERSION"
if ((Test-Path $marker) -and ((Get-Content $marker -Raw).Trim() -eq $Version)) {
    Write-Host "PostgreSQL $Version already in $Destination"
    exit 0
}

$work = Join-Path ([IO.Path]::GetTempPath()) "nexus-postgres-$Version"
New-Item -ItemType Directory -Force -Path $work | Out-Null
$zip = Join-Path $work $Asset

Write-Host "Downloading $Asset..."
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -UseBasicParsing -OutFile $zip `
    -Uri "https://github.com/theseus-rs/postgresql-binaries/releases/download/$Version/$Asset"

$actual = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $Sha256) {
    Remove-Item $zip
    throw "Checksum mismatch for $Asset. Expected $Sha256, got $actual."
}

Write-Host "Extracting..."
$extracted = Join-Path $work "extracted"
if (Test-Path $extracted) { Remove-Item -Recurse -Force $extracted }
Expand-Archive -Path $zip -DestinationPath $extracted
$root = Get-ChildItem $extracted -Directory | Select-Object -First 1

if (Test-Path $Destination) { Remove-Item -Recurse -Force $Destination }
New-Item -ItemType Directory -Force -Path $Destination | Out-Null

# What the server and its tools need to run, and the licences that must travel
# with them. Not the headers, and not StackBuilder, an installer for add-ons.
foreach ($item in "bin", "lib", "share", "LICENSE", "server_license.txt", "commandlinetools_3rd_party_licenses.txt") {
    $source = Join-Path $root.FullName $item
    if (Test-Path $source) { Copy-Item -Recurse -Path $source -Destination $Destination }
}
Set-Content -Path $marker -Value $Version -Encoding ASCII

Remove-Item -Recurse -Force $work
$size = (Get-ChildItem $Destination -Recurse | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("PostgreSQL {0} in {1} ({2:N0} MB)" -f $Version, $Destination, $size)
