#!/usr/bin/env bash
# smoke-sandbox-orb.sh — end-to-end smoke test for the sandboxed, out-of-container
# Orb read path (the exact path that broke in Runner 1.5.0–1.5.2).
#
# WHY THIS IS SEMI-AUTOMATED
#   The failing path only exists under: signed + sandboxed app, Orb library
#   OUTSIDE the container, launched the way Claude Desktop launches it
#   (mcporb-runner --gateway-stdio). Minting the security-scoped bookmark
#   requires the NSOpenPanel powerbox UI, which cannot be scripted — so this
#   test assumes you have already installed the signed app and picked an Orb
#   library folder ONCE via the GUI. It then drives the real gateway over stdio
#   and asserts an Orb actually spawns (no bookmark / spawn failure).
#
# USAGE
#   scripts/smoke-sandbox-orb.sh [app_path] [registry_dir]
#   Defaults:
#     app_path      /Applications/MCPOrb Runner.app
#     registry_dir  ~/Library/Containers/com.mcporb.runner/Data/.mcporb
#
# EXIT
#   0  an Orb spawned and answered (bookmark/exec/direct-read path all work)
#   1  the regression is present (bookmark could not be resolved / did not grant
#      access / failed to spawn Orb), or preconditions are missing.
set -euo pipefail

APP_PATH="${1:-/Applications/MCPOrb Runner.app}"
REGISTRY_DIR="${2:-$HOME/Library/Containers/com.mcporb.runner/Data/.mcporb}"
RUNNER_BIN="$APP_PATH/Contents/MacOS/mcporb-runner"
SETTINGS="$REGISTRY_DIR/settings.json"
REGISTRY="$REGISTRY_DIR/registry.json"

die() { echo "✗ $*" >&2; exit 1; }
log() { echo "→ $*"; }

[[ "$(uname -s)" == "Darwin" ]] || die "must run on macOS"
[[ -x "$RUNNER_BIN" ]] || die "runner not found: $RUNNER_BIN"
[[ -f "$SETTINGS" ]] || die "settings not found: $SETTINGS"
[[ -f "$REGISTRY" ]] || die "registry not found: $REGISTRY (install the app and add an Orb first)"

# Precondition: an out-of-container library with a persisted bookmark. If the
# library lives inside the container the bookmark path is not exercised, so this
# smoke would pass vacuously — warn loudly in that case.
python3 - "$SETTINGS" <<'PY' || exit 1
import json, os, sys
s = json.load(open(sys.argv[1]))
d = s.get("orb_library_dir")
bm = s.get("orb_library_bookmark")
if not d or not bm:
    print("✗ settings.json has no orb_library_dir/bookmark — pick an Orb library "
          "folder in the app (Settings → Choose…) first", file=sys.stderr)
    sys.exit(1)
container = os.path.expanduser("~/Library/Containers/com.mcporb.runner")
if os.path.realpath(d).startswith(os.path.realpath(container)):
    print(f"⚠️  library dir {d} is INSIDE the container — this smoke does not "
          f"exercise the security-scoped bookmark path", file=sys.stderr)
print(f"→ library dir: {d} (bookmark present, {len(bm)} b64 chars)")
PY

log "driving gateway: $RUNNER_BIN --gateway-stdio $REGISTRY_DIR"

# Drive the MCP stdio handshake and call the first tool. Success = the Orb
# spawned (the response is NOT a bookmark/exec/spawn failure). A tool-argument
# validation error still counts as success: it means the Orb process started.
python3 - "$RUNNER_BIN" "$REGISTRY_DIR" <<'PY'
import json, subprocess, sys, threading, time

runner, registry_dir = sys.argv[1], sys.argv[2]
proc = subprocess.Popen(
    [runner, "--gateway-stdio", registry_dir],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    bufsize=1, text=True,
)

FAIL_MARKERS = (
    "bookmark could not be resolved",
    "did not grant access",
    "failed to read Orb ZIP",
    "Failed to spawn Orb",
    "security-scoped bookmark",
)

def send(obj):
    proc.stdin.write(json.dumps(obj) + "\n")
    proc.stdin.flush()

def read_until(pred, timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        line = proc.stdout.readline()
        if not line:
            break
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        if pred(msg):
            return msg
    return None

def fail(reason):
    proc.kill()
    err = proc.stderr.read() if proc.stderr else ""
    for m in FAIL_MARKERS:
        if m in err:
            print(f"✗ REGRESSION: gateway/runtime reported: …{m}…", file=sys.stderr)
            print(err[-2000:], file=sys.stderr)
            sys.exit(1)
    print(f"✗ smoke failed: {reason}", file=sys.stderr)
    print(err[-2000:], file=sys.stderr)
    sys.exit(1)

send({"jsonrpc": "2.0", "id": 0, "method": "initialize",
      "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                 "clientInfo": {"name": "smoke", "version": "0"}}})
if not read_until(lambda m: m.get("id") == 0 and "result" in m):
    fail("no initialize result")
send({"jsonrpc": "2.0", "method": "notifications/initialized"})
send({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
tl = read_until(lambda m: m.get("id") == 1 and "result" in m)
if not tl:
    fail("no tools/list result")
tools = tl["result"].get("tools", [])
if not tools:
    fail("no tools advertised (no Orbs?)")
tool = tools[0]["name"]
print(f"→ calling tool: {tool}")
send({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
      "params": {"name": tool, "arguments": {}}})
resp = read_until(lambda m: m.get("id") == 2)
if not resp:
    fail("no tools/call response")

blob = json.dumps(resp)
if any(m in blob for m in FAIL_MARKERS):
    fail(f"tools/call returned a spawn/bookmark failure: {blob[:400]}")

proc.kill()
# Reaching here means the Orb spawned and answered (result OR a benign
# validation error) — the sandboxed out-of-container read path works.
print("✓ smoke passed: Orb spawned via the sandboxed out-of-container path")
PY
