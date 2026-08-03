<#
.SYNOPSIS
    Sync Package.appxmanifest version from Cargo.toml.

.DESCRIPTION
    Reads version from crates/mcporb-runtime-app/Cargo.toml and updates the
    Package.appxmanifest <Identity Version="x.x.x.0" /> attribute.

.PARAMETER DryRun
    Show what would change without modifying the file.

.PARAMETER ManifestPath
    Path to Package.appxmanifest. Default: stores-release/windows/Package.appxmanifest

.EXAMPLE
    .\stores-release\windows\sync-version.ps1

.EXAMPLE
    .\stores-release\windows\sync-version.ps1 -DryRun
#>

param(
    [switch]$DryRun = $false,
    [string]$ManifestPath = ""
)

$ErrorActionPreference = "Stop"

# Resolve paths
$ScriptRoot = Split-Path -Parent $PSScriptRoot
$StoreWinDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$StoreWinDir\..\.."

if (-not $ManifestPath) {
    $ManifestPath = Join-Path $StoreWinDir "Package.appxmanifest"
}

$CargoToml = Join-Path (Join-Path (Join-Path $RepoRoot "crates") "mcporb-runtime-app") "Cargo.toml"

Write-Host "=== MCPOrb Runner - Version Sync ===" -ForegroundColor Cyan

# Read Cargo.toml version
if (-not (Test-Path $CargoToml)) {
    throw "Cargo.toml not found: $CargoToml"
}

$cargoContent = Get-Content $CargoToml -Raw
$versionMatch = [regex]::Match($cargoContent, 'version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success) {
    throw "Could not find version in Cargo.toml"
}
$cargoVersion = $versionMatch.Groups[1].Value
$msixVersion = "$cargoVersion.0"

Write-Host "  Cargo.toml version:  $cargoVersion" -ForegroundColor White
Write-Host "  MSIX version:        $msixVersion" -ForegroundColor White

# Read manifest
if (-not (Test-Path $ManifestPath)) {
    throw "Package.appxmanifest not found: $ManifestPath"
}

$manifestContent = Get-Content $ManifestPath -Raw
$manifestVersionMatch = [regex]::Match($manifestContent, '<Identity\s+Name="[^"]+"\s+Publisher="[^"]+"\s+Version="(\d+\.\d+\.\d+\.\d+)"')
if (-not $manifestVersionMatch.Success) {
    throw "Could not find Version attribute in Package.appxmanifest"
}
$currentManifestVersion = $manifestVersionMatch.Groups[1].Value

Write-Host "  Current manifest:   $currentManifestVersion" -ForegroundColor White

# Compare
if ($currentManifestVersion -eq $msixVersion) {
    Write-Host ""
    Write-Host "Versions are already in sync: $msixVersion" -ForegroundColor Green
    exit 0
}

Write-Host ""
Write-Host "  Update needed: $currentManifestVersion -> $msixVersion" -ForegroundColor Yellow

if ($DryRun) {
    Write-Host ""
    Write-Host "[DRY RUN] Would update Package.appxmanifest Version to $msixVersion" -ForegroundColor Yellow
    exit 0
}

# Update manifest - use [regex]::Replace for reliable escaping
$pattern = [regex]::Escape("""$currentManifestVersion""")
$updatedContent = $manifestContent -replace '(<Identity\s+Name="[^"]+"\s+Publisher="[^"]+"\s+)Version="\d+\.\d+\.\d+\.\d+"', ('${1}Version="' + $msixVersion + '"')
Set-Content -Path $ManifestPath -Value $updatedContent -NoNewline

Write-Host ""
Write-Host "Package.appxmanifest updated to version $msixVersion" -ForegroundColor Green
