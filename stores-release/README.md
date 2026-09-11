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

## macOS Release Checklist & Regression Prevention

> **Use `scripts/build-mas.sh`, not the bare `cargo tauri build` above, for any
> real Mac App Store / TestFlight build.** The externalBin sidecars
> (`mcporb-runtime`, `mcporb-gateway-stdio`, `mcporb-gateway-http`) are NOT
> cargo deps of the app, so `cargo tauri build` ships whatever stale binaries
> sit in `target/release/`. `build-mas.sh` rebuilds them, signs each nested
> executable inherit-only, and runs the anti-regression gates below.

### The bug class this guards against (Runner 1.5.0–1.5.2)

Reading an Orb library **outside the sandbox container** (e.g. `~/Documents/…`,
or the default `~/.mcporb/Orbs`) failed with *"security-scoped bookmark did not
grant access"* / *"bookmark could not be resolved"* / *"failed to spawn Orb"*.
It only reproduces under **signed + sandboxed + out-of-container + launched the
way Claude Desktop launches it** (`mcporb-runner --gateway-stdio`), so unit
tests, `cargo build`, and non-sandbox dev runs all pass while it is broken.

Root causes, all now fixed and guarded:

1. **Wrong FFI constant.** `kCFURLBookmarkResolutionWithSecurityScope` is
   `1 << 10`, not `1 << 11` (the *creation* value). Resolving without the
   security-scope flag drops the scope, so `startAccessing` always fails.
2. **Gateway re-resolved the bookmark** instead of using the access the Runner
   already started before `exec()`ing into it (sandbox extensions survive
   `execve()` on the same process). The inherit-only gateway can't resolve
   app-scoped bookmarks, so it must read the ZIP directly.
3. **Stale externalBin sidecars** shipped pre-fix gateway code (1.5.0).

### Automated gates (run by `build-mas.sh`)

- **Constant guard.** `cargo test -p mcporb-macos-access` runs before the build.
  `constants_match_sdk_header` parses the installed `CFURL.h` and fails if any
  bookmark constant drifts from Apple's value. A wrong bit → build fails.
- **Sidecar freshness.** The bundled gateways are grepped for the
  `security-scoped bookmark` marker; a stale pre-fix sidecar fails the build.
- All bookmark FFI lives in one crate, `mcporb-macos-access` (no more three
  divergent copies). It is `cfg(macos)`-gated and empty on Windows.

### Manual pre-release smoke (the part that can't be headless)

Minting a security-scoped bookmark requires the NSOpenPanel powerbox UI, so a
fully headless E2E is impossible. After installing the build and picking an Orb
library folder **outside the container** once (Settings → Choose…), run:

```bash
scripts/smoke-sandbox-orb.sh
```

It drives `mcporb-runner --gateway-stdio` over stdio and asserts an Orb actually
spawns — covering bookmark resolve + exec-inherited access + gateway direct read.
Exit 0 = the out-of-container path works; exit 1 = the regression is back.

### Windows note

None of the above touches the Windows/MSIX build: `mcporb-macos-access` and all
`macos_access` usage are `cfg(target_os = "macos")`-gated, and the crate's deps
are macOS-target-gated (verified with
`cargo check --target x86_64-pc-windows-msvc`). Keep it that way — never add a
non-macOS dependency to `mcporb-macos-access`.
