#!/usr/bin/env bash
# Bootstrap Tier-1 Rust fixture repos for the C8-R clean-clone preflight.
#
# Scope (intentionally minimal):
#   5 Rust crates required by the h44_* continuation_e2e.rs tests:
#   - serde, ripgrep, anyhow, tokio, clap
# Each pinned to a specific commit SHA (reproducible across runs).
# Verified after clone: actual HEAD == expected SHA, else FAIL.
#
# Out of scope:
#   - The full 28-repo sandbox/scripts/clone_repos.sh corpus (use sandbox
#     justfile for that: `just sandbox-setup`).
#   - JS/TS/Go/Java/Python/etc language matrices.
#
# Network: allowed only during this bootstrap stage. The rest of the
# preflight must remain hermetic/offline regarding these fixtures.
#
# Exit codes:
#   0  all 5 repos cloned/verified at expected SHAs.
#   1+ any mismatch, network failure, or repo inaccessibility.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="${1:-${SCRIPT_DIR}/../../sandbox/repos}"
mkdir -p "$REPO_DIR"

# Pin function: clone or update a repo at a specific commit SHA.
# Verifies after checkout that HEAD == expected SHA.
pin_tier1() {
    local name="$1"
    local url="$2"
    local sha="$3"
    local target="$REPO_DIR/$name"

    if [ -d "$target" ]; then
        local current
        current=$(cd "$target" && git rev-parse HEAD 2>/dev/null || echo "")
        if [ "$current" = "$sha" ]; then
            echo "[tier1] $name: already at expected SHA $sha"
            return 0
        fi
        echo "[tier1] $name: exists at $current, expected $sha - re-cloning"
        rm -rf "$target"
    fi

    echo "[tier1] $name: cloning (pinned to $sha)..."
    if ! git clone --no-checkout "$url" "$target"; then
        echo "[tier1] FATAL: $name clone failed (network or repo inaccessible)" >&2
        return 1
    fi

    (
        cd "$target"
        git fetch --depth=1 origin "$sha" 2>/dev/null
        git checkout "$sha" --force 2>/dev/null
    ) || {
        echo "[tier1] FATAL: $name fetch/checkout of pinned SHA failed" >&2
        return 1
    }

    local actual
    actual=$(cd "$target" && git rev-parse HEAD 2>/dev/null || echo "")
    if [ "$actual" != "$sha" ]; then
        echo "[tier1] FATAL: $name SHA mismatch: actual=$actual expected=$sha" >&2
        return 1
    fi
    echo "[tier1] $name: OK at SHA $sha"
    return 0
}

fail=0
pin_tier1 "serde"    "https://github.com/serde-rs/serde.git"      "03eec42c3313b36da416be1486e9ecac345784d5" || fail=$((fail + 1))
pin_tier1 "ripgrep"  "https://github.com/BurntSushi/ripgrep.git"  "4649aa9700619f94cf9c66876e9549d83420e16c" || fail=$((fail + 1))
pin_tier1 "anyhow"   "https://github.com/dtolnay/anyhow.git"      "8ea1819c4c7829d0eb09e54a52806f382b8d445b" || fail=$((fail + 1))
pin_tier1 "tokio"    "https://github.com/tokio-rs/tokio.git"      "dd344a550c2c6ddc500a2a8ad2eca8e097795252" || fail=$((fail + 1))
pin_tier1 "clap"     "https://github.com/clap-rs/clap.git"        "4684d7abc545cef1d78708864cfe8c7668ed49c1" || fail=$((fail + 1))

if [ "$fail" -gt 0 ]; then
    echo "[tier1] FATAL: $fail repo(s) failed bootstrap; preflight MUST abort" >&2
    exit 1
fi

echo "[tier1] All 5 Tier-1 Rust repos bootstrapped and SHA-verified"
exit 0
