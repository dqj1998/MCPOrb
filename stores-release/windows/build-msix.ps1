<#
.SYNOPSIS
    Build MCPOrb Runner MSIX package for Windows Store submission.

.DESCRIPTION
    Builds the mcporb-runtime sidecar, the Tauri app, and packages into MSIX
    using Windows SDK makeappx.exe.  Output: target\msix\MCPOrbRunner.msix

.PARAMETER Configuration
    Build configuration: Release (default) or Debug.

.PARAMETER SkipBuild
    Skip all build steps (use existing binaries under target/).

.PARAMETER SkipSign
    Skip code signing (Microsoft signs Store submissions automatically).

.PARAMETER SkipSidecar
    Skip building sidecar binaries (mcporb-runtime, mcporb-gateway-stdio).
    Has no effect when -SkipBuild is set.

.EXAMPLE
    .\stores-release\windows\build-msix.ps1
    .\stores-release\windows\build-msix.ps1 -SkipBuild -SkipSign
    .\stores-release\windows\build-msix.ps1 -SkipSidecar
#>

param(
    [ValidateSet("Release","Debug")]
    [string]$Configuration = "Release",
    [switch]$SkipBuild = $false,
    [switch]$SkipSign = $false,
    [switch]$SkipSidecar = $false
)

$ErrorActionPreference = "Stop"
$configDir  = if ($Configuration -eq "Release") { "release" } else { "debug" }

# ── Paths ───────────────────────────────────────────────────────────────────
$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot   = [System.IO.Path]::Combine($ScriptDir, "..", "..")
$AppCrate   = [System.IO.Path]::Combine($RepoRoot, "crates", "mcporb-runtime-app")
$Manifest   = [System.IO.Path]::Combine($ScriptDir, "Package.appxmanifest")
$IconDir    = [System.IO.Path]::Combine($ScriptDir, "icons")
$StageDir   = [System.IO.Path]::Combine($RepoRoot, "target", "msix-stage")
$OutputDir  = [System.IO.Path]::Combine($RepoRoot, "target", "msix")
$MsixPath   = [System.IO.Path]::Combine($OutputDir, "MCPOrbRunner.msix")

$TargetDir   = [System.IO.Path]::Combine($RepoRoot, "target", $configDir)
$ExePath     = [System.IO.Path]::Combine($TargetDir, "mcporb-runner.exe")
$RuntimePath = [System.IO.Path]::Combine($TargetDir, "mcporb-runtime.exe")
$GatewayPath = [System.IO.Path]::Combine($TargetDir, "mcporb-gateway-stdio.exe")

# Sidecar crates
$RuntimeCrate   = [System.IO.Path]::Combine($RepoRoot, "crates", "mcporb-runtime")
$GatewayCrate   = [System.IO.Path]::Combine($RepoRoot, "crates", "mcporb-gateway-stdio")

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

# ── Rust build helper ───────────────────────────────────────────────────────
function Invoke-CargoBuild {
    param([string]$CrateDir, [string]$PackageName)
    $crateName = Split-Path -Leaf $CrateDir
    Write-Host "  Building $crateName ($PackageName, $Configuration)..." -ForegroundColor White
    $flag = if ($Configuration -eq "Debug") { "" } else { "--release" }
    Push-Location $RepoRoot
    try {
        # Native (cargo) stderr reporting triggers NativeCommandError under
        # $ErrorActionPreference=Stop; temporarily Continue in *this* scope and
        # merge streams so progress output is not mistaken for failure.
        $prev = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        & cargo build $flag --package $PackageName 2>&1 | Out-Host
        $code = $LASTEXITCODE
        $ErrorActionPreference = $prev
        if ($code -ne 0) { throw "Build failed for $PackageName" }
    }
    finally { Pop-Location }
    Write-Host "    $PackageName OK" -ForegroundColor Green
}

Write-Host "=== MCPOrb Runner MSIX Build ===" -ForegroundColor Cyan
Write-Host "  Configuration: $Configuration" -ForegroundColor White
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

# ── Step 0: Build sidecar binaries (mcporb-runtime + mcporb-gateway-stdio) ──
if (-not $SkipBuild -and -not $SkipSidecar) {
    Write-Host "`n[0/4] Building sidecar binaries..." -ForegroundColor Yellow

    # mcporb-runtime (full / default features = vector-embedder)
    Invoke-CargoBuild -CrateDir $RuntimeCrate -PackageName "mcporb-runtime"

    # mcporb-gateway-stdio (if the crate directory exists)
    if (Test-Path $GatewayCrate) {
        Invoke-CargoBuild -CrateDir $GatewayCrate -PackageName "mcporb-gateway-stdio"
    } else {
        Write-Host "    mcporb-gateway-stdio crate not found, skipping" -ForegroundColor Yellow
    }

    Write-Host "  Sidecar build OK" -ForegroundColor Green
} else {
    Write-Host "`n[0/4] Skipping sidecar build" -ForegroundColor Yellow
}

# ── Step 1: Build Tauri app ─────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "`n[1/4] Building MCPOrb Runner (Tauri)..." -ForegroundColor Yellow
    $flag = if ($Configuration -eq "Debug") { "--debug" } else { "" }
    Push-Location $AppCrate
    try {
        # ── Tauri v2: build sidecar binary paths listed in externalBin
        #    must exist *before* cargo tauri build runs, otherwise Tauri's
        #    bundler cannot discover them.  We build them in step 0 above.
        #    --no-bundle: MSIX is produced by makeappx below; tauri's msi/nsis
        #    bundler needs WiX/external tooling that is absent on this machine.
        $prev = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        & cargo tauri build $flag --no-bundle 2>&1 | Out-Host
        $code = $LASTEXITCODE
        $ErrorActionPreference = $prev
        if ($code -ne 0) { throw "cargo tauri build failed" }
    }
    finally { Pop-Location }
    Write-Host "  Build OK" -ForegroundColor Green
} else {
    Write-Host "`n[1/4] Skipping Tauri build" -ForegroundColor Yellow
}

# ── Step 2: Stage ───────────────────────────────────────────────────────────
Write-Host "`n[2/4] Staging MSIX content..." -ForegroundColor Yellow

if (-not (Test-Path $ExePath)) { throw "Executable not found: $ExePath" }
if (Test-Path $StageDir) { Remove-Item -Path $StageDir -Recurse -Force }
New-Item -ItemType Directory -Path ([System.IO.Path]::Combine($StageDir, "Assets")) -Force | Out-Null

# Manifest must be named AppxManifest.xml for makeappx
Copy-Item -Path $Manifest -Destination ([System.IO.Path]::Combine($StageDir, "AppxManifest.xml")) -Force

# Icons
foreach ($i in $icons) {
    Copy-Item -Path ([System.IO.Path]::Combine($IconDir, $i)) -Destination ([System.IO.Path]::Combine($StageDir, "Assets", $i)) -Force
}

# Main executable
Copy-Item -Path $ExePath -Destination ([System.IO.Path]::Combine($StageDir, "mcporb-runner.exe")) -Force

# Sidecar: mcporb-runtime.exe (required at runtime via default_runtime_binary())
if (Test-Path $RuntimePath) {
    Copy-Item -Path $RuntimePath -Destination ([System.IO.Path]::Combine($StageDir, "mcporb-runtime.exe")) -Force
    Write-Host "  Staged mcporb-runtime.exe" -ForegroundColor Gray
} else {
    Write-Host "  WARNING: mcporb-runtime.exe not found at $RuntimePath" -ForegroundColor Red
}

# Sidecar: mcporb-gateway-stdio.exe (optional, for gateway config snippets)
if (Test-Path $GatewayPath) {
    Copy-Item -Path $GatewayPath -Destination ([System.IO.Path]::Combine($StageDir, "mcporb-gateway-stdio.exe")) -Force
    Write-Host "  Staged mcporb-gateway-stdio.exe" -ForegroundColor Gray
} else {
    Write-Host "  (mcporb-gateway-stdio.exe not found, gateway disabled in build)" -ForegroundColor Yellow
}

$count = (Get-ChildItem -Path $StageDir -Recurse -File).Count
Write-Host "  Staged $count files" -ForegroundColor Green

# ── Step 3: Pack ────────────────────────────────────────────────────────────
Write-Host "`n[3/4] Creating MSIX package..." -ForegroundColor Yellow

if (-not (Test-Path $OutputDir)) { New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null }

& $makeappx pack /p $MsixPath /d $StageDir /l /o
if ($LASTEXITCODE -ne 0) { throw "makeappx.exe failed (exit $LASTEXITCODE)" }
if (-not (Test-Path $MsixPath)) { throw "MSIX not created" }

$size = [math]::Round((Get-Item $MsixPath).Length / 1MB, 2)
Write-Host "  MSIX created: $MsixPath ($size MB)" -ForegroundColor Green

# ── Step 4: Sign (optional) ─────────────────────────────────────────────────
if (-not $SkipSign) {
    Write-Host "`n[4/4] Signing MSIX..." -ForegroundColor Yellow
    $signtool = Find-SdkTool "signtool"
    if ($signtool) {
        $cert = Get-ChildItem Cert:\LocalMachine\My | Where-Object { $_.Subject -eq "CN=MCPOrb" } | Select-Object -First 1
        if (-not $cert) {
            Write-Host "  Creating self-signed certificate (CN=MCPOrb)..." -ForegroundColor Yellow
            $cert = New-SelfSignedCertificate -Subject "CN=MCPOrb" -CertStoreLocation Cert:\LocalMachine\My -Type CodeSigningCert -KeyUsage DigitalSignature
        }
        if ($cert) {
            & $signtool sign /fd SHA256 /a /s My /n "CN=MCPOrb" /v $MsixPath
            if ($LASTEXITCODE -eq 0) { Write-Host "  Signed OK" -ForegroundColor Green }
            else { Write-Host "  Signing FAILED" -ForegroundColor Red }
        }
    } else {
        Write-Host "  signtool not found, skipping sign" -ForegroundColor Yellow
    }
} else {
    Write-Host "`n[4/4] Skipping sign (Microsoft signs on Store submission)" -ForegroundColor Yellow
}

# ── Done ────────────────────────────────────────────────────────────────────
Write-Host "`n=== BUILD COMPLETE ===" -ForegroundColor Cyan
Write-Host "  $MsixPath ($size MB)" -ForegroundColor White
Write-Host ""
Write-Host "  MSIX contents:" -ForegroundColor Gray
Get-ChildItem -Path $StageDir -Recurse -File | ForEach-Object {
    $name = $_.Name
    $len = "{0,8:F1} KB" -f ($_.Length / 1KB)
    if ($_.Name -match "\.(exe|dll)$") {
        $ver = (Get-Item $_.FullName).VersionInfo.ProductVersion
        Write-Host "    $len  $name  (v$ver)" -ForegroundColor Gray
    } else {
        Write-Host "    $len  $name" -ForegroundColor Gray
    }
}
Write-Host ""
if (-not $SkipSign) {
    Write-Host "  Test install: Add-AppxPackage -Path `"$MsixPath`"" -ForegroundColor Gray
    Write-Host "  Uninstall:    Get-AppxPackage *MCPOrb* | Remove-AppxPackage" -ForegroundColor Gray
}
