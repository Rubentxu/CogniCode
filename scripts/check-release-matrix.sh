#!/usr/bin/env bash
# e85 — release contract coherence check.
#
# Enforces the release invariants in CI:
#
#   R9  every platform the release contract publishes must have exactly one
#       lane, and that lane's target must be exactly the token the contract
#       derives for it. The contract (`cognicode-release platform-token`) is
#       the authority; the lane must not carry a second, drifting copy of the
#       mapping.
#
#   Tier-1 completeness: every platform the contract publishes must have a lane,
#       and every lane must be a platform the contract knows.
#
# This replaces the e74 version, which required *every* `Platform` variant to
# have a matrix lane. e84 WU9 deliberately changed that: Tier 1 is Linux GNU on
# native runners (x86_64 + aarch64), while MUSL, macOS and Windows keep their
# Platform variants and adapter seams without being advertised as supported.
# The old invariant would have forced us to claim support we do not have.
#
# ## What it used to read, and why it stopped
#
# The lane list came out of `.github/workflows/release.yml` with three greps,
# one each for `platform:`, `rust_target:` and `runner:`, and a fourth
# allowlist of vetted GitHub-hosted runner labels. That is four more places
# stating the same facts, in a file this check exists to outlive, and the
# greps were shaped around a YAML block: reformatting one matrix entry would
# have read as "the lanes drifted" and failed a check that had nothing to say
# about the platforms.
#
# The lanes now come from `release-candidate.pipeline.kts`, through the same
# reader both product generators use, and the runner allowlist is gone: there
# is no runner label in a PipelineK lane, so an allowlist over labels that no
# longer exist is a list of permissions over nothing. Which runners a release
# builds on is a provisioning question, not a coherence one.
#
# Two authorities remain, deliberately: the release contract says what the
# *product* supports, and the lane says what is *built*. R9 is the check that
# they agree, which is only meaningful while they are separate.
#
# Exit codes: 0 in sync, 1 mismatch.

set -euo pipefail

cd "$(dirname "$0")/.."

# Resolve the real target directory: it can be moved by CARGO_TARGET_DIR or by a
# repository or user `.cargo/config.toml`, so never assume `target/`. The
# resolver is shared with every pipeline stage that has to find a built binary;
# a second copy of cargo's config resolution here would be a second thing to
# drift.
TARGET_DIR=$(scripts/ci/target-dir.sh)
RELEASE_BIN="$TARGET_DIR/release/cognicode-release"

if [[ ! -x "$RELEASE_BIN" ]]; then
    echo "building $RELEASE_BIN ..."
    cargo build --release --bin cognicode-release >/dev/null
fi
if [[ ! -x "$RELEASE_BIN" ]]; then
    echo "FAIL: could not build $RELEASE_BIN"
    exit 1
fi

# --- what the contract publishes ------------------------------------------------
mapfile -t TIER1_TOKENS < <("$RELEASE_BIN" platforms | sort -u)

# --- what the release lane builds ----------------------------------------------
# `<platform-id> <target>`, from the lane's `val targets` and its `platformOf`
# mapping, read through the shared reader rather than by a third parse. The
# reader also refuses to answer if the two declarations disagree, which is the
# failure this check would otherwise report as a lane mismatch with the wrong
# target named.
if ! LANE_PAIRS=$(python3 scripts/product/release_lane.py); then
    echo "FAIL: could not read the release lane's platform declarations."
    exit 1
fi
mapfile -t LANE_PLATFORMS < <(printf '%s\n' "$LANE_PAIRS" | cut -f1 | sort -u)
mapfile -t LANE_TOKENS < <(printf '%s\n' "$LANE_PAIRS" | cut -f2 | sort -u)

contains() { local needle=$1; shift; local x; for x in "$@"; do [[ "$x" == "$needle" ]] && return 0; done; return 1; }

echo "=== e85 release contract coherence ==="
echo
echo "Tier-1 tokens published by the contract:"
printf '  %s\n' "${TIER1_TOKENS[@]}"
echo
echo "Lanes declared in release-candidate.pipeline.kts:"
for platform in "${LANE_PLATFORMS[@]}"; do
    printf '  %s -> %s\n' "$platform" "$("$RELEASE_BIN" platform-token --platform "$platform" 2>/dev/null || echo '?')"
done
echo

ERRORS=0

if [[ "${#LANE_PLATFORMS[@]}" -ne "${#LANE_TOKENS[@]}" ]]; then
    echo "FAIL: ${#LANE_PLATFORMS[@]} lane(s) but ${#LANE_TOKENS[@]} target(s)."
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
        echo "FAIL: lane '$platform' should build '$derived' (R9)."
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

echo
if [[ "$ERRORS" -gt 0 ]]; then
    echo "RESULT: FAIL ($ERRORS problem(s))"
    exit 1
fi
echo "RESULT: OK"
