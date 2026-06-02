#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

FULL_OUT="target/release/mcporb-runtime-full"
LITE_OUT="target/release/mcporb-runtime-lite"
LITE_TARGET_DIR="target/builder-runtime-lite"

echo "Building full runtime..."
cargo build --release -p mcporb-runtime
cp target/release/mcporb-runtime "$FULL_OUT"

echo "Building lite runtime..."
CARGO_TARGET_DIR="$LITE_TARGET_DIR" cargo build --release -p mcporb-runtime --no-default-features
cp "$LITE_TARGET_DIR/release/mcporb-runtime" "$LITE_OUT"

chmod +x "$FULL_OUT" "$LITE_OUT"

echo "Staged Builder runtimes:"
ls -lh "$FULL_OUT" "$LITE_OUT"
echo
echo "Builder can now discover these binaries from MCPOrbBuilder without manual copying."