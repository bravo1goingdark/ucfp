#!/usr/bin/env bash
# Start the ucfp Rust API server.
# Designed to be called as ExecStart for ucfp-server.service.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BINARY="$REPO_DIR/target/release/ucfp"

if [ ! -x "$BINARY" ]; then
  echo "ERROR: $BINARY not found. Build first: cargo build --release" >&2
  exit 1
fi

exec "$BINARY"
