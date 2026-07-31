#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/common.sh"

WORKSPACE="${1:-$ROOT_DIR}"
PORT="${2:-8010}"

if [[ -z "$WORKSPACE" ]]; then
  WORKSPACE="$ROOT_DIR"
fi

"$SCRIPT_DIR/real_api_up.sh" "$PORT"
"$SCRIPT_DIR/real_ingest.sh" "$WORKSPACE" "$PORT"
"$SCRIPT_DIR/real_api_stop.sh" "$PORT" >/dev/null
"$SCRIPT_DIR/real_api_up.sh" "$PORT" >/dev/null

curl -fsS "http://127.0.0.1:${PORT}/health" | tee "$TMP_DIR/health.json"
echo

curl -fsS -X POST "http://127.0.0.1:${PORT}/api/workspaces/open" \
  -H "Content-Type: application/json" \
  -d "{\"root_path\":\"$WORKSPACE\"}" \
  > "$TMP_DIR/workspace.json"

WORKSPACE_ID="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$TMP_DIR/workspace.json")"
echo "Workspace: $WORKSPACE_ID"

if ! curl -fsS "http://127.0.0.1:${PORT}/api/workspaces/${WORKSPACE_ID}/landing" > "$TMP_DIR/landing.json"; then
  echo "❌ Landing request failed. Last API log lines:" >&2
  tail -n 40 "$TMP_DIR/api.log" >&2 || true
  exit 1
fi

python3 -c 'import json,sys; data=json.load(open(sys.argv[1])); ws=data.get("workspace",{}); print("Landing graph_status:", ws.get("graph_status")); print("Landing symbol_count:", ws.get("symbol_count")); print("Entry points:", len(data.get("entry_points",[]))); print("Hot paths:", len(data.get("hot_paths",[])))' "$TMP_DIR/landing.json"

echo "✅ Real Explorer smoke data saved under $TMP_DIR/"
