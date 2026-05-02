#!/bin/bash
set -e
echo "=== CogniCode Full Test Suite ==="
echo ""

echo "1. Unit tests (cognicode-axiom)..."
cargo test -p cognicode-axiom --lib -- --quiet

echo "2. Edge cases..."
cargo test -p cognicode-axiom --test edge_cases -- --quiet

echo "3. Rule fixtures (Rust + JS + Java + Python + Go)..."
cargo test -p cognicode-axiom --test rule_fixtures -- --nocapture --quiet

echo "4. System tests..."
cargo test -p cognicode-axiom --test system_integration -- --quiet

echo "5. MCP integration tests..."
cargo test -p cognicode-quality --test mcp_integration -- --quiet

echo ""
echo "=== All tests passed ==="
echo "Total: $(cargo test -p cognicode-axiom -p cognicode-quality 2>&1 | grep 'test result' | awk '{sum+=$2} END {print sum}') tests"
