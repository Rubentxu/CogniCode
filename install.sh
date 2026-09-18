#!/bin/sh
# CogniCode installer — Layer 0 bootstrap.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Rubentxu/CogniCode/main/install.sh | sh
#
# Installs ONLY the `cogh` executable into ~/.cognicode/bin (or $COGNICODE_INSTALL_DIR).
# It does NOT install `cognicode`, `cognicode-mcp`, skills, IDE config, or the
# ~/.cognicode runtime — that is Layer 1, owned by `cogh install`.
#
# Security posture:
#   unsupported platform   -> fail loud
#   missing checksum       -> fail closed
#   checksum mismatch      -> fail closed
#   partial download       -> never reaches the destination
#   existing destination   -> replaced only AFTER the new binary verifies
#
# Version selection:
#   default: latest stable GitHub release
#   COGNICODE_VERSION=v0.96.0  -> pin an exact release
# Testability (not for normal use):
#   COGNICODE_RELEASE_BASE overrides the download base URL so a local
#   fake release can drive the checksum/tamper tests.
set -eu

REPO="Rubentxu/CogniCode"
DEST="${COGNICODE_INSTALL_DIR:-$HOME/.cognicode/bin}"

log() { printf '%s\n' "$*" >&2; }
fail() { log "install.sh: error: $*"; exit 1; }

# --- 1. Platform detection (T1) -------------------------------------------
os="$(uname -s)" || fail "cannot detect OS"
arch="$(uname -m)" || fail "cannot detect architecture"

case "$os" in
  Linux) os_name="linux" ;;
  *) fail "unsupported OS '$os' — Tier-1 channels support Linux only (x86_64, aarch64)" ;;
esac

case "$arch" in
  x86_64|amd64)  rust_target="x86_64-unknown-linux-gnu" ;;
  aarch64|arm64) rust_target="aarch64-unknown-linux-gnu" ;;
  *) fail "unsupported architecture '$arch' — supported: x86_64, aarch64" ;;
esac

log "install.sh: detected $os_name/$rust_target"

# --- 2. Release resolution (T3: pinned version is honoured exactly) --------
api="https://api.github.com/repos/$REPO"
tag="${COGNICODE_VERSION:-}"
if [ -z "$tag" ]; then
  # Latest stable release (gh /releases/latest excludes drafts and prereleases).
  tag="$(curl -fsSL "$api/releases/latest" | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  [ -n "$tag" ] || fail "cannot resolve latest release from $api/releases/latest"
fi
case "$tag" in
  v*) version="${tag#v}" ;;
  *)  version="$tag"; tag="v$tag" ;;
esac
log "install.sh: installing cogh $tag"

base="${COGNICODE_RELEASE_BASE:-https://github.com/$REPO/releases/download/$tag}"
asset="cogh-$version-$rust_target.tar.gz"
asset_url="$base/$asset"

# --- 3. Download to temp (never straight to the destination) ---------------
tmpdir="$(mktemp -d)" || fail "cannot create temp dir"
trap 'rm -rf "$tmpdir"' EXIT
archive="$tmpdir/$asset"

log "install.sh: downloading $asset_url"
curl -fsSL --retry 3 -o "$archive" "$asset_url" || fail "download failed: $asset_url"

# Non-empty archive: a partial download must never proceed.
[ -s "$archive" ] || fail "downloaded archive is empty (partial download?)"

# --- 4. Checksum verification (fail closed) --------------------------------
# Fetch SHA256SUMS from the same release; it is the public digest surface.
sums="$tmpdir/SHA256SUMS"
curl -fsSL --retry 3 -o "$sums" "$base/SHA256SUMS" \
  || fail "cannot download SHA256SUMS from $base — refusing to install without a checksum (fail closed)"

expected="$(grep " $asset\$" "$sums" | awk '{print $1}')"
[ -n "$expected" ] || fail "no checksum found for $asset in SHA256SUMS (fail closed)"

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$archive" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
else
  fail "no sha256sum/shasum available — cannot verify checksum (fail closed)"
fi

[ "$actual" = "$expected" ] || fail "checksum mismatch for $asset:
  expected: $expected
  actual:   $actual"

log "install.sh: checksum verified ($actual)"

# --- 5. Extract + validate the binary in temp ------------------------------
mkdir -p "$tmpdir/extract"
tar -xzf "$archive" -C "$tmpdir/extract" || fail "cannot extract archive"
binary="$tmpdir/extract/bin/cogh"
[ -f "$binary" ] || fail "archive does not contain bin/cogh"

# Validate the binary actually runs before touching the destination.
chmod +x "$binary"
validated_version="$("$binary" --version 2>/dev/null || true)"
case "$validated_version" in
  *" $version") log "install.sh: validated binary: $validated_version" ;;
  *) fail "downloaded binary failed validation (expected version $version, got: '$validated_version')" ;;
esac

# --- 6. Atomic-ish install (rename after full verification) ----------------
mkdir -p "$DEST"
target="$DEST/cogh"
old="$tmpdir/cogh.old"
if [ -e "$target" ]; then
  # Safe replacement: preserve the old binary until the new one is in place.
  cp "$target" "$old" 2>/dev/null || true
fi
mv "$binary" "$target.tmp"
if ! mv "$target.tmp" "$target" 2>/dev/null; then
  fail "cannot install into $DEST — set COGNICODE_INSTALL_DIR to a writable location"
fi

log "install.sh: installed cogh $version -> $target"

# --- 7. PATH instructions (never mutate shell rc silently) ------------------
case ":$PATH:" in
  *":$DEST:"*) ;;
  *)
    log ""
    log "install.sh: $DEST is not on your PATH. Add this to your shell profile:"
    log "  export PATH=\"$DEST:\$PATH\""
    ;;
esac

log "install.sh: done. Next: cogh install (Layer 1) configures the CogniCode runtime."
