#!/usr/bin/env bash
# e74 WU3 — release matrix coherence check.
#
# Validates that the platform targets declared in the release matrix
# (.github/workflows/release.yml) match the BundleManifest::Platform
# variants in Rust (crates/cognicode-cli/src/cmd/bundle_manifest.rs).
#
# Why this script: when a new target is added (or one is dropped with
# a waiver), the YAML matrix and the Rust enum must stay in sync. This
# is the cheapest way to catch drift before it becomes a wrong-platform
# artifact shipped to users.
#
# Invariants enforced:
#   1. Every target in the YAML matrix is a known BundleManifest::Platform.
#   2. Every BundleManifest::Platform appears at least once in the matrix
#      (no silent gap).
#   3. Each runner is from a vetted GitHub-hosted runner allowlist.
#
# Exit codes:
#   0 — matrix is in sync
#   1 — at least one mismatch (with a descriptive diff)
#
# Usage:  scripts/check-release-matrix.sh

set -euo pipefail

cd "$(dirname "$0")/.."

WORKFLOW=".github/workflows/release.yml"
BUNDLE_RS="crates/cognicode-cli/src/cmd/bundle_manifest.rs"

if [[ ! -f "$WORKFLOW" ]]; then
    echo "FAIL: $WORKFLOW not found"
    exit 1
fi

if [[ ! -f "$BUNDLE_RS" ]]; then
    echo "FAIL: $BUNDLE_RS not found"
    exit 1
fi

# Allowlist of runners. Adding a new runner requires updating this list
# AND documenting why it is allowed in the comment block above.
ALLOWED_RUNNERS=(
    "ubuntu-latest"
    "ubuntu-24.04-arm"
    "macos-13"
    "macos-latest"
    "macos-14"
    "windows-latest"
    "windows-11-arm"
)

# Extract platform targets from YAML: lines like `      - target: linux-x86-64`
# then immediately followed by `        runner: ubuntu-latest` (or similar).
YAML_TARGETS=$(grep -E '^\s*-\s*target:\s*[a-z0-9-]+\s*$' "$WORKFLOW" \
    | sed -E 's/.*target:\s*([a-z0-9-]+).*/\1/' \
    | sort -u)

# Extract runners from the same blocks.
YAML_RUNNERS=$(grep -E '^\s*runner:\s*[a-zA-Z0-9_-]+\s*$' "$WORKFLOW" \
    | sed -E 's/.*runner:\s*([a-zA-Z0-9_-]+).*/\1/' \
    | sort -u)

# Extract BundleManifest::Platform variants from Rust. The enum is
# defined with `LinuxX86_64,` etc.; we lower-case + kebab-case convert.
RUST_VARIANTS=$(grep -E '^\s*(Linux|MacOs|Windows)(X86_64|Aarch64),?\s*$' "$BUNDLE_RS" \
    | sed -E 's/.*\b(Linux|MacOs|Windows)(X86_64|Aarch64).*/\1\2/' \
    | sort -u)
# Convert PascalCase → kebab-case:
#   LinuxX86_64    → linux-x86-64
#   LinuxAarch64   → linux-aarch64
#   MacOsX86_64    → mac-os-x86-64
#   MacOsAarch64   → mac-os-aarch64
#   WindowsX86_64  → windows-x86-64
RUST_TARGETS=$(echo "$RUST_VARIANTS" \
    | sed -E 's/LinuxX86_64/linux-x86-64/; s/LinuxAarch64/linux-aarch64/; s/MacOsX86_64/mac-os-x86-64/; s/MacOsAarch64/mac-os-aarch64/; s/WindowsX86_64/windows-x86-64/' \
    | sort -u)

echo "=== Release matrix coherence check (e74 WU3) ==="
echo
echo "Targets in YAML matrix:"
echo "$YAML_TARGETS" | sed 's/^/  /'
echo
echo "Targets in BundleManifest::Platform:"
echo "$RUST_TARGETS" | sed 's/^/  /'
echo

ERRORS=0

# 1. Every YAML target must be a known Rust variant.
while IFS= read -r t; do
    [[ -z "$t" ]] && continue
    if ! grep -qx "$t" <<<"$RUST_TARGETS"; then
        echo "FAIL: YAML target '$t' is not a known BundleManifest::Platform."
        echo "      Add the variant to $BUNDLE_RS or remove the matrix lane."
        ERRORS=$((ERRORS + 1))
    fi
done <<<"$YAML_TARGETS"

# 2. Every Rust variant must appear in the matrix (no silent gaps).
while IFS= read -r t; do
    [[ -z "$t" ]] && continue
    if ! grep -qx "$t" <<<"$YAML_TARGETS"; then
        echo "FAIL: BundleManifest::Platform '$t' has no YAML matrix lane."
        echo "      Add it to $WORKFLOW or issue an explicit waiver in docs/adr/."
        ERRORS=$((ERRORS + 1))
    fi
done <<<"$RUST_TARGETS"

# 3. Every runner must be on the allowlist.
while IFS= read -r r; do
    [[ -z "$r" ]] && continue
    allowed=0
    for a in "${ALLOWED_RUNNERS[@]}"; do
        if [[ "$r" == "$a" ]]; then
            allowed=1
            break
        fi
    done
    if [[ "$allowed" == "0" ]]; then
        echo "FAIL: Runner '$r' is not in the vetted allowlist."
        echo "      Allowlist: ${ALLOWED_RUNNERS[*]}"
        ERRORS=$((ERRORS + 1))
    fi
done <<<"$YAML_RUNNERS"

echo
if [[ "$ERRORS" -eq 0 ]]; then
    echo "OK: release matrix and BundleManifest::Platform are in sync."
    exit 0
else
    echo "FAIL: $ERRORS mismatch(es) found."
    exit 1
fi
