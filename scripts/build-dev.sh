#!/usr/bin/env bash
# build-dev.sh — one-command dev build that keeps the runtime and gateway
# binaries in sync, and verifies the arg contract the gateway relies on.
#
# WHY THIS EXISTS
#   The gateway (mcporb-gateway-stdio / -http) spawns mcporb-runtime as a
#   child and passes a fixed argument contract:
#       --orb-zip <path> --stdio-only --orb-id <id>
#       --metrics-dir <dir> --mcp-transport <label>
#   These args were added incrementally (Jul 5–13). When the runtime binary
#   is stale relative to the gateway source (built on different days, or one
#   `cargo build -p` only), the child exits with
#   "error: unexpected argument '--orb-zip' found", the gateway sees stdout
#   close during init, and every MCP client fails with
#   "Orb child closed stdout during init".
#
#   This script rebuilds the whole dev toolchain from the SAME source tree in
#   one invocation and then checks that the freshly built runtime actually
#   accepts the args the gateway will pass — so the two can never drift again.
#
# USAGE
#   scripts/build-dev.sh                 # debug profile (default)
#   scripts/build-dev.sh --release       # release profile
#
# ENV
#   BUILD_PROFILE   debug (default) or release
set -euo pipefail

die() { echo "✗ $*" >&2; exit 1; }
log() { echo "→ $*"; }

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

PROFILE="${BUILD_PROFILE:-debug}"
case "$PROFILE" in
    debug|release) ;;
    *) die "BUILD_PROFILE must be 'debug' or 'release', got: $PROFILE" ;;
esac

# Allow `--release` as a positional shorthand too.
if [[ "${1:-}" == "--release" ]]; then
    PROFILE="release"
fi

TARGET_DIR="target/$PROFILE"
RUNTIME="$TARGET_DIR/mcporb-runtime"

# Cargo profile flags: `debug` maps to the default dev profile (target/debug);
# `release` maps to --release (target/release).
CARGO_FLAGS=()
if [[ "$PROFILE" == "release" ]]; then
    CARGO_FLAGS=(--release)
fi

# Arguments the gateway passes when spawning the runtime child. If any of
# these is missing from `mcporb-runtime --help`, the spawn would fail at
# runtime with "unexpected argument" — fail the build instead.
REQUIRED_ARGS=(--orb-zip --stdio-only --orb-id --metrics-dir --mcp-transport)

[[ "$(uname -s)" == "Darwin" ]] || log "note: not macOS; runtime build flags may differ"

# ── 1. runtime first (the contract holder) ──────────────────────────────────
log "Building mcporb-runtime ($PROFILE)..."
cargo build "${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"}" -p mcporb-runtime

# ── 2. gateways (stdio + http) — same source tree, same contract ────────────
log "Building mcporb-gateway-stdio ($PROFILE)..."
cargo build "${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"}" -p mcporb-gateway-stdio

log "Building mcporb-gateway-http ($PROFILE)..."
cargo build "${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"}" -p mcporb-gateway-http

# ── 3. contract self-check against the freshly built runtime ────────────────
[[ -x "$RUNTIME" ]] || die "runtime binary not found at $RUNTIME (build failed?)"

log "Verifying runtime arg contract..."
HELP="$("$RUNTIME" --help 2>&1)"
for arg in "${REQUIRED_ARGS[@]}"; do
    if ! grep -q -- "$arg" <<<"$HELP"; then
        die "runtime $RUNTIME is missing required arg '$arg' — it will be rejected by the gateway. Rebuild runtime from current source."
    fi
done

log "All required args present in runtime: ${REQUIRED_ARGS[*]}"
log "Done. Dev toolchain ($PROFILE) is in sync:"
ls -lh "$TARGET_DIR"/mcporb-runtime "$TARGET_DIR"/mcporb-gateway-stdio "$TARGET_DIR"/mcporb-gateway-http
