#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$ROOT_DIR/.tmp-explorer-real"

mkdir -p "$TMP_DIR"

tracked_status() {
  git -C "$ROOT_DIR" status --short --untracked-files=no
}
