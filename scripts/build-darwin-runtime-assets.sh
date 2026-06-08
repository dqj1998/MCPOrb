#!/usr/bin/env bash
# build-darwin-runtime-assets.sh — build the headerpad'd macOS runtime binaries
# on a real Mac and publish them as release assets, WITHOUT consuming GitHub
# Actions minutes.
#
# WHY THIS EXISTS
#   The Builder release workflow builds the darwin runtime in CI, but GitHub
#   Actions free minutes are exhausted. This script is the no-Actions fallback:
#   build lite+full for aarch64-apple-darwin locally (the `.cargo/config.toml`
#   headerpad reservation applies because we build from the MCPOrb root), prove
#   the header slack the signing worker needs, then `gh release upload` the two
#   assets + an updated SHA256SUMS.txt to mcporb/mcporb-dist. `gh release upload`
#   hits the GitHub REST API — it does NOT run a workflow, so it costs zero
#   Actions minutes.
#
# SCOPE — DELIBERATELY NARROW (EDR-safe)
#   This script ONLY: compiles a binary, measures Mach-O header slack (read-only),
#   and uploads files. It performs NO segment injection, NO code signing, and
#   NO execution of any produced/downloaded artifact. Injection + Developer ID
#   signing + Apple notarization all happen later on the dedicated signing
#   worker (153.126.215.188), never here. (Avoids the download→chmod+x→
#   forge-quarantine→exec pattern that the managed Mac's EDR flags.)
#
# PROD MIRROR IS VERSION-GATED — READ THIS
#   prod's cli-autoupdate.sh re-mirrors runtimes only when the release version
#   CHANGES (its `.runtime-version` marker differs). Uploading these assets to
#   the CURRENT latest release (same version) will NOT auto-refresh prod's
#   STORE_RUNTIMES_ROOT/darwin-arm64. To push to prod you either (a) cut a new
#   version, or (b) force a re-sync on prod (clear the marker / re-run the
#   updater), or (c) scp the binaries onto prod directly.
#
# USAGE
#   scripts/build-darwin-runtime-assets.sh <version>      # e.g. 1.2.3  or builder-v1.2.3
#   REPO=owner/repo SKIP_UPLOAD=1 scripts/build-darwin-runtime-assets.sh 1.2.3
# ENV
#   REPO            release repo (default: mcporb/mcporb-dist)
#   SKIP_UPLOAD=1   build + slack-verify only; don't touch the release
#   ALLOW_FRESH_SUMS=1  permit creating a SHA256SUMS.txt from scratch when the
#                       release has none (default: refuse, to avoid wiping the
#                       integrity entries for the other OSes' assets)
set -euo pipefail

REPO="${REPO:-mcporb/mcporb-dist}"
RPLAT="darwin-arm64"
MIN_SLACK=1024  # headerpad,0x1000 ⇒ ~4096; a build that dropped it ⇒ ~56

die() { echo "✗ $*" >&2; exit 1; }
log() { echo "→ $*"; }

[[ $# -eq 1 ]] || die "usage: $(basename "$0") <version>  (e.g. 1.2.3 or builder-v1.2.3)"
RAW="$1"
VER="${RAW#builder-v}"; VER="${VER#v}"
TAG="builder-v${VER}"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

# ── preconditions ───────────────────────────────────────────────────────────
[[ "$(uname -s)" == "Darwin" ]] || die "must run on macOS (need ld64 for headerpad-correct Mach-O)"
[[ "$(uname -m)" == "arm64" ]] || die "must run on Apple Silicon (target aarch64-apple-darwin)"
command -v cargo   >/dev/null || die "cargo not found"
command -v python3 >/dev/null || die "python3 not found"
command -v shasum  >/dev/null || die "shasum not found"
[[ -f .cargo/config.toml ]] || die ".cargo/config.toml missing — headerpad reservation would be absent"
if [[ -z "${SKIP_UPLOAD:-}" ]]; then
  command -v gh >/dev/null || die "gh not found (set SKIP_UPLOAD=1 to build only)"
  gh auth status >/dev/null 2>&1 || die "gh not authenticated (needs write to $REPO)"
fi
rustup target add aarch64-apple-darwin >/dev/null 2>&1 || true

TARGET="aarch64-apple-darwin"
OUT="target/${TARGET}/release/mcporb-runtime"
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT
LITE_ASSET="mcporb-runtime-lite-${RPLAT}-${VER}"
FULL_ASSET="mcporb-runtime-full-${RPLAT}-${VER}"

# ── build (NO injection / NO signing / NO exec) ─────────────────────────────
log "building full runtime ($TARGET, headerpad via .cargo/config.toml)…"
cargo build --release --target "$TARGET" -p mcporb-runtime
cp "$OUT" "$STAGING/$FULL_ASSET"

log "building lite runtime (--no-default-features)…"
cargo build --release --target "$TARGET" -p mcporb-runtime --no-default-features
cp "$OUT" "$STAGING/$LITE_ASSET"

# ── prove the worker can inject (header slack ≥ what the injector needs) ─────
log "verifying Mach-O header slack (>= ${MIN_SLACK} B)…"
python3 scripts/macho-header-slack.py --min "$MIN_SLACK" \
  "$STAGING/$LITE_ASSET" "$STAGING/$FULL_ASSET" \
  || die "header slack insufficient — headerpad did not take effect; aborting before upload"
echo "✓ both runtimes carry sufficient header slack for segment injection"

if [[ -n "${SKIP_UPLOAD:-}" ]]; then
  log "SKIP_UPLOAD set — built + verified, not uploading. Artifacts in: $STAGING"
  cp "$STAGING/$LITE_ASSET" "$STAGING/$FULL_ASSET" ./ 2>/dev/null || true
  echo "  $LITE_ASSET"; echo "  $FULL_ASSET"
  trap - EXIT
  exit 0
fi

# ── update SHA256SUMS.txt (keep other OSes' entries intact) ─────────────────
gh release view "$TAG" --repo "$REPO" >/dev/null 2>&1 \
  || die "release $TAG not found on $REPO (create it first, or fix the version)"

cd "$STAGING"
if gh release download "$TAG" --repo "$REPO" --pattern SHA256SUMS.txt --dir . --clobber 2>/dev/null; then
  log "merging into existing SHA256SUMS.txt (preserving other assets' entries)…"
  pat=" [*]?(${LITE_ASSET//./\\.}|${FULL_ASSET//./\\.})\$"
  grep -vE "$pat" SHA256SUMS.txt > merged.sums || true
  shasum -a 256 "$LITE_ASSET" "$FULL_ASSET" >> merged.sums
  mv merged.sums SHA256SUMS.txt
else
  [[ -n "${ALLOW_FRESH_SUMS:-}" ]] \
    || die "release $TAG has no SHA256SUMS.txt — refusing to create one (would drop integrity entries for other OS assets). Set ALLOW_FRESH_SUMS=1 to override."
  log "no existing SHA256SUMS.txt — creating fresh (ALLOW_FRESH_SUMS set)…"
  shasum -a 256 "$LITE_ASSET" "$FULL_ASSET" > SHA256SUMS.txt
fi

log "uploading darwin runtime assets + SHA256SUMS.txt to $TAG on $REPO…"
gh release upload "$TAG" "$LITE_ASSET" "$FULL_ASSET" SHA256SUMS.txt --repo "$REPO" --clobber

echo "✓ published:"
echo "    $LITE_ASSET"
echo "    $FULL_ASSET"
echo "    SHA256SUMS.txt (updated)"
echo
echo "NOTE: prod mirror is version-gated. If $VER == prod's current runtime"
echo "      marker, run cli-autoupdate won't re-sync. See header comment."
