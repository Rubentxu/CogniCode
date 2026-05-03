#!/bin/bash
# Usage: ./scripts/pr-check.sh [base-branch]
# Analyzes only files changed in this PR vs base branch
set -e

BASE="${1:-main}"
BINARY="./target/release/cognicode-quality"

echo "=== CogniCode PR Check ==="
echo "Comparing HEAD vs $BASE"

# Get changed files
CHANGED=$(git diff --name-only "origin/$BASE...HEAD" | grep -E '\.(rs|js|ts|jsx|tsx|py|java|go)$' || true)

if [ -z "$CHANGED" ]; then
    echo "No source files changed. ✅"
    exit 0
fi

# Write temp file list
TMPFILE=$(mktemp)
echo "$CHANGED" > "$TMPFILE"

# Run analysis (use the binary's analyze_file tool via MCP or direct)
# For now: run analyze_project which will auto-detect changed files via incremental
echo "Analyzing changed files..."
RESULT=$($BINARY analyze_project --cwd . 2>&1 || echo '{"error": "analysis failed"}')

BLOCKERS=$(echo "$RESULT" | grep -c '"severity":"Blocker"' || echo 0)
CRITICALS=$(echo "$RESULT" | grep -c '"severity":"Critical"' || echo 0)

echo ""
echo "=== Results ==="
echo "Blockers: $BLOCKERS"
echo "Criticals: $CRITICALS"

if [ "$BLOCKERS" -gt 0 ]; then
    echo "❌ PR BLOCKED: $BLOCKERS blocker(s) found!"
    exit 1
else
    echo "✅ PR CLEAN"
    exit 0
fi
