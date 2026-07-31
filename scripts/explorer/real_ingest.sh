#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "$0")" && pwd)/common.sh"

WORKSPACE="${1:-$ROOT_DIR}"
PORT="${2:-8010}"
if [[ -z "$WORKSPACE" ]]; then
  WORKSPACE="$ROOT_DIR"
fi

echo "🧪 Real ingest target: $WORKSPACE"
BEFORE="$(tracked_status)"

ensure_pg
bash "$(cd "$(dirname "$0")" && pwd)/real_api_up.sh" "$PORT" >/dev/null

curl -fsS -X POST "http://127.0.0.1:${PORT}/api/workspaces/open" \
  -H "Content-Type: application/json" \
  -d "{\"root_path\":\"$WORKSPACE\"}" \
  > "$TMP_DIR/workspace.json"

WORKSPACE_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$TMP_DIR/workspace.json")"
echo "Workspace: $WORKSPACE_ID"

curl -fsS -X POST "http://127.0.0.1:${PORT}/api/workspaces/${WORKSPACE_ID}/scan" \
  > "$TMP_DIR/scan-start.json"

JOB_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["job_id"])' "$TMP_DIR/scan-start.json")"
echo "Scan job: $JOB_ID"

for _ in $(seq 1 240); do
  curl -fsS "http://127.0.0.1:${PORT}/api/jobs/${JOB_ID}" > "$TMP_DIR/scan-status.json"
  STATUS="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["status"])' "$TMP_DIR/scan-status.json")"
  if [[ "$STATUS" == "Completed" || "$STATUS" == "completed" ]]; then
    break
  fi
  sleep 1
done

STATUS="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["status"])' "$TMP_DIR/scan-status.json")"
if [[ "$STATUS" != "Completed" && "$STATUS" != "completed" ]]; then
  echo "❌ Scan job did not complete successfully. Last status: $STATUS" >&2
  cat "$TMP_DIR/scan-status.json" >&2
  exit 1
fi

AFTER="$(tracked_status)"
if [[ "$BEFORE" != "$AFTER" ]]; then
  echo "❌ Tracked git state changed during ingest. Review 'git status --short'." >&2
  exit 1
fi

echo "✅ Ingest complete. No tracked files changed."
