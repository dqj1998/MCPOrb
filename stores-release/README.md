# MCPOrb Runner — Store Submission Materials

## Structure

```
stores-release/
├── macos/
│   ├── icons/
│   │   └── icon_1024x1024.png     # Required for App Store Connect
│   ├── screenshots/               # Add screenshots here before submission
│   │   └── README.txt             # Capture instructions
│   └── metadata/
│       ├── description.txt        # App Store description (en-US)
│       ├── keywords.txt           # Comma-separated keywords
│       ├── support_url.txt
│       ├── marketing_url.txt
│       └── privacy_url.txt
├── windows/
│   ├── icons/
│   │   ├── Square44x44Logo.png
│   │   ├── Square150x150Logo.png
│   │   ├── Square310x310Logo.png
│   │   ├── StoreLogo.png          # 50x50
│   │   └── Wide310x150Logo.png
│   ├── screenshots/               # Add screenshots here before submission
│   │   └── README.txt             # Capture instructions
│   └── metadata/
│       └── description.txt
└── README.md                      # This file
```

## Before Submission

1. **Screenshots** — Follow the instructions in each `screenshots/README.txt`
2. **macOS `.app` bundle** — Build with `cargo tauri build` (MAS profile)
3. **Windows `.msix`** — Build on Windows with `cargo tauri build`

## App Identity

- **Product Name**: MCPOrb Runner
- **Bundle ID**: `com.mcporb.runner`
- **Windows Store ID**: `9N7PR6PHJZ80`
- **Category**: Developer Tools
- **Publisher**: MCPOrb
- **Website**: https://mcporb.ai
- **Copyright**: © 2026 MCPOrb
- **License**: Apache-2.0

## Build Commands

### macOS (Mac App Store)
```bash
cargo tauri build -p mcporb-runtime-app --bundles dmg
```
The resulting `.app` bundle will be in `target/release/bundle/macos/`.

### Windows (Microsoft Store)
```powershell
# Sync version then build MSIX
.\stores-release\windows\sync-version.ps1
.\stores-release\windows\build-msix.ps1 -SkipSign
```
The resulting `.msix` will be in `target\msix\MCPOrbRunner.msix`.
Upload to Partner Center → **Packages** → upload MSIX file.
