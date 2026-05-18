#!/usr/bin/env bash
# Binary size regression check for CI.
# Fails if mcporb-runtime release binary exceeds budget.
set -e

BUDGET_MB=20
BINARY="target/release/mcporb-runtime"

if [ ! -f "$BINARY" ]; then
  echo "ERROR: $BINARY not found. Run: cargo build --release -p mcporb-runtime"
  exit 1
fi

SIZE_BYTES=$(wc -c < "$BINARY")
SIZE_MB=$(echo "scale=2; $SIZE_BYTES / 1048576" | bc)
BUDGET_BYTES=$((BUDGET_MB * 1048576))

echo "Binary: $BINARY"
echo "Size:   ${SIZE_MB} MB (budget: ${BUDGET_MB} MB)"

if [ "$SIZE_BYTES" -gt "$BUDGET_BYTES" ]; then
  echo "FAIL: Binary size ${SIZE_MB} MB exceeds budget ${BUDGET_MB} MB"
  exit 1
else
  echo "PASS: Binary size within budget"
fi
