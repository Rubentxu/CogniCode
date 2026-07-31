#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "$0")" && pwd)/common.sh"

PORT="${1:-8010}"
PID_FILE="$TMP_DIR/api.pid"

if [[ -f "$PID_FILE" ]]; then
  kill "$(cat "$PID_FILE")" 2>/dev/null || true
  rm -f "$PID_FILE"
fi

fuser -k "${PORT}/tcp" 2>/dev/null || true
echo "✅ Explorer real API stopped"
