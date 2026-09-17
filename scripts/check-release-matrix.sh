#!/usr/bin/env bash
# e85 — release contract coherence check.
#
# Enforces the release invariants in CI:
#
#   R9  the workflow's declared `rust_target` for a platform must be exactly the
#       token the Rust contract derives for that platform. The contract
#       (`cognicode-release platform-token`) is the authority; the workflow must
#       not carry a second, drifting copy of the mapping.
#
#   Tier-1 completeness: every platform the contract publishes must have a lane,
#       and every lane must be a platform the contract knows.
#
#   Runner allowlist: a lane must use a vetted native GitHub-hosted runner.
#
# This replaces the e74 version, which required *every* `Platform` variant to
# have a matrix lane. e84 WU9 deliberately changed that: Tier 1 is Linux GNU on
# native runners (x86_64 + aarch64), while MUSL, macOS and Windows keep their
# Platform variants and adapter seams without being advertised as supported.
# The old invariant would have forced us to claim support we do not have.
#
# Exit codes: 0 in sync, 1 mismatch.

set -euo pipefail

cd "$(dirname "$0")/.."

WORKFLOW=".github/workflows/release.yml"
# Resolve the real target directory: it can be moved by CARGO_TARGET_DIR or by a
# repository `.cargo/config.toml`, so never assume `target/`.
TARGET_DIR=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])' 2>/dev/null \
    || echo "${CARGO_TARGET_DIR:-target}")
RELEASE_BIN="$TARGET_DIR/release/cognicode-release"

if [[ ! -f "$WORKFLOW" ]]; then
    echo "FAIL: $WORKFLOW not found"
    exit 1
fi

if [[ ! -x "$RELEASE_BIN" ]]; then
    echo "building $RELEASE_BIN ..."
    cargo build --release --bin cognicode-release >/dev/null
fi
if [[ ! -x "$RELEASE_BIN" ]]; then
    echo "FAIL: could not build $RELEASE_BIN"
    exit 1
fi

ALLOWED_RUNNERS=(
    "ubuntu-latest"
    "ubuntu-24.04-arm"
    "macos-13"
    "macos-latest"
    "macos-14"
    "windows-latest"
    "windows-11-arm"
)

# --- what the contract publishes ------------------------------------------------
mapfile -t TIER1_TOKENS < <("$RELEASE_BIN" platforms | sort -u)

# --- what the workflow declares -------------------------------------------------
mapfile -t LANE_PLATFORMS < <(grep -E '^[[:space:]]*(-[[:space:]]+)?platform:[[:space:]]*[a-z0-9-]+[[:space:]]*$' "$WORKFLOW" \
    | sed -E 's/.*platform:[[:space:]]*([a-z0-9-]+).*/\1/' | sort -u)
mapfile -t LANE_TOKENS < <(grep -E '^[[:space:]]*(-[[:space:]]+)?rust_target:[[:space:]]*[a-z0-9_-]+[[:space:]]*$' "$WORKFLOW" \
    | sed -E 's/.*rust_target:[[:space:]]*([a-z0-9_-]+).*/\1/' | sort -u)
mapfile -t LANE_RUNNERS < <(grep -E '^[[:space:]]*(-[[:space:]]+)?runner:[[:space:]]*[a-zA-Z0-9_.-]+[[:space:]]*$' "$WORKFLOW" \
    | sed -E 's/.*runner:[[:space:]]*([a-zA-Z0-9_.-]+).*/\1/' | sort -u)

contains() { local needle=$1; shift; local x; for x in "$@"; do [[ "$x" == "$needle" ]] && return 0; done; return 1; }

echo "=== e85 release contract coherence ==="
echo
echo "Tier-1 tokens published by the contract:"
printf '  %s\n' "${TIER1_TOKENS[@]}"
echo
echo "Lanes declared in $WORKFLOW:"
printf '  %s\n' "${LANE_PLATFORMS[@]}"
echo

ERRORS=0

if [[ "${#LANE_PLATFORMS[@]}" -ne "${#LANE_TOKENS[@]}" ]]; then
    echo "FAIL: ${#LANE_PLATFORMS[@]} lane(s) but ${#LANE_TOKENS[@]} rust_target(s)."
    ERRORS=$((ERRORS + 1))
fi

# --- R9: every lane platform must resolve, and its derived token must be used ----
for platform in "${LANE_PLATFORMS[@]}"; do
    derived=$("$RELEASE_BIN" platform-token --platform "$platform" 2>/dev/null || true)
    if [[ -z "$derived" ]]; then
        echo "FAIL: lane platform '$platform' is not a platform the contract knows."
        ERRORS=$((ERRORS + 1))
        continue
    fi
    if contains "$derived" "${LANE_TOKENS[@]}"; then
        echo "  ok  $platform -> $derived"
    else
        echo "FAIL: lane '$platform' should declare rust_target '$derived' (R9)."
        ERRORS=$((ERRORS + 1))
    fi
done

# --- no lane outside the published tier -----------------------------------------
for token in "${LANE_TOKENS[@]}"; do
    if ! contains "$token" "${TIER1_TOKENS[@]}"; then
        echo "FAIL: lane '$token' is not published by the contract (R9)."
        ERRORS=$((ERRORS + 1))
    fi
done

# --- no silent gap: everything we promise must actually be built -----------------
for token in "${TIER1_TOKENS[@]}"; do
    if contains "$token" "${LANE_TOKENS[@]}"; then
        echo "  ok  published '$token' has a lane"
    else
        echo "FAIL: the contract publishes '$token' but no lane builds it."
        ERRORS=$((ERRORS + 1))
    fi
done

# --- runner allowlist -----------------------------------------------------------
for runner in "${LANE_RUNNERS[@]}"; do
    if contains "$runner" "${ALLOWED_RUNNERS[@]}"; then
        echo "  ok  runner '$runner' is vetted"
    else
        echo "FAIL: runner '$runner' is not on the vetted allowlist."
        ERRORS=$((ERRORS + 1))
    fi
done

echo
if [[ "$ERRORS" -gt 0 ]]; then
    echo "RESULT: FAIL ($ERRORS problem(s))"
    exit 1
fi
echo "RESULT: OK"
