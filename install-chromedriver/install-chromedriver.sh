#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$SCRIPT_DIR/target/release/install-chromedriver"

if [[ ! -x "$BINARY" ]]; then
  echo "Binary not found at $BINARY. Run 'cargo build --release' first." >&2
  exit 1
fi

exec "$BINARY" "$@"
