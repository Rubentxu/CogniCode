#!/usr/bin/env bash
# Test: a published release tag must carry the full set of assets
# that the install / cogh-update path expects.
#
# Background: BLOQUE 5 / BLOQUE 8 audit (2026-10-15). v0.101.10
# published 8 assets; v0.101.11 through v0.101.16 published only
# 3. The missing 5 are exactly the ones the install paths depend on:
#
#   - bundle-<v>-<platform>.yaml          (per-platform manifest; cogh update reads this)
#   - cognicode-<v>.tar.gz                (portable skill bundle, no platform suffix)
#   - cognicode-mcp-<v>.tar.gz            (portable MCP skill bundle)
#   - release-inventory-<v>.json          (channel/platform inventory)
#   - SHA256SUMS                          (install.sh requires this to verify)
#
# The release-candidate pipeline had a documented post-mortem of
# the exact failure mode (release-candidate.pipeline.kts:631-640):
# the skill-bundles stage used to trust a pipe and exit 0 with
# zero bundles when the subcommand was missing. That code path was
# fixed, but no test pins the post-fix contract at the published-
# release level. This test is that pin.
#
# Usage:
#   bash sandbox/scripts/test_release_asset_set.sh                  # checks the latest published tag
#   bash sandbox/scripts/test_release_asset_set.sh v0.101.16         # checks a specific tag
#   bash sandbox/scripts/test_release_asset_set.sh v0.101.16 linux-x86_64   # tag + platform
#
# Requires: gh CLI, authenticated against Rubentxu/CogniCode.
#
# Refs: BLOQUE 5 stabilization, CP2-CP5 distribution gate, the
# post-mortem in release-candidate.pipeline.kts:631-640.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# ---------- args ----------
TAG="${1:-}"
PLATFORM="${2:-linux-x86_64}"   # default to the platform the v0.101.10 manifest uses

# If no tag was given, resolve to the latest published tag.
if [ -z "$TAG" ]; then
    if ! command -v gh >/dev/null 2>&1; then
        echo "FAIL: gh CLI not found on PATH; cannot resolve the latest tag"
        exit 1
    fi
    TAG=$(gh release view --json tagName -q '.tagName' 2>/dev/null) || {
        echo "FAIL: cannot resolve the latest release tag via gh"
        exit 1
    }
    echo "Resolved latest published tag: $TAG"
fi

# Strip leading 'v' to get the version, used in asset-name templates.
VERSION="${TAG#v}"
PLATFORM_TOKEN=""
case "$PLATFORM" in
    linux-x86_64)  PLATFORM_TOKEN="x86_64-unknown-linux-gnu" ;;
    linux-aarch64) PLATFORM_TOKEN="aarch64-unknown-linux-gnu" ;;
    macos-x86_64)  PLATFORM_TOKEN="x86_64-apple-darwin" ;;
    macos-aarch64) PLATFORM_TOKEN="aarch64-apple-darwin" ;;
    windows-x86_64) PLATFORM_TOKEN="x86_64-pc-windows-msvc" ;;
    *)
        echo "FAIL: unknown platform '$PLATFORM' (expected one of linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64, windows-x86_64)"
        exit 1
        ;;
esac

# ---------- expected asset set ----------
# The 8 assets v0.101.10 published. Every published release for the
# given platform must carry the full set. Adding a new asset means
# adding it here AND in the release-candidate lane.
EXPECTED=(
    "cogh-${VERSION}-${PLATFORM_TOKEN}.tar.gz"
    "cognicode-${VERSION}-${PLATFORM_TOKEN}.tar.gz"
    "cognicode-mcp-${VERSION}-${PLATFORM_TOKEN}.tar.gz"
    "cognicode-${VERSION}.tar.gz"
    "cognicode-mcp-${VERSION}.tar.gz"
    "bundle-${VERSION}-${PLATFORM_TOKEN}.yaml"
    "release-inventory-${VERSION}.json"
    "SHA256SUMS"
)

# ---------- fetch published assets ----------
if ! command -v gh >/dev/null 2>&1; then
    echo "FAIL: gh CLI not found on PATH"
    exit 1
fi

# Fetch and sort for stable diff output. The trailing newline is
# trimmed because `gh ... -q '.assets[].name'` already strips JSON
# framing but leaves a final newline.
published=$(gh release view "$TAG" --json assets -q '.assets[].name' 2>/dev/null | sort) || {
    echo "FAIL: cannot read release $TAG via gh (does the tag exist? are you authenticated against $REPO?)"
    exit 1
}

if [ -z "$published" ]; then
    echo "FAIL: $TAG published 0 assets"
    exit 1
fi

# ---------- compare ----------
expected_sorted=$(printf '%s\n' "${EXPECTED[@]}" | sort)

missing=()
while IFS= read -r name; do
    if ! printf '%s\n' "${EXPECTED[@]}" | grep -qxF "$name"; then
        missing+=("$name")
    fi
done <<< "$published"

absent=()
while IFS= read -r name; do
    if ! printf '%s\n' "$published" | grep -qxF "$name"; then
        absent+=("$name")
    fi
done <<< "$expected_sorted"

published_count=$(printf '%s\n' "$published" | wc -l)
expected_count=${#EXPECTED[@]}

# ---------- report ----------
if [ "${#missing[@]}" -gt 0 ] || [ "${#absent[@]}" -gt 0 ]; then
    echo "FAIL: release $TAG is missing assets."
    echo ""
    echo "  Published: $published_count asset(s)"
    echo "  Expected:  $expected_count asset(s) for $PLATFORM"
    echo ""
    if [ "${#missing[@]}" -gt 0 ]; then
        echo "  Unexpected assets (not in the contract):"
        printf '    - %s\n' "${missing[@]}"
    fi
    if [ "${#absent[@]}" -gt 0 ]; then
        echo "  Missing assets (expected by install / cogh update):"
        printf '    - %s\n' "${absent[@]}"
    fi
    echo ""
    echo "  See sandbox/scripts/test_release_asset_set.sh header for the audit context."
    exit 1
fi

echo "Test result: PASS (release $TAG carries all $expected_count expected assets for $PLATFORM)"
exit 0
