#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "$0")" && pwd)/common.sh"

PORT="${1:-8010}"
PID_FILE="$TMP_DIR/api.pid"
LOG_FILE="$TMP_DIR/api.log"

if curl -fsS "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1; then
  echo "✅ Explorer API already running on :${PORT}"
  exit 0
fi

ensure_pg
cargo build -p cognicode-runtime --bin explorer-api >/dev/null

DATABASE_URL="$DB_URL" nohup "$ROOT_DIR/target/debug/explorer-api" \
  --cwd "$ROOT_DIR" \
  --listen "127.0.0.1:${PORT}" \
  > "$LOG_FILE" 2>&1 &

echo $! > "$PID_FILE"

for _ in $(seq 1 30); do
  if curl -fsS "http://127.0.0.1:${PORT}/health" >/dev/null 2>&1; then
    echo "✅ Explorer API ready on http://127.0.0.1:${PORT}"
    exit 0
  fi
  sleep 1
done

echo "❌ Explorer API did not become ready. See $LOG_FILE" >&2
exit 1
