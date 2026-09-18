#!/usr/bin/env bash
# e88 entry gates: E88-G0a + E88-G0b against the PUBLIC v0.97.0 release.
# Zero checkout, zero target/debug, zero localhost, zero COGNICODE_* seams.
set -euo pipefail

TAG="v0.97.0"
VER="0.97.0"
REPO="Rubentxu/CogniCode"
WORK="${1:?usage: $0 <workdir>}"

rm -rf "$WORK"; mkdir -p "$WORK"; cd "$WORK"
echo "== [0] clean env, fresh download of public cogh =="
env -u COGNICODE_BUNDLE_MANIFEST -u COGNICODE_API_BASE_URL -u COGNICODE_ASSET_BASE_URL -u COGNICODE_RELEASE_BASE_URL true

curl -fsSLO "https://github.com/$REPO/releases/download/$TAG/cogh-$VER-x86_64-unknown-linux-gnu.tar.gz"
curl -fsSLO "https://github.com/$REPO/releases/download/$TAG/SHA256SUMS"
grep "cogh-$VER-x86_64-unknown-linux-gnu.tar.gz" SHA256SUMS | sha256sum -c -
tar xzf "cogh-$VER-x86_64-unknown-linux-gnu.tar.gz"
COGH="$WORK/bin/cogh"
chmod +x "$COGH"

echo "== [E88-G0a] public cogh --version =="
V="$($COGH --version)"
echo "version: $V"
[ "$V" = "cogh $VER" ] || { echo "FAIL G0a: got $V"; exit 1; }

echo "== [E88-G0a] product install resolves a PUBLIC release (not DEV fixture) =="
export HOME="$WORK/home"; mkdir -p "$HOME"
# Real bootstrap journey first: public install.sh creates ~/.cognicode/bin (Layer 0).
curl -fsSL "https://raw.githubusercontent.com/$REPO/$TAG/install.sh" -o install.sh
sh install.sh > /dev/null
[ -x "$HOME/.cognicode/bin/cogh" ] || { echo "FAIL bootstrap: install.sh"; exit 1; }
OUT="$($COGH install mcp-server --version $VER --profile reviewer 2>&1)"
echo "$OUT" | tail -3
echo "$OUT" | grep -q "resolved $VER" || { echo "FAIL G0a: no public resolution"; exit 1; }
echo "$OUT" | grep -qi "DEV-ONLY fixture" && { echo "FAIL G0a: DEV fixture fallback!"; exit 1; }
[ "$(cat "$HOME/.cognicode/tracker/version")" = "$VER" ] || { echo "FAIL G0a: tracker"; exit 1; }

echo "== [E88-G0b] doctor (public binary) reports healthy via canonical shim =="
DOCT="$($COGH doctor 2>&1)"; echo "$DOCT"
echo "$DOCT" | grep -q "cognicode-mcp shim present" || { echo "FAIL G0b: doctor MCP not shim-based"; exit 1; }
echo "$DOCT" | grep -q "overall: healthy" || { echo "FAIL G0b: not healthy"; exit 1; }

echo "== ALL e88 ENTRY GATES PASS =="
