#!/usr/bin/env bash
# ============================================================================
# run-all-contracts.sh — the single entry point for the contract suite.
# ----------------------------------------------------------------------------
# Every orchestrator calls this. Nothing else lists the contract files.
#
# Why this exists, measured 2026-10-02. The list of contract files lived inside
# `.github/workflows/pr-ci.yml`, and eight contract files had to be named in it
# by hand. That is the CP5 failure waiting to happen again: a new contract is
# written, it passes locally, and it runs in CI only if whoever wrote it also
# remembered to edit a YAML list. The CP5 skill gate was green for a long time
# and was invoked by no workflow at all.
#
# A glob removes the second step. `scripts/ci/test_*.py` was verified to match
# exactly the eight files the hand-written list named, so discovery by glob is
# not a loosening — it is the same set, with no opportunity to forget one.
#
# The orchestrators are GitHub Actions today and PipelineK once the migration
# closes. Both call this file. Adding a third orchestrator must not mean
# editing this list again.
#
# Fail-closed: a glob that matches nothing runs nothing and exits 0, which is
# the vacuous pass this repository keeps refusing to ship. An empty contract
# suite is an error here, not a clean bill.
#
# Usage:  ./scripts/ci/run-all-contracts.sh
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${PROJECT_ROOT}"

if ! command -v python3 >/dev/null 2>&1; then
    echo "ERROR: python3 not found in PATH" >&2
    exit 2
fi

if [[ ! -f scripts/run_contract_tests.py ]]; then
    echo "ERROR: contract runner not found at scripts/run_contract_tests.py" >&2
    exit 2
fi

# The product contracts are already globbed in the runner invocation below.
# The CI contracts are discovered here so no orchestrator has to name them.
shopt -s nullglob
CI_CONTRACTS=(scripts/ci/test_*.py)
shopt -u nullglob

if [[ ${#CI_CONTRACTS[@]} -eq 0 ]]; then
    echo "ERROR: no contracts matched scripts/ci/test_*.py" >&2
    echo "Either the contracts were moved or deleted. An empty contract suite" >&2
    echo "is not a passing contract suite." >&2
    exit 3
fi

echo "=== contract suite: ${#CI_CONTRACTS[@]} CI contracts + scripts/product/*.py ==="
printf '  %s\n' "${CI_CONTRACTS[@]}"

exec python3 scripts/run_contract_tests.py \
    scripts/product/test_*.py \
    "${CI_CONTRACTS[@]}"
