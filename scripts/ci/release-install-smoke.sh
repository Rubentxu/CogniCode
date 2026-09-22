#!/usr/bin/env bash
# Release-candidate UAT: consume ONLY the produced distribution archives.
# No executable, skill or MCP command is allowed to refer to target/release.
set -euo pipefail

VERSION="${1:?usage: release-install-smoke.sh VERSION RELEASE_DIR}"
RELEASE_DIR="${2:?usage: release-install-smoke.sh VERSION RELEASE_DIR}"
RELEASE_DIR="$(realpath "$RELEASE_DIR")"
TRIPLE="x86_64-unknown-linux-gnu"
BUNDLE="bundle-${VERSION}-${TRIPLE}.yaml"
COGH="cogh-${VERSION}-${TRIPLE}.tar.gz"
test -f "$RELEASE_DIR/$BUNDLE"
test -f "$RELEASE_DIR/$COGH"
test -f "$RELEASE_DIR/SHA256SUMS"

TMP="$(mktemp -d)"
SERVER_PID=""
cleanup() {
  if [ -n "$SERVER_PID" ]; then kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$TMP"
}
trap cleanup EXIT

mkdir -p "$TMP/release/v$VERSION" "$TMP/staging" "$TMP/bootstrap" "$TMP/home/.config/opencode"
cp "$RELEASE_DIR/"* "$TMP/release/v$VERSION/"
tar -xzf "$RELEASE_DIR/$COGH" -C "$TMP/bootstrap"
COGH_BIN="$TMP/bootstrap/bin/cogh"
test -x "$COGH_BIN"
printf '{}\n' > "$TMP/home/.config/opencode/opencode.json"

python3 -u - "$TMP/release" > "$TMP/server.port" 2> "$TMP/server.err" <<'PY' &
import functools
import http.server
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(root))
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
print(server.server_address[1], flush=True)
server.serve_forever()
PY
SERVER_PID=$!
for attempt in $(seq 1 100); do
  if [ -s "$TMP/server.port" ]; then break; fi
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    cat "$TMP/server.err" >&2
    echo "release smoke: local asset server stopped" >&2
    exit 1
  fi
  sleep 0.1
done
test -s "$TMP/server.port"
PORT="$(head -n 1 "$TMP/server.port")"
BASE="http://127.0.0.1:$PORT"
cat > "$TMP/staging/releases.json" <<EOF
{
  "tag_name": "v$VERSION",
  "draft": false,
  "prerelease": false,
  "published_at": "2026-01-01T00:00:00Z",
  "html_url": "$BASE/v$VERSION",
  "assets": [
    {
      "name": "$BUNDLE",
      "browser_download_url": "$BASE/v$VERSION/$BUNDLE"
    }
  ]
}
EOF

export HOME="$TMP/home"
export COGNICODE_HOME="$HOME/.cognicode"
export XDG_CONFIG_HOME="$HOME/.config"
export XDG_DATA_HOME="$HOME/.local/share"
export OPENCODE_CONFIG="$HOME/.config/opencode/opencode.json"
export COGNICODE_ASSET_BASE_URL="$BASE"
unset COGNICODE_BUNDLE_MANIFEST COGNICODE_API_BASE_URL COGNICODE_RELEASE_BASE_URL

"$COGH_BIN" init
"$COGH_BIN" install mcp-server --version "$VERSION" --profile reviewer \
  --ide opencode --staging "$TMP/staging"

CLI="$COGNICODE_HOME/shims/cognicode"
MCP="$COGNICODE_HOME/shims/cognicode-mcp"
test -x "$CLI" && test -x "$MCP"
"$CLI" --version | grep -F "$VERSION"
"$MCP" --version | grep -F "$VERSION"
"$COGH_BIN" doctor | tee "$TMP/doctor.out"
grep -Eq 'PASS[[:space:]]+MCP' "$TMP/doctor.out"
test -f "$COGNICODE_HOME/versions/$VERSION/skills/cognicode/SKILL.md"
test -f "$COGNICODE_HOME/versions/$VERSION/skills/cognicode-mcp/SKILL.md"
test -L "$HOME/.config/opencode/skills/cognicode-$VERSION"
test -L "$HOME/.config/opencode/skills/cognicode-mcp-$VERSION"
jq -e '.mcp["cognicode-mcp"].command[0]' "$OPENCODE_CONFIG" > "$TMP/mcp-command"
! grep -q 'target/release\|/tmp/prf-' "$TMP/mcp-command"

# A same-version update must not silently replace reviewer with core.
"$COGH_BIN" update --staging "$TMP/staging"
"$COGH_BIN" doctor | grep -Eq 'PASS[[:space:]]+MCP'

# Recover a broken temporary link only from the active installed version.
rm "$MCP"
ln -s "/tmp/prf-deleted-version/bin/cognicode-mcp" "$MCP"
"$COGH_BIN" reshim
test -x "$MCP"
test "$(readlink -f "$MCP")" = \
  "$COGNICODE_HOME/versions/$VERSION/cognicode-mcp/bin/cognicode-mcp"

"$COGH_BIN" uninstall mcp-server --version "$VERSION" --ide opencode
test ! -e "$COGNICODE_HOME/versions/$VERSION"
test ! -e "$CLI"
test ! -e "$MCP"
test ! -e "$HOME/.config/opencode/skills/cognicode-$VERSION"
test ! -e "$HOME/.config/opencode/skills/cognicode-mcp-$VERSION"
jq -e '.mcp["cognicode-mcp"] == null' "$OPENCODE_CONFIG" >/dev/null
echo "PASS: published-layout CLI + MCP + skills install/update/reshim/uninstall"
