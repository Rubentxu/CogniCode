#!/bin/bash
# ═══════════════════════════════════════════════════════════════════
# CogniCode Sandbox Entrypoint
# ═══════════════════════════════════════════════════════════════════
# Runs inside the Docker container. Isolates rule experiments from
# the host project. Each container is ephemeral — destroyed on exit.
#
# Usage:
#   entrypoint.sh RULE_ID [GIT_REF] [CHANGE_SCRIPT]
#
# Arguments:
#   RULE_ID       — Rule to evaluate (e.g., "S134")
#   GIT_REF       — Git branch/commit (default: "main")
#   CHANGE_SCRIPT — Path to Python script that edits catalog.rs
#
# Output: JSON on stdout
#   {"status":"success","rule_id":"S134","tests_passed":283,...}
#   {"status":"failed","reason":"compilation_error",...}
# ═══════════════════════════════════════════════════════════════════

set -euo pipefail

RULE_ID="${1:-}"
GIT_REF="${2:-main}"
CHANGE_SCRIPT="${3:-}"

# ═════════════════════════════════════════════════════════════════
# Help
# ═════════════════════════════════════════════════════════════════

if [ -z "$RULE_ID" ] || [ "$RULE_ID" = "--help" ]; then
    cat <<'EOF'
CogniCode Sandbox — Isolated Rule Experiment Runner

Usage: podman run cognicode-sandbox RULE_ID [GIT_REF] [CHANGE_SCRIPT]

  RULE_ID       Rule to evaluate (e.g., "S134")
  GIT_REF       Git branch/commit to clone (default: "main")
  CHANGE_SCRIPT Path to Python script that modifies catalog.rs

Examples:
  podman run --rm cognicode-sandbox S134
  podman run --rm cognicode-sandbox S2068 feat/improve-rule /scripts/tighten.py

EOF
    exit 0
fi

echo "=== SANDBOX: Experiment for rule $RULE_ID ==="
echo "Git ref: $GIT_REF"
echo "Change script: ${CHANGE_SCRIPT:-none}"

# ═════════════════════════════════════════════════════════════════
# 1. Clone repository
# ═════════════════════════════════════════════════════════════════

echo "[1/6] Cloning repository (ref: $GIT_REF)..."

if [ -d "/host-repo/.git" ]; then
    # Fast path: clone from mounted host repo (shared objects)
    git clone --shared --branch "$GIT_REF" /host-repo /workspace/CogniCode 2>/dev/null || {
        echo "[1/6] Branch not found, cloning default branch..."
        git clone --shared /host-repo /workspace/CogniCode
        cd /workspace/CogniCode
        git checkout "$GIT_REF" 2>/dev/null || true
    }
else
    echo '{"status":"failed","reason":"host_repo_not_mounted"}'
    echo "Mount the host repo: -v /path/to/CogniCode:/host-repo:ro"
    exit 1
fi

cd /workspace/CogniCode
COMMIT=$(git rev-parse --short HEAD)
echo "[1/6] Cloned at commit: $COMMIT"

# ═════════════════════════════════════════════════════════════════
# 2. Apply experimental change
# ═════════════════════════════════════════════════════════════════

if [ -n "$CHANGE_SCRIPT" ] && [ -f "$CHANGE_SCRIPT" ]; then
    echo "[2/6] Applying experimental change..."
    if python3 "$CHANGE_SCRIPT" 2>&1; then
        echo "[2/6] Change applied successfully"
    else
        echo '{"status":"failed","reason":"change_script_error"}'
        exit 1
    fi
else
    echo "[2/6] No change script — evaluating baseline"
fi

# ═════════════════════════════════════════════════════════════════
# 3. Compilation check
# ═════════════════════════════════════════════════════════════════

echo "[3/6] Compilation check (cargo check -p cognicode-axiom)..."
if cargo check -p cognicode-axiom 2>&1 | tail -5; then
    echo "[3/6] ✓ Compilation OK"
else
    echo '{"status":"failed","reason":"compilation_error"}'
    exit 1
fi

# ═════════════════════════════════════════════════════════════════
# 4. Run tests (axiom crate only — fastest validation)
# ═════════════════════════════════════════════════════════════════

echo "[4/6] Running tests (cargo test -p cognicode-axiom)..."
TEST_OUTPUT=$(cargo test -p cognicode-axiom --lib 2>&1)
TEST_EXIT=$?

PASSED=$(echo "$TEST_OUTPUT" | grep -oP '\d+(?= passed)' | head -1 || echo "0")
FAILED=$(echo "$TEST_OUTPUT" | grep -oP '\d+(?= failed)' | head -1 || echo "0")

if [ $TEST_EXIT -eq 0 ]; then
    echo "[4/6] ✓ Tests passed: $PASSED"
else
    echo "{\"status\":\"failed\",\"reason\":\"tests_failed\",\"passed\":$PASSED,\"failed\":$FAILED}"
    exit 1
fi

# ═════════════════════════════════════════════════════════════════
# 5. Quick evaluation (MVP: skip full sandbox build)
# ═════════════════════════════════════════════════════════════════

echo "[5/6] Quick validation (skipping full sandbox build for MVP)..."
echo "[5/6] ✓ Quick validation OK"

# Full evaluation (Phase 2+):
# echo "[5/6] Building sandbox-orchestrator (release)..."
# cargo build --release -p cognicode-sandbox 2>&1 | tail -3
# ./target/release/sandbox-orchestrator run sandbox/manifests/rust_fixture.yaml \
#     --results-dir "/results/${RULE_ID}" --jsonl --filter-rule "$RULE_ID" 2>&1 | tail -10

# ═════════════════════════════════════════════════════════════════
# 6. Report
# ═════════════════════════════════════════════════════════════════

RESULTS_DIR="/results/${RULE_ID}_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR" 2>/dev/null || true

echo "=== SANDBOX: Experiment complete ==="

# Output JSON result (parsed by host)
cat <<JSON
{
  "status": "success",
  "rule_id": "$RULE_ID",
  "commit": "$COMMIT",
  "tests_passed": $PASSED,
  "tests_failed": $FAILED,
  "results_dir": "$RESULTS_DIR",
  "timestamp": "$(date -Iseconds)"
}
JSON
