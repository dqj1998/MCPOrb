<#
.SYNOPSIS
    Build MCPOrb Runner MSIX package for Windows Store submission.

.DESCRIPTION
    Builds MCPOrb Runner and packages into MSIX using Windows SDK makeappx.exe.
    Output: target\msix\MCPOrbRunner.msix

.PARAMETER Configuration
    Build configuration: Release (default) or Debug.

.PARAMETER SkipBuild
    Skip cargo tauri build step (use existing target/release/mcporb-runner.exe).

.PARAMETER SkipSign
    Skip code signing (Microsoft signs Store submissions automatically).

.EXAMPLE
    .\stores-release\windows\build-msix.ps1
    .\stores-release\windows\build-msix.ps1 -SkipBuild -SkipSign
#>

param(
    [ValidateSet("Release","Debug")]
    [string]$Configuration = "Release",
    [switch]$SkipBuild = $false,
    [switch]$SkipSign = $false
)

$ErrorActionPreference = "Stop"

# ── Paths ───────────────────────────────────────────────────────────────────
$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot   = [System.IO.Path]::Combine($ScriptDir, "..", "..")
$AppCrate   = [System.IO.Path]::Combine($RepoRoot, "crates", "mcporb-runtime-app")
$Manifest   = [System.IO.Path]::Combine($ScriptDir, "Package.appxmanifest")
$IconDir    = [System.IO.Path]::Combine($ScriptDir, "icons")
$StageDir   = [System.IO.Path]::Combine($RepoRoot, "target", "msix-stage")
$OutputDir  = [System.IO.Path]::Combine($RepoRoot, "target", "msix")
$MsixPath   = [System.IO.Path]::Combine($OutputDir, "MCPOrbRunner.msix")
$configDir  = if ($Configuration -eq "Release") { "release" } else { "debug" }
$ExePath    = [System.IO.Path]::Combine($RepoRoot, "target", $configDir, "mcporb-runner.exe")

# ── SDK tool finder ─────────────────────────────────────────────────────────
function Find-SdkTool {
    param([string]$name)
    $c = Get-Command "$name.exe" -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }
    $patterns = @(
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\$name.exe",
        "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x86\$name.exe",
        "${env:ProgramFiles}\Windows Kits\10\bin\*\x64\$name.exe",
        "${env:ProgramFiles}\Windows Kits\10\bin\*\x86\$name.exe"
    )
    foreach ($p in $patterns) {
        $found = Resolve-Path $p -ErrorAction SilentlyContinue
        if ($found) { return ($found | Select-Object -First 1).Path }
    }
    return $null
}

Write-Host "=== MCPOrb Runner MSIX Build ===" -ForegroundColor Cyan
Write-Host ""

# ── Check prerequisites ─────────────────────────────────────────────────────
$makeappx = Find-SdkTool "makeappx"
if (-not $makeappx) { throw "makeappx.exe not found. Install Windows SDK." }
Write-Host "  makeappx: $makeappx" -ForegroundColor Green

if (-not (Test-Path $Manifest)) { throw "Manifest not found: $Manifest" }
$icons = "StoreLogo.png","Square44x44Logo.png","SmallTile.png","Square150x150Logo.png","Wide310x150Logo.png","Square310x310Logo.png","SplashScreen.png","BadgeLogo.png"
$missing = $icons | Where-Object { -not (Test-Path ([System.IO.Path]::Combine($IconDir, $_))) }
if ($missing) { throw "Missing icons: $($missing -join ', ')" }
Write-Host "  Icons: OK" -ForegroundColor Green

# ── Step 1: Build ───────────────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "`n[1/3] Building MCPOrb Runner..." -ForegroundColor Yellow
    $flag = if ($Configuration -eq "Debug") { "--debug" } else { "" }
    Push-Location $AppCrate
    try { & cargo tauri build $flag; if ($LASTEXITCODE -ne 0) { throw "Build failed" } }
    finally { Pop-Location }
    Write-Host "  Build OK" -ForegroundColor Green
} else {
    Write-Host "`n[1/3] Skipping build" -ForegroundColor Yellow
}

# ── Step 2: Stage ───────────────────────────────────────────────────────────
Write-Host "`n[2/3] Staging MSIX content..." -ForegroundColor Yellow

if (-not (Test-Path $ExePath)) { throw "Executable not found: $ExePath" }
if (Test-Path $StageDir) { Remove-Item -Path $StageDir -Recurse -Force }
New-Item -ItemType Directory -Path ([System.IO.Path]::Combine($StageDir, "Assets")) -Force | Out-Null

# Manifest must be named AppxManifest.xml for makeappx
Copy-Item -Path $Manifest -Destination ([System.IO.Path]::Combine($StageDir, "AppxManifest.xml")) -Force

# Icons
foreach ($i in $icons) {
    Copy-Item -Path ([System.IO.Path]::Combine($IconDir, $i)) -Destination ([System.IO.Path]::Combine($StageDir, "Assets", $i)) -Force
}

# Executable
Copy-Item -Path $ExePath -Destination ([System.IO.Path]::Combine($StageDir, "mcporb-runner.exe")) -Force

$count = (Get-ChildItem -Path $StageDir -Recurse -File).Count
Write-Host "  Staged $count files" -ForegroundColor Green

# ── Step 3: Pack ────────────────────────────────────────────────────────────
Write-Host "`n[3/3] Creating MSIX package..." -ForegroundColor Yellow

if (-not (Test-Path $OutputDir)) { New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null }

& $makeappx pack /p $MsixPath /d $StageDir /l
if ($LASTEXITCODE -ne 0) { throw "makeappx.exe failed (exit $LASTEXITCODE)" }
if (-not (Test-Path $MsixPath)) { throw "MSIX not created" }

$size = [math]::Round((Get-Item $MsixPath).Length / 1MB, 2)
Write-Host "  MSIX created: $MsixPath ($size MB)" -ForegroundColor Green

# ── Sign (optional) ─────────────────────────────────────────────────────────
if (-not $SkipSign) {
    $signtool = Find-SdkTool "signtool"
    if ($signtool) {
        $cert = Get-ChildItem Cert:\LocalMachine\My | Where-Object { $_.Subject -eq "CN=MCPOrb" } | Select-Object -First 1
        if (-not $cert) {
            $cert = New-SelfSignedCertificate -Subject "CN=MCPOrb" -CertStoreLocation Cert:\LocalMachine\My -Type CodeSigningCert -KeyUsage DigitalSignature
        }
        if ($cert) {
            & $signtool sign /fd SHA256 /a /s My /n "CN=MCPOrb" /v $MsixPath
            if ($LASTEXITCODE -eq 0) { Write-Host "  Signed OK" -ForegroundColor Green }
        }
    }
} else {
    Write-Host "  (unsigned - Microsoft signs on Store submission)" -ForegroundColor Yellow
}

# ── Done ────────────────────────────────────────────────────────────────────
Write-Host "`n=== BUILD COMPLETE ===" -ForegroundColor Cyan
Write-Host "  $MsixPath ($size MB)" -ForegroundColor White
Write-Host ""
if (-not $SkipSign) {
    Write-Host "  Test: Add-AppxPackage -Path `"$MsixPath`"" -ForegroundColor Gray
}
