# Windows Store Release Materials

This directory contains all materials needed to publish MCPOrb Runner to the Microsoft Store.

## Contents

```
windows/
├── SUBMISSION-GUIDE.md          # Step-by-step submission guide (START HERE)
├── CHECKLIST.md                 # Complete pre-submission checklist
├── Package.appxmanifest         # MSIX package manifest
├── build-msix.ps1               # Build MSIX package from Tauri build output
├── sync-version.ps1             # Sync Package.appxmanifest version from Cargo.toml
├── icons/                       # Store icons (8 sizes, all ready)
│   ├── StoreLogo.png           # 50x50
│   ├── Square44x44Logo.png     # 44x44
│   ├── SmallTile.png           # 71x71
│   ├── Square150x150Logo.png   # 150x150
│   ├── Wide310x150Logo.png     # 310x150
│   ├── Square310x310Logo.png   # 310x310
│   ├── SplashScreen.png        # 620x300
│   └── BadgeLogo.png           # 24x24
├── screenshots/                 # Store screenshots
│   ├── capture.ps1             # Screenshot capture helper script
│   └── README.txt              # Capture instructions
├── metadata/                    # Listing text content
│   ├── description.txt         # App description
│   ├── keywords.txt            # Search keywords
│   ├── marketing_url.txt       # Marketing URL
│   ├── privacy_url.txt         # Privacy policy URL
│   └── support_url.txt         # Support URL
├── privacy-policy.md            # Privacy policy document
└── README.md                    # This file
```

## Quick Start

1. Read `SUBMISSION-GUIDE.md` for the full workflow
2. Run `sync-version.ps1` to ensure version matches Cargo.toml
3. Capture screenshots from the running app (or verify existing ones)
4. Build the MSIX package with `build-msix.ps1`
5. Upload to Microsoft Partner Center

## Files Status

| File | Status | Notes |
|------|--------|-------|
| Icons | ✅ Ready | All 8 required sizes generated |
| Metadata | ✅ Ready | Description, keywords, URLs |
| Manifest | ✅ Ready | Package.appxmanifest |
| Privacy Policy | ✅ Ready | privacy-policy.md |
| Screenshots | ✅ Existing | Verify/recapture as needed |
| Build Script | ✅ Added | build-msix.ps1 |
| Version Sync | ✅ Added | sync-version.ps1 |
| Screenshot Tool | ✅ Added | screenshots/capture.ps1 |
| Checklist | ✅ Added | CHECKLIST.md |

## Build Commands

```powershell
# Sync version with Cargo.toml
.\stores-release\windows\sync-version.ps1

# Dry-run version sync (no modification)
.\stores-release\windows\sync-version.ps1 -DryRun

# Build MSIX package (full build + pack + sign)
.\stores-release\windows\build-msix.ps1

# Build MSIX using existing artifacts, skip signing
.\stores-release\windows\build-msix.ps1 -SkipBuild -SkipSign

# Capture screenshot
.\stores-release\windows\screenshots\capture.ps1 -OutputName library

# Test install
Add-AppxPackage -Path "target\msix\MCPOrbRunner.msix"
```
