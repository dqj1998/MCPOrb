#!/bin/bash
# verify-1.2.26.sh — Two-launch verification of the stale-bookmark rebuild fix.
#
# Launch 1 (headless --mcp-stdio): expects "bookmark is stale" + rebuild +
# "persisting refreshed Orb library bookmark" + settings.json bookmark CHANGED.
# Launch 2 (headless --mcp-stdio): expects NO stale marker — the persisted
# rebuilt bookmark resolves healthy (success = fix works end-to-end).
#
# Usage: bash scripts/verify-1.2.26.sh
set -u

APP="/Applications/MCPOrb Runner.app"
BIN="$APP/Contents/MacOS/mcporb-runner"
SETTINGS="$HOME/Library/Containers/com.mcporb.runner/Data/Library/Application Support/MCPOrb/Runtime/settings.json"
LAUNCH_SECS=6

pass() { printf "  \033[32mPASS\033[0m %s\n" "$1"; }
fail() { printf "  \033[31mFAIL\033[0m %s\n" "$1"; }

echo "=== [0] Preconditions ==="
VER=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APP/Contents/Info.plist" 2>/dev/null || echo MISSING)
SIGNER=$(codesign -dvvv "$BIN" 2>&1 | sed -n 's/^Authority=//p' | head -1)
echo "  installed: v$VER  signer: ${SIGNER:-none}"
if [ "$VER" != "1.2.26" ] || [ "$SIGNER" != "TestFlight Beta Distribution" ]; then
    printf "  \033[33mSKIP\033[0m 1.2.26 TestFlight build not installed yet. Upload pkg via Transporter -> install via TestFlight,\nthen re-run. (Current state is the 1.2.25 baseline.)\n"
    exit 2
fi
[ -f "$SETTINGS" ] || { echo "  settings.json not found: $SETTINGS"; exit 2; }

bm_md5() { /usr/bin/python3 -c '
import json,sys,hashlib
s=json.load(open(sys.argv[1]))
b=s.get("orb_library_bookmark","")
print(hashlib.md5(b.encode()).hexdigest(), len(b), s.get("orb_library_dir",""))' "$1" 2>/dev/null || echo "ERR"; }

echo "=== [1] Baseline (pre-update bookmark, known STALE) ==="
stat -f "  mtime: %Sm  size: %z" "$SETTINGS"
echo "  bookmark: $(bm_md5 "$SETTINGS")"

launch_once() {
    local tag="$1"
    local logf="/tmp/bmtest/launch-${tag}.log"
    "$BIN" --mcp-stdio </dev/null >"$logf" 2>&1 &
    local pid=$!
    sleep "$LAUNCH_SECS"
    kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
    echo "$logf"
}

echo "=== [2] Launch 1 — expect stale -> rebuild -> persist ==="
LOG1=$(launch_once 1)
grep -q "resolved security-scoped bookmark is stale" "$LOG1" && pass "stale detected" || fail "no stale marker (unexpected: bookmark already healthy?)"
grep -q "persisting refreshed Orb library bookmark" "$LOG1" && pass "rebuilt bookmark persisted" || fail "no 'persisting refreshed' log"
grep -q "failed to refresh stale" "$LOG1" && fail "rebuild FAILED ($(grep -m1 'failed to refresh stale' "$LOG1"))" || pass "rebuild succeeded"
grep -q "startAccessingSecurityScopedResource failed" "$LOG1" && pass "access denied this session (expected: no GUI consent in headless)" || true
stat -f "  settings after: mtime %Sm  size %z" "$SETTINGS"
echo "  bookmark after: $(bm_md5 "$SETTINGS")"

echo "=== [3] Launch 2 — expect fresh bookmark resolves healthy ==="
LOG2=$(launch_once 2)
if grep -q "resolved security-scoped bookmark is stale" "$LOG2"; then
    fail "bookmark STILL STALE on launch 2 — macOS 26.6 sandbox refuses silent renewal; fallback = auto folder-picker (vNext)"
else
    pass "no stale marker on launch 2"
    grep -q "startAccessingSecurityScopedResource failed" "$LOG2" \
        && fail "resolved but access still denied — same verdict as above" \
        || echo "  (no access-failure marker; headless session, GUI launch will confirm banner)"
fi

echo "=== [4] Verdict ==="
if grep -q "persisting refreshed Orb library bookmark" "$LOG1" && ! grep -q "resolved security-scoped bookmark is stale" "$LOG2"; then
    pass "FIX CONFIRMED: rebuilt bookmark persisted on launch 1, healthy on launch 2."
    echo "  Final GUI check: open the app; the 'access has expired' banner should NOT appear."
else
    echo "  \033[33mPARTIAL/FAIL\033[0m — see markers above. Next step if stale persists:"
    echo "  implement auto folder-picker on stale (vNext), or user re-selects once in Settings."
fi
echo "logs: $LOG1 $LOG2"