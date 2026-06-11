#!/usr/bin/env bash
# ============================================================================
# perf-budget-check.sh
# ----------------------------------------------------------------------------
# Runs the CogniCode benchmark suite, compares the mean time of each
# benchmark against the budget declared in `perf-budget.toml`, prints a
# results table, and exits 0 when every benchmark is at or below budget,
# 1 otherwise.
#
# Usage:  ./scripts/perf-budget-check.sh
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BUDGET_FILE="${PROJECT_ROOT}/perf-budget.toml"
BENCH_OUTPUT="$(mktemp -t cognicode-bench.XXXXXX)"
trap 'rm -f "${BENCH_OUTPUT}"' EXIT

# --- 1. Pre-flight checks ---------------------------------------------------

if [[ ! -f "${BUDGET_FILE}" ]]; then
    echo "ERROR: budget file not found at ${BUDGET_FILE}" >&2
    exit 2
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo not found in PATH" >&2
    exit 2
fi

if ! command -v awk >/dev/null 2>&1; then
    echo "ERROR: awk not found in PATH" >&2
    exit 2
fi

# --- 2. Run benchmarks ------------------------------------------------------

echo "=== Running benchmarks (this may take a few minutes) ==="
# `cargo bench` already passes `--bench` to the compiled binary, which
# switches Criterion into the real-bench harness (instead of the
# libtest "Testing X / Success" stub that ignores --output-format).
# We just need to add `--output-format bencher` so each result prints
# a parseable "test <name> ... bench: <value> <unit>/iter" line.
if ! cargo bench \
        -p cognicode-core \
        --bench graph_benchmarks \
        -- --output-format bencher \
        >"${BENCH_OUTPUT}" 2>&1; then
    echo "ERROR: cargo bench failed. Tail of output:" >&2
    tail -n 40 "${BENCH_OUTPUT}" >&2
    exit 2
fi

# --- 3. Parse benchmark output ---------------------------------------------
# Criterion's `bencher` output format looks like:
#   test <name> ... bench:           123 ns/iter (+/- 5)
#   test <name> ... bench:         2.345 us/iter (+/- 0.1)
# We normalise both units to microseconds.

# /tmp/bench.results contains: name<TAB>mean_us
PARSED="$(mktemp -t cognicode-parsed.XXXXXX)"
trap 'rm -f "${BENCH_OUTPUT}" "${PARSED}"' EXIT

awk '
    /bench:/ {
        # Extract name (first field after "test")
        name = $2
        # Iterate over tokens to find a "<number> <unit>/iter" pair
        for (i = 1; i <= NF; i++) {
            if ($i == "bench:") {
                val = $(i + 1)
                unit = $(i + 2)
                # strip "/iter" tail from unit
                sub(/\/iter$/, "", unit)
                break
            }
        }
        # Convert to microseconds
        if (unit == "ns")  us = val / 1000.0
        else               us = val          # us, ms, s already > us
        printf "%s\t%.4f\n", name, us
    }
' "${BENCH_OUTPUT}" > "${PARSED}"

# --- 4. Parse budget file ---------------------------------------------------
# Extracts "<section>.<key> = <value_us>" triples from perf-budget.toml.
# We treat the section name as a logical group and the key as the benchmark
# name. The current TOML is in microseconds already, so no conversion.

BUDGETS="$(mktemp -t cognicode-budgets.XXXXXX)"
trap 'rm -f "${BENCH_OUTPUT}" "${PARSED}" "${BUDGETS}"' EXIT

awk -v OUT="${BUDGETS}" '
    function flush_section() {
        if (section != "") {
            # nothing to do on flush, sections are kept in scope
        }
    }
    /^\[/ {
        # New section header like [graph.operations]
        section = $0
        gsub(/^\[|\]$/, "", section)
        next
    }
    /^[ \t]*#/ { next }   # comment line
    /^[ \t]*$/ { next }   # blank line
    /=/ {
        key = $1
        val = $3
        gsub(/[ \t]/, "", key)
        gsub(/[ \t]/, "", val)
        if (section != "" && key != "" && val != "") {
            printf "%s\t%s\n", key, val > OUT
        }
    }
' "${BUDGET_FILE}"

if [[ ! -s "${BUDGETS}" ]]; then
    echo "ERROR: no budget entries parsed from ${BUDGET_FILE}" >&2
    exit 2
fi

# --- 5. Compare and print the table ----------------------------------------

printf "\n%-32s %14s %14s %10s\n" "OPERATION" "BUDGET (us)" "ACTUAL (us)" "STATUS"
printf -- "-%.0s" {1..75}; printf "\n"

# Track which budget entries were checked
declare -A CHECKED

while IFS=$'\t' read -r bench_name actual_us; do
    [[ -z "${bench_name}" ]] && continue
    budget_us="$(awk -F'\t' -v n="${bench_name}" '$1 == n { print $2; exit }' "${BUDGETS}")"
    if [[ -z "${budget_us}" ]]; then
        # No budget for this benchmark — skip silently (other benches may
        # be exploratory and out of scope for the budget gate).
        continue
    fi
    CHECKED["${bench_name}"]=1

    # awk comparison: prints PASS/FAIL
    status="$(awk -v a="${actual_us}" -v b="${budget_us}" 'BEGIN {
        if (a <= b) print "PASS"; else print "FAIL"
    }')"

    printf "%-32s %14s %14s %10s\n" \
        "${bench_name}" "${budget_us}" "${actual_us}" "${status}"
done < "${PARSED}"

# --- 6. Report on any budget entries that no benchmark produced ------------

while IFS=$'\t' read -r bench_name budget_us; do
    [[ -z "${bench_name}" ]] && continue
    if [[ -z "${CHECKED[${bench_name}]:-}" ]]; then
        printf "%-32s %14s %14s %10s\n" \
            "${bench_name}" "${budget_us}" "(missing)" "SKIP"
    fi
done < "${BUDGETS}"

# --- 7. Decide exit code ----------------------------------------------------
# FAIL if any benchmark is over budget.

fail_count="$(awk -F'\t' '
    FILENAME == ARGV[1] { actual[$1] = $2; next }
    FILENAME == ARGV[2] { budget[$1] = $2; next }
    END {
        fails = 0
        for (name in actual) {
            if (name in budget && actual[name] + 0 > budget[name] + 0) {
                fails++
            }
        }
        print fails
    }
' "${PARSED}" "${BUDGETS}")"

echo
if [[ "${fail_count}" -gt 0 ]]; then
    echo "=== ${fail_count} benchmark(s) over budget ==="
    exit 1
fi
echo "=== All benchmarks within budget ==="
exit 0
