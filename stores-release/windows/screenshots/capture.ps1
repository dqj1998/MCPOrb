<#
.SYNOPSIS
    Capture MCPOrb Runner screenshots for Windows Store listing.

.DESCRIPTION
    Uses Add-Type -AssemblyName System.Windows.Forms to capture screenshots.
    Run this while MCPOrb Runner is open on the tab you want to capture.

.PARAMETER OutputName
    Base name for the screenshot file. Default: timestamp-based.

.PARAMETER OutputDir
    Output directory. Default: same directory as this script.

.EXAMPLE
    .\stores-release\windows\screenshots\capture.ps1 -OutputName library

    Captures the active window to mcporb-runner-win-library-1920x1080.png

.NOTES
    Windows Store requires screenshots at least 1366x768 PNG.
    Recommended: 1920x1080 PNG.
#>

param(
    [string]$OutputName = "",
    [string]$OutputDir = ""
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

if (-not $OutputDir) {
    $OutputDir = Split-Path -Parent $MyInvocation.MyCommand.Path
}

# Generate filename
$resolution = "$([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width)x$([System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height)"
if ($OutputName) {
    $filename = "mcporb-runner-win-${OutputName}-${resolution}.png"
} else {
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $filename = "mcporb-runner-win-${timestamp}-${resolution}.png"
}

$outputPath = Join-Path $OutputDir $filename

Write-Host "╔══════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║   MCPOrb Runner — Screenshot Capture     ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Output: $outputPath" -ForegroundColor White
Write-Host ""

Write-Host "  Switch to MCPOrb Runner and focus the tab you want to capture." -ForegroundColor Yellow
Write-Host "  Press ENTER to capture (you have 5 seconds)... " -ForegroundColor Yellow
$null = $Host.UI.ReadLine()

# Capture the entire screen (simplest approach)
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

# Save as PNG
$bitmap.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)

$graphics.Dispose()
$bitmap.Dispose()

Write-Host ""
Write-Host "✓ Screenshot saved: $outputPath" -ForegroundColor Green
Write-Host "  Resolution: $resolution" -ForegroundColor Gray
Write-Host "  Size: $([math]::Round((Get-Item $outputPath).Length / 1KB, 1)) KB" -ForegroundColor Gray

Write-Host ""
Write-Host "Suggested captures: " -ForegroundColor Yellow
Write-Host "  1. Library tab (imported Orbs)" -ForegroundColor Gray
Write-Host "  2. MCP Config tab (STDIO config)" -ForegroundColor Gray
Write-Host "  3. HTTP tab (server settings)" -ForegroundColor Gray
Write-Host "  4. Settings tab" -ForegroundColor Gray
Write-Host ""
Write-Host "Example sequence:" -ForegroundColor Gray
Write-Host "  .\stores-release\windows\screenshots\capture.ps1 -OutputName library" -ForegroundColor Gray
Write-Host "  .\stores-release\windows\screenshots\capture.ps1 -OutputName config" -ForegroundColor Gray
Write-Host "  .\stores-release\windows\screenshots\capture.ps1 -OutputName http" -ForegroundColor Gray
Write-Host "  .\stores-release\windows\screenshots\capture.ps1 -OutputName settings" -ForegroundColor Gray
