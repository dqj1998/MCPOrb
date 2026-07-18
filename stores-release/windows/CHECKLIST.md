# MCPOrb Runner — Windows Store Submission Checklist

> **Product**: MCPOrb Runner
> **Version**: 1.2.0
> **Publisher**: MCPOrb
> **Category**: Developer Tools

---

## Prerequisites

- [ ] **Windows 10/11** (build machine)
- [ ] **Windows SDK** installed (for `makeappx.exe` and `signtool.exe`)
      Download: https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/
- [ ] **Rust toolchain** installed (`rustup`)
- [ ] **Microsoft Partner Center** developer account (one-time $19 USD)
      Register: https://partner.microsoft.com/dashboard
- [ ] **App registration** created in Partner Center → **Applications and Games**
      → Reserved app name: `MCPOrb Runner`

---

## Material Preparation

### Icons
- [x] StoreLogo.png (50x50) — `stores-release/windows/icons/`
- [x] Square44x44Logo.png (44x44)
- [x] SmallTile.png (71x71)
- [x] Square150x150Logo.png (150x150)
- [x] Wide310x150Logo.png (310x150)
- [x] Square310x310Logo.png (310x310)
- [x] SplashScreen.png (620x300)
- [x] BadgeLogo.png (24x24)

### Screenshots
- [ ] Library tab — shows imported Orbs
- [ ] MCP Config tab — STDIO configuration snippets
- [ ] HTTP tab — HTTP MCP server settings
- [ ] Settings tab — application settings

> **Requirements**: ≥1366×768, PNG format, 1–8 images
> **Capture tool**: `.\stores-release\windows\screenshots\capture.ps1 -OutputName <tab>`

### Metadata
- [x] Short description (≤100 chars) — `metadata/description.txt`
- [x] Long description (≤4000 chars) — `metadata/description.txt`
- [x] Keywords (≤10) — `metadata/keywords.txt`
- [x] Marketing URL — `metadata/marketing_url.txt`
- [x] Privacy URL — `metadata/privacy_url.txt`
- [x] Support URL — `metadata/support_url.txt`
- [x] Privacy policy document — `privacy-policy.md`

### App Manifest
- [x] `Package.appxmanifest` — Identity name, version, capabilities
- [x] Version synced with `Cargo.toml`:
      ```powershell
      .\stores-release\windows\sync-version.ps1
      ```

---

## Build & Package

- [x] **Sidecar built** (mcporb-runtime + mcporb-gateway-stdio):
      ```powershell
      cargo build --release -p mcporb-runtime
      cargo build --release -p mcporb-gateway-stdio
      ```
- [x] **Tauri app built** (mcporb-runner.exe):
      ```powershell
      cargo tauri build --release  # or via build-msix.ps1
      ```
      Bundles: `.msi` + `.exe` (NSIS) in `target/release/bundle/`
- [x] **MSIX packaged**:
      ```powershell
      .\stores-release\windows\build-msix.ps1
      ```
      Output: `target\msix\MCPOrbRunner.msix` (11.5 MB)
- [ ] **Verify MSIX contents**:
      ```powershell
      .\stores-release\windows\verify-msix.ps1
      ```
- [ ] **Test local install**:
      ```powershell
      Add-AppxPackage -Path "target\msix\MCPOrbRunner.msix"
      ```
- [ ] **Verify app launches** from Start Menu
- [ ] **Verify deep link** works: `mcporb://` protocol
- [ ] **Uninstall** after testing:
      ```powershell
      Get-AppxPackage *MCPOrb* | Remove-AppxPackage
      ```

---

## Partner Center Submission

### Store Listing
- [ ] Upload screenshots (4 recommended)
- [ ] Fill short description
- [ ] Fill long description
- [ ] Enter keywords
- [ ] Set category: **Developer Tools**
- [ ] Set sub-category: **Utilities** (optional)
- [ ] Enter URLs (marketing, privacy, support)
- [ ] Set age rating: **Everyone** (age 3+)
- [ ] Set copyright: `© 2026 MCPOrb`

### Packages
- [ ] Upload `MCPOrbRunner.msix`
- [ ] Verify platform target: **Windows 10/11 Desktop**

### Certification Notes
- [ ] App uses `runFullTrust` capability (desktop app)
- [ ] No sandbox violations expected
- [ ] WebView2 Runtime dependency noted

---

## Post-Release

- [ ] Verify Store listing visible
- [ ] Test install from Store on clean machine
- [ ] Update GitHub release with Store link
- [ ] Update project README with Store badge

---

## Quick Reference

| Action | Command |
|--------|---------|
| Sync version | `.\stores-release\windows\sync-version.ps1` |
| Build MSIX (full) | `.\stores-release\windows\build-msix.ps1` |
| Build MSIX (skip build) | `.\stores-release\windows\build-msix.ps1 -SkipBuild -SkipSign` |
| Build MSIX (skip sidecar) | `.\stores-release\windows\build-msix.ps1 -SkipSidecar` |
| Build MSIX (unsigned) | `.\stores-release\windows\build-msix.ps1 -SkipSign` |
| Verify MSIX | `.\stores-release\windows\verify-msix.ps1` |
| Capture screenshot | `.\stores-release\windows\screenshots\capture.ps1 -OutputName <tab>` |
| Install locally | `Add-AppxPackage -Path "target\msix\MCPOrbRunner.msix"` |
| Uninstall | `Get-AppxPackage *MCPOrb* \| Remove-AppxPackage` |
