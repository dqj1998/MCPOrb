#!/usr/bin/env bash
# build-mas.sh — Build MCPOrb Runner for Mac App Store submission
#
# Prerequisites:
#   1. Apple Distribution certificate installed in Keychain
#   2. 3rd Party Mac Developer Installer certificate installed in Keychain
#   3. Mac App Store provisioning profile installed
#   3. All metadata files in stores-release/macos/
#
# Usage:
#   scripts/build-mas.sh [version]
#   scripts/build-mas.sh 1.1.9
set -euo pipefail

VERSION="${1:-$(grep '^version' crates/mcporb-runtime-app/Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

die() { echo "✗ $*" >&2; exit 1; }
log() { echo "→ $*"; }

clear_quarantine_attrs() {
  local target="$1"
  [[ -e "$target" ]] || return 0

  # App Store / TestFlight packages must not contain com.apple.quarantine.
  xattr -r -d com.apple.quarantine "$target" 2>/dev/null || true
  # Also clear any other inherited xattrs that can leak in from downloaded files.
  xattr -rc "$target" 2>/dev/null || true
}

extract_profile_entitlement() {
  local profile_plist="$1"
  local key="$2"

  /usr/libexec/PlistBuddy -c "Print :Entitlements:${key}" "$profile_plist" 2>/dev/null || true
}

assert_sandbox_enabled() {
  local bin="$1"
  local ent
  ent="$(codesign -d --entitlements :- "$bin" 2>&1 || true)"

  if ! printf '%s' "$ent" | tr -d '\n' | grep -q '<key>com.apple.security.app-sandbox</key>[[:space:]]*<true/>'; then
    echo "$ent" >&2
    die "sandbox entitlement missing on $bin"
  fi
}

# Sandboxed processes abort in secinit (SIGTRAP in _libsecinit_appsandbox)
# before main() when application-identifier is missing — assert it explicitly.
assert_application_identifier() {
  local bin="$1"
  local ent
  ent="$(codesign -d --entitlements :- "$bin" 2>&1 || true)"

  if ! printf '%s' "$ent" | tr -d '\n' | grep -q '<key>com.apple.application-identifier</key>'; then
    echo "$ent" >&2
    die "application-identifier entitlement missing on $bin (sandboxed processes crash in secinit without it)"
  fi
}

# Nested executables must NOT carry application-identifier: TestFlight rejects
# any executable whose signature has an app id but no embedded provisioning
# profile (ITMS-90885). The app id's profile lives in the bundle as
# Contents/embedded.provisionprofile, which validates only the MAIN executable.
assert_no_application_identifier() {
  local bin="$1"
  local ent
  ent="$(codesign -d --entitlements :- "$bin" 2>&1 || true)"

  if printf '%s' "$ent" | tr -d '\n' | grep -q '<key>com.apple.application-identifier</key>'; then
    echo "$ent" >&2
    die "application-identifier present on nested executable $bin (TestFlight ITMS-90885)"
  fi
}

# team-identifier alone is sufficient for secinit to build the sandbox profile
# (verified empirically: sandbox + team-identifier, no app id → process passes
# secinit; the old crash only occurred when BOTH were absent).
assert_team_identifier() {
  local bin="$1"
  local ent
  ent="$(codesign -d --entitlements :- "$bin" 2>&1 || true)"

  if ! printf '%s' "$ent" | tr -d '\n' | grep -q '<key>com.apple.developer.team-identifier</key>'; then
    echo "$ent" >&2
    die "team-identifier entitlement missing on $bin (sandboxed processes crash in secinit without it)"
  fi
}

# ── preconditions ───────────────────────────────────────────────────────────
[[ "$(uname -s)" == "Darwin" ]] || die "must run on macOS"
command -v cargo >/dev/null || die "cargo not found"
command -v codesign >/dev/null || die "codesign not found (install Xcode)"

# Check for Apple Distribution certificate (for app code signing)
APP_IDENTITY=$(security find-identity -v -p basic 2>/dev/null | grep "Apple Distribution" | head -1 | sed 's/.*"\(.*\)"/\1/' || true)
if [[ -z "$APP_IDENTITY" ]]; then
  echo "⚠️  No Apple Distribution certificate found."
  echo "   Please create one at https://developer.apple.com/account/resources/certificates"
  echo "   Then re-run this script."
  exit 1
fi

# Check for Installer certificate (for pkg signing)
# Apple has used multiple labels over time; support both.
INSTALLER_IDENTITY=$(security find-identity -v -p basic 2>/dev/null | grep -E "3rd Party Mac Developer Installer|Mac Installer Distribution" | head -1 | sed 's/.*"\(.*\)"/\1/' || true)
if [[ -z "$INSTALLER_IDENTITY" ]]; then
  echo "⚠️  No Installer certificate found ('3rd Party Mac Developer Installer' or 'Mac Installer Distribution')."
  echo "   Please create one at https://developer.apple.com/account/resources/certificates"
  echo "   Then re-run this script."
  exit 1
fi

log "using app identity: $APP_IDENTITY"
log "using installer identity: $INSTALLER_IDENTITY"

# Check for provisioning profiles
PROFILES_DIR="$HOME/Library/MobileDevice/Provisioning Profiles"
if [[ ! -d "$PROFILES_DIR" ]] || [[ -z "$(ls "$PROFILES_DIR"/*.provisionprofile 2>/dev/null)" ]]; then
  echo "⚠️  No provisioning profiles found."
  echo "   Please create a Mac App Store profile at https://developer.apple.com/account/resources/profiles"
  exit 1
fi

# ── build ───────────────────────────────────────────────────────────────────
log "building MCPOrb Runner v${VERSION} for Mac App Store..."
log "  (using default 'mas' feature)"

(cd crates/mcporb-runtime-app && cargo tauri build --bundles app) 2>&1 | tail -20

# Find the built .app (prefer deterministic release outputs, never debug)
if [[ -d "target/universal-apple-darwin/release/bundle/macos/MCPOrb Runner.app" ]]; then
  APP_PATH="target/universal-apple-darwin/release/bundle/macos/MCPOrb Runner.app"
elif [[ -d "target/release/bundle/macos/MCPOrb Runner.app" ]]; then
  APP_PATH="target/release/bundle/macos/MCPOrb Runner.app"
else
  APP_PATH=$(find target -path "*/release/bundle/macos/MCPOrb Runner.app" -type d 2>/dev/null | head -1)
fi

if [[ -z "${APP_PATH:-}" ]]; then
  die "MCPOrb Runner.app not found in release bundle output"
fi
log "built app: $APP_PATH"

# ── embed provisioning profile ──────────────────────────────────────────────
# MAS requirement: the .app bundle must contain the provisioning profile.
# Find the matching MAS profile from the installed profiles.
MAS_PROFILE=$(find "$PROFILES_DIR" -name "*.provisionprofile" -exec sh -c '
  appid=$(security cms -D -i "$1" 2>/dev/null | grep -A1 "<key>Name</key>" | grep "<string>" | sed "s/.*<string>\(.*\)<\/string>.*/\1/" 2>/dev/null)
  if echo "$appid" | grep -qi "MCPOrb Runner"; then
    echo "$1"
  fi
' _ {} \; | head -1)

if [[ -z "$MAS_PROFILE" ]]; then
  die "No MCPOrb Runner provisioning profile found in $PROFILES_DIR"
fi

log "embedding provisioning profile: $MAS_PROFILE"
cp "$MAS_PROFILE" "$APP_PATH/Contents/embedded.provisionprofile"
clear_quarantine_attrs "$APP_PATH/Contents/embedded.provisionprofile"
clear_quarantine_attrs "$APP_PATH"

# ── sign ────────────────────────────────────────────────────────────────────
RUNTIME_BIN="$APP_PATH/Contents/MacOS/mcporb-runtime"
RUNNER_BIN="$APP_PATH/Contents/MacOS/mcporb-runner"
GATEWAY_BIN="$APP_PATH/Contents/MacOS/mcporb-gateway-stdio"
BASE_ENTITLEMENTS="crates/mcporb-runtime-app/entitlements-mas.plist"

PROFILE_PLIST=$(mktemp /tmp/mcporb-mas-profile.XXXXXX)
APP_SIGN_ENTITLEMENTS=$(mktemp /tmp/mcporb-mas-entitlements.XXXXXX)
NESTED_SIGN_ENTITLEMENTS=$(mktemp /tmp/mcporb-mas-nested-entitlements.XXXXXX)
trap 'rm -f "$PROFILE_PLIST" "$APP_SIGN_ENTITLEMENTS" "$NESTED_SIGN_ENTITLEMENTS"' EXIT

security cms -D -i "$MAS_PROFILE" > "$PROFILE_PLIST"

APP_IDENTIFIER="$(extract_profile_entitlement "$PROFILE_PLIST" "com.apple.application-identifier")"
if [[ -z "$APP_IDENTIFIER" ]]; then
  APP_IDENTIFIER="$(extract_profile_entitlement "$PROFILE_PLIST" "application-identifier")"
fi
TEAM_IDENTIFIER="$(extract_profile_entitlement "$PROFILE_PLIST" "com.apple.developer.team-identifier")"

[[ -n "$APP_IDENTIFIER" ]] || die "provisioning profile is missing Entitlements:com.apple.application-identifier"
[[ -n "$TEAM_IDENTIFIER" ]] || die "provisioning profile is missing Entitlements:com.apple.developer.team-identifier"

cp "$BASE_ENTITLEMENTS" "$APP_SIGN_ENTITLEMENTS"
/usr/libexec/PlistBuddy -c "Delete :com.apple.application-identifier" "$APP_SIGN_ENTITLEMENTS" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Delete :com.apple.developer.team-identifier" "$APP_SIGN_ENTITLEMENTS" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :com.apple.application-identifier string $APP_IDENTIFIER" "$APP_SIGN_ENTITLEMENTS"
/usr/libexec/PlistBuddy -c "Add :com.apple.developer.team-identifier string $TEAM_IDENTIFIER" "$APP_SIGN_ENTITLEMENTS"

# Nested executables (runtime + gateway) get sandbox + team-identifier ONLY.
# application-identifier is intentionally omitted: TestFlight rejects nested
# executables whose signature carries an app id without an embedded
# provisioning profile (ITMS-90885), and codesign on this machine cannot embed
# profiles. The runner/bundle keep the app id, validated via the bundle-level
# Contents/embedded.provisionprofile.
cp "$BASE_ENTITLEMENTS" "$NESTED_SIGN_ENTITLEMENTS"
/usr/libexec/PlistBuddy -c "Delete :com.apple.application-identifier" "$NESTED_SIGN_ENTITLEMENTS" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Delete :com.apple.developer.team-identifier" "$NESTED_SIGN_ENTITLEMENTS" 2>/dev/null || true
/usr/libexec/PlistBuddy -c "Add :com.apple.developer.team-identifier string $TEAM_IDENTIFIER" "$NESTED_SIGN_ENTITLEMENTS"

log "using provisioning app id: $APP_IDENTIFIER"
log "using provisioning team id: $TEAM_IDENTIFIER"

# MAS requirement: every executable in the app bundle must be sandboxed.
# The runner and the bundle itself carry application-identifier (matching the
# embedded.provisionprofile). Nested executables (runtime, gateway) are signed
# WITHOUT application-identifier — TestFlight rejects any nested executable
# whose signature carries an app id without an embedded provisioning profile
# (ITMS-90885), and codesign here cannot embed profiles. team-identifier alone
# prevents the secinit SIGTRAP crash (verified empirically).
log "signing mcporb-runtime (sandbox, no app id)..."
codesign --force --sign "$APP_IDENTITY" \
  --identifier "com.mcporb.runner.runtime" \
  --entitlements "$NESTED_SIGN_ENTITLEMENTS" \
  --options runtime \
  --timestamp \
  "$RUNTIME_BIN"
assert_sandbox_enabled "$RUNTIME_BIN"
assert_team_identifier "$RUNTIME_BIN"
assert_no_application_identifier "$RUNTIME_BIN"

# Sign mcporb-gateway-stdio (Tauri externalBin) with sandbox too — Transporter
# rejects the package if ANY bundled executable lacks the sandbox entitlement.
log "signing mcporb-gateway-stdio (sandbox, no app id)..."
codesign --force --sign "$APP_IDENTITY" \
  --identifier "com.mcporb.runner.gateway.stdio" \
  --entitlements "$NESTED_SIGN_ENTITLEMENTS" \
  --options runtime \
  --timestamp \
  "$GATEWAY_BIN"
assert_sandbox_enabled "$GATEWAY_BIN"
assert_team_identifier "$GATEWAY_BIN"
assert_no_application_identifier "$GATEWAY_BIN"

# Sign mcporb-runner WITH sandbox entitlements (the Tauri GUI app)
log "signing mcporb-runner (with sandbox)..."
codesign --force --sign "$APP_IDENTITY" \
  --entitlements "$APP_SIGN_ENTITLEMENTS" \
  --options runtime \
  --timestamp \
  "$RUNNER_BIN"
assert_sandbox_enabled "$RUNNER_BIN"
assert_application_identifier "$RUNNER_BIN"

# Sign the bundle with the same entitlements for the main executable.
log "signing app bundle..."
codesign --force --sign "$APP_IDENTITY" \
  --entitlements "$APP_SIGN_ENTITLEMENTS" \
  --timestamp \
  "$APP_PATH"
assert_sandbox_enabled "$APP_PATH/Contents/MacOS/mcporb-runner"

# Verify signature
codesign --verify --verbose "$APP_PATH" || die "signature verification failed"
log "signature verified ✓"

# Ensure no quarantine xattr remains before packaging.
clear_quarantine_attrs "$APP_PATH"

# ── create pkg ──────────────────────────────────────────────────────────────
PKG_NAME="MCPOrbRunner-${VERSION}-mas.pkg"
log "creating installer package: $PKG_NAME..."

productbuild \
  --component "$APP_PATH" /Applications \
  --sign "$INSTALLER_IDENTITY" \
  --timestamp \
  "$PKG_NAME"

# Clear quarantine xattr from final installer package as a final safety net.
clear_quarantine_attrs "$PKG_NAME"

pkgutil --check-signature "$PKG_NAME" >/dev/null || die "pkg signature verification failed"

log "package created: $PKG_NAME"

# ── summary ─────────────────────────────────────────────────────────────────
echo ""
echo "════════════════════════════════════════════════════════════════════"
echo "  ✅ Build complete!"
echo "════════════════════════════════════════════════════════════════════"
echo ""
echo "  App:     $APP_PATH"
echo "  Package: $PKG_NAME"
echo "  Version: $VERSION"
echo "  App Identity: $APP_IDENTITY"
echo "  Installer Identity: $INSTALLER_IDENTITY"
echo ""
echo "  Next steps:"
echo "    1. Open Transporter (from Mac App Store)"
echo "    2. Drag $PKG_NAME to Transporter"
echo "    3. Click 'Deliver'"
echo "    4. Go to App Store Connect to submit for review"
echo ""
echo "  MAS note: use Transporter/App Store Connect upload for this package."
echo ""
