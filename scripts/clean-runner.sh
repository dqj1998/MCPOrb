#!/usr/bin/env bash
# clean-runner.sh — Complete cleanup of MCPOrb Runner runtime environment
#
# Usage:
#   scripts/clean-runner.sh          # Interactive (asks for confirmation)
#   scripts/clean-runner.sh --force  # No confirmation
set -euo pipefail

FORCE=false
[[ "${1:-}" == "--force" ]] && FORCE=true

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

log() { echo "→ $*"; }
ok()  { echo "  ✓ $*"; }

echo "════════════════════════════════════════════════════════════════════"
echo "  MCPOrb Runner — Full Runtime Cleanup"
echo "════════════════════════════════════════════════════════════════════"
echo ""

# ── 1. Kill running processes ────────────────────────────────────────────────
log "Stopping MCPOrb processes..."
PIDS=$(pgrep -f "mcporb-(runner|gateway-http|gateway-stdio|runtime)" 2>/dev/null || true)
if [[ -n "$PIDS" ]]; then
  echo "$PIDS" | xargs kill -TERM 2>/dev/null || true
  sleep 2
  PIDS=$(pgrep -f "mcporb-(runner|gateway-http|gateway-stdio|runtime)" 2>/dev/null || true)
  if [[ -n "$PIDS" ]]; then
    echo "$PIDS" | xargs kill -9 2>/dev/null || true
    sleep 1
  fi
  ok "Processes terminated"
else
  ok "No running processes"
fi

# ── 2. Kill tmux sessions ────────────────────────────────────────────────────
log "Killing tmux sessions..."
for sess in gw gateway; do
  tmux kill-session -t "$sess" 2>/dev/null || true
done
ok "Tmux sessions cleaned"

# ── 3. Remove data directories ───────────────────────────────────────────────
log "Removing data directories..."

# Container (sandbox data — Orb registry, Orbs, metrics, models)
rm -rf ~/Library/Containers/com.mcporb.runner/Data/.mcporb
ok "~/Library/Containers/com.mcporb.runner/Data/.mcporb"

# App Support
rm -rf ~/Library/Application\ Support/MCPOrbRunner
ok "~/Library/Application Support/MCPOrbRunner"

# Caches
rm -rf ~/Library/Caches/MCPOrbRunner
ok "~/Library/Caches/MCPOrbRunner"

# Preferences
rm -f ~/Library/Preferences/com.mcporb.runner.plist
ok "~/Library/Preferences/com.mcporb.runner.plist"

# Saved Application State
rm -rf ~/Library/Saved\ Application\ State/com.mcporb.runner.savedState
ok "~/Library/Saved Application State"

# HTTP Storages
rm -rf ~/Library/HTTPStorages/com.mcporb.runner
ok "~/Library/HTTPStorages"

# WebKit
rm -rf ~/Library/WebKit/com.mcporb.runner
ok "~/Library/WebKit"

# ── 4. Remove Orb zip files (if any exist outside container) ─────────────────
log "Checking for Orb archives..."
ORB_COUNT=$(find ~/Library -name "*.orb.zip" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$ORB_COUNT" -gt 0 ]]; then
  find ~/Library -name "*.orb.zip" -delete 2>/dev/null || true
  ok "Removed $ORB_COUNT Orb archive(s)"
else
  ok "No Orb archives found"
fi

# ── 5. Verify ────────────────────────────────────────────────────────────────
echo ""
log "Verification..."

STILL_RUNNING=$(pgrep -f "mcporb-(runner|gateway|runtime)" 2>/dev/null | wc -l | tr -d ' ')
if [[ "$STILL_RUNNING" -eq 0 ]]; then
  ok "No MCPOrb processes running"
else
  echo "  ⚠️  $STILL_RUNNING process(es) still running"
fi

MCPORB_DIR=~/Library/Containers/com.mcporb.runner/Data/.mcporb
if [[ ! -d "$MCPORB_DIR" ]]; then
  ok "Orb data directory removed"
else
  echo "  ⚠️  $MCPORB_DIR still exists"
fi

echo ""
echo "════════════════════════════════════════════════════════════════════"
echo "  ✅ Cleanup complete!"
echo "════════════════════════════════════════════════════════════════════"
echo ""
echo "  Runner is in clean state. Next launch will start fresh."
echo ""
