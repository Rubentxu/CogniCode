#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB_URL="postgres://cognicode:cognicode@localhost:5432/cognicode"
TMP_DIR="$ROOT_DIR/.tmp-explorer-real"

mkdir -p "$TMP_DIR"

ensure_pg() {
  systemctl --user start cognicode-postgres 2>/dev/null || true
  for _ in $(seq 1 30); do
    if env -u LD_LIBRARY_PATH podman exec cognicode-postgres pg_isready -U cognicode -d cognicode >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "❌ PostgreSQL is not ready. Run 'just dev-pg-status' for details." >&2
  exit 1
}

tracked_status() {
  git -C "$ROOT_DIR" status --short --untracked-files=no
}
