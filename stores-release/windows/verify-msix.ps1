<#
.SYNOPSIS
    Verify a built MCPOrb Runner MSIX package before Store submission.

.DESCRIPTION
    Checks the MSIX for required files, valid manifest, correct binaries,
    and produces a validation report.

.PARAMETER MsixPath
    Path to the .msix file. Default: target\msix\MCPOrbRunner.msix

.EXAMPLE
    .\stores-release\windows\verify-msix.ps1
#>

param([string]$MsixPath = "")

$ErrorActionPreference = "Stop"
$ScriptDir    = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot     = [System.IO.Path]::Combine($ScriptDir, "..", "..")
$ManifestPath = [System.IO.Path]::Combine($ScriptDir, "Package.appxmanifest")
$IconDir      = [System.IO.Path]::Combine($ScriptDir, "icons")

if (-not $MsixPath) {
    $MsixPath = [System.IO.Path]::Combine($RepoRoot, "target", "msix", "MCPOrbRunner.msix")
}

Write-Host "=== MCPOrb Runner MSIX Verification ===" -ForegroundColor Cyan
Write-Host ""

$passed   = 0
$failed   = 0
$warnings = 0

function Check {
    param([string]$Item, [scriptblock]$Condition, [string]$FailMsg)
    if (& $Condition) {
        Write-Host "  [PASS] $Item" -ForegroundColor Green
        $script:passed++
    } else {
        Write-Host "  [FAIL] $Item - $FailMsg" -ForegroundColor Red
        $script:failed++
    }
}

function Warn {
    param([string]$Item, [string]$Msg)
    Write-Host "  [WARN] $Item - $Msg" -ForegroundColor Yellow
    $script:warnings++
}

# -- 1. MSIX file exists --
Write-Host "-- 1. MSIX Package -----------------------------------------" -ForegroundColor White
Check "MSIX file exists" { Test-Path $MsixPath } "Not found at $MsixPath"
if (Test-Path $MsixPath) {
    $sizeBytes = (Get-Item $MsixPath).Length
    $sizeMB = [math]::Round($sizeBytes / 1MB, 2)
    Check "MSIX file size > 1 MB" { $sizeBytes -gt 1MB } "Only $sizeMB MB"
    Write-Host "       Size: $sizeMB MB" -ForegroundColor Gray
}

# -- 2. Package.appxmanifest --
Write-Host "-- 2. Package.appxmanifest ---------------------------------" -ForegroundColor White
Check "Manifest file exists" { Test-Path $ManifestPath } "Not found at $ManifestPath"

if (Test-Path $ManifestPath) {
    $manifestXml = Get-Content $ManifestPath -Raw

    $idMatch    = [regex]::Match($manifestXml, 'Name="([^"]+)"')
    $verMatch   = [regex]::Match($manifestXml, 'Version="(\d+\.\d+\.\d+\.\d+)"')
    $pubMatch   = [regex]::Match($manifestXml, 'Publisher="([^"]+)"')
    $dnMatch    = [regex]::Match($manifestXml, '<DisplayName>([^<]+)</DisplayName>')
    $exeMatch   = [regex]::Match($manifestXml, 'Executable="([^"]+)"')

    Check "Identity Name present"     { $idMatch.Success }  "Missing Identity Name"
    if ($idMatch.Success)  { Write-Host "       Identity: $($idMatch.Groups[1].Value)" -ForegroundColor Gray }

    Check "Identity Version present"  { $verMatch.Success } "Missing Version"
    if ($verMatch.Success) { Write-Host "       Version: $($verMatch.Groups[1].Value)" -ForegroundColor Gray }

    Check "Publisher present"         { $pubMatch.Success } "Missing Publisher"
    Check "DisplayName present"       { $dnMatch.Success }  "Missing DisplayName"
    Check "runFullTrust capability"   { $manifestXml -match "runFullTrust" } "Missing runFullTrust"
    Check "Executable specified"      { $exeMatch.Success } "Missing Executable attribute"
    if ($exeMatch.Success) {
        Check "Executable is mcporb-runner.exe" { $exeMatch.Groups[1].Value -eq "mcporb-runner.exe" } "Wrong: $($exeMatch.Groups[1].Value)"
    }
    Check "mcporb:// protocol"        { $manifestXml -match 'mcporb' } "Missing mcporb protocol"
}

# -- 3. Icons --
Write-Host "-- 3. Store Icons ------------------------------------------" -ForegroundColor White
$requiredIcons = @(
    @("StoreLogo.png", 50, 50),
    @("Square44x44Logo.png", 44, 44),
    @("SmallTile.png", 71, 71),
    @("Square150x150Logo.png", 150, 150),
    @("Wide310x150Logo.png", 310, 150),
    @("Square310x310Logo.png", 310, 310),
    @("SplashScreen.png", 620, 300),
    @("BadgeLogo.png", 24, 24)
)
foreach ($icon in $requiredIcons) {
    $name     = $icon[0]
    $iconPath = [System.IO.Path]::Combine($IconDir, $name)
    Check "Icon: $name" { Test-Path $iconPath } "Missing in $IconDir"
}

$presentIcons = Get-ChildItem -Path $IconDir -Filter "*.png" | ForEach-Object { $_.Name }
$extra = $presentIcons | Where-Object { $_ -notin ($requiredIcons | ForEach-Object { $_[0] }) }
foreach ($e in $extra) {
    Warn "Unexpected icon: $e" "Not in required set - verify it is intended"
}

# -- 4. Metadata --
Write-Host "-- 4. Store Metadata ---------------------------------------" -ForegroundColor White
$requiredMetadata = @("description.txt","keywords.txt","support_url.txt","privacy_url.txt","marketing_url.txt")
$metaDir = [System.IO.Path]::Combine($ScriptDir, "metadata")
foreach ($m in $requiredMetadata) {
    $path = [System.IO.Path]::Combine($metaDir, $m)
    Check "Metadata: $m" { Test-Path $path } "Missing in $metaDir"
}

$descPath = [System.IO.Path]::Combine($metaDir, "description.txt")
if (Test-Path $descPath) {
    $descLen = (Get-Content $descPath -Raw).Length
    if ($descLen -gt 100)  { Warn "description.txt" "Short desc >100 chars ($descLen)" }
    if ($descLen -gt 4000) { Warn "description.txt" "Long desc >4000 chars ($descLen)" }
}

# -- 5. Privacy Policy --
Write-Host "-- 5. Privacy Policy ---------------------------------------" -ForegroundColor White
$ppPath = [System.IO.Path]::Combine($ScriptDir, "privacy-policy.md")
Check "Privacy policy" { Test-Path $ppPath } "Missing privacy-policy.md"

# -- 6. Screenshots --
Write-Host "-- 6. Screenshots ------------------------------------------" -ForegroundColor White
$ssDir = [System.IO.Path]::Combine($ScriptDir, "screenshots")
$screenshots = Get-ChildItem -Path $ssDir -Filter "*.png" -ErrorAction SilentlyContinue
$ssCount = ($screenshots | Where-Object { $_.Name -notmatch '^capture-' }).Count
Check "At least 1 Store screenshot" { $ssCount -ge 1 } "No screenshots found"
if ($ssCount -lt 4) { Warn "Screenshots" "Only $ssCount found - Partner Center recommends 4+" }

# -- 7. MSIX contents --
Write-Host "-- 7. MSIX Contents ----------------------------------------" -ForegroundColor White
if (Test-Path $MsixPath) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem -ErrorAction SilentlyContinue
    $zip = $null
    try {
        $zip = [System.IO.Compression.ZipFile]::OpenRead($MsixPath)
        $entries = $zip.Entries | ForEach-Object { $_.FullName }

        Check "MSIX contains mcporb-runner.exe" { $entries -contains "mcporb-runner.exe" } "Missing main exe"
        Check "MSIX contains AppxManifest.xml"  { $entries -contains "AppxManifest.xml" } "Missing manifest"
        Check "MSIX contains Assets/" { ($entries | Where-Object { $_ -like "Assets/*" }).Count -gt 0 } "Missing Assets"

        if ($entries -contains "mcporb-runtime.exe") {
            Write-Host "  [INFO] mcporb-runtime.exe included" -ForegroundColor Cyan
        } else {
            Warn "mcporb-runtime.exe not in MSIX" "App cannot launch Orbs at runtime"
        }
        if ($entries -contains "mcporb-gateway-stdio.exe") {
            Write-Host "  [INFO] mcporb-gateway-stdio.exe included" -ForegroundColor Cyan
        }

        $assetIcons = $entries | Where-Object { $_ -like "Assets/*.png" }
        foreach ($icon in $requiredIcons) {
            $name = "Assets/$($icon[0])"
            Check "MSIX icon: $($icon[0])" { $assetIcons -contains $name } "Missing in package"
        }
    } catch {
        Warn "Could not inspect MSIX contents" $_.Exception.Message
    } finally {
        if ($zip) { $zip.Dispose() }
    }
} else {
    Warn "MSIX not found" "Skipping contents check"
}

# -- Summary --
Write-Host "`n============================================================" -ForegroundColor Cyan
$total = $passed + $failed
Write-Host "  Results: $passed passed, $failed failed, $warnings warnings" -ForegroundColor White
Write-Host "============================================================" -ForegroundColor Cyan

if ($failed -gt 0) {
    Write-Host "`nFAILED - fix issues above before submission." -ForegroundColor Red
    exit 1
} elseif ($warnings -gt 0) {
    Write-Host "`nPassed with $warnings warnings - review before submission." -ForegroundColor Yellow
    exit 0
} else {
    Write-Host "`nAll checks passed - ready for Store submission!" -ForegroundColor Green
    exit 0
}
