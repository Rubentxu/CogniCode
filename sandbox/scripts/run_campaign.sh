#!/usr/bin/env bash
# run_campaign.sh — Ejecutar una corrida per-run con reporte HTML aislado
#
# Uso:
#   bash sandbox/scripts/run_campaign.sh [-j N] <manifest-path>...
#
# -j N: ejecuta N manifests en paralelo (default: 1)
#
# Crea un directorio sandbox/results-runs/<run-id>/ con los resultados
# y el reporte HTML. No contamina sandbox/results/ con corridas nuevas.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SANDBOX="$ROOT/sandbox"

# ── Setup: ensure Tier C synthetic fixtures exist ──
# These are small test files per language, not tracked in git
TIERC_DIR="$SANDBOX/fixtures/tierc"
if [ ! -f "$TIERC_DIR/main.lua" ]; then
    echo "  [SETUP] Creating Tier C synthetic fixtures..."
    mkdir -p "$TIERC_DIR"
    cat > "$TIERC_DIR/main.lua" << 'LUA'
-- Lua test fixture
function greet(name)
    return "Hello, " .. name
end
local function compute(x)
    return x * 2
end
function main()
    local result = compute(42)
    print(greet("world"))
    return result
end
main()
LUA
    cat > "$TIERC_DIR/main.zig" << 'ZIG'
// Zig test fixture
const std = @import("std");
fn compute(x: i32) i32 { return x * 2; }
fn greet(name: []const u8) !void {
    const stdout = std.io.getStdOut().writer();
    try stdout.print("Hello, {s}\n", .{name});
}
pub fn main() !void {
    const result = compute(42);
    try greet("world");
    _ = result;
}
ZIG
    cat > "$TIERC_DIR/main.dart" << 'DART'
// Dart test fixture
int compute(int x) { return x * 2; }
void greet(String name) { print("Hello, $name"); }
void main() { var result = compute(42); greet("world"); print(result); }
DART
    cat > "$TIERC_DIR/Main.hs" << 'HASKELL'
-- Haskell test fixture
module Main where
compute :: Int -> Int
compute x = x * 2
greet :: String -> IO ()
greet name = putStrLn ("Hello, " ++ name)
main :: IO ()
main = do let result = compute 42; greet "world"; print result
HASKELL
    cat > "$TIERC_DIR/main.jl" << 'JULIA'
# Julia test fixture
function compute(x) return x * 2 end
function greet(name) println("Hello, ", name) end
function main() result = compute(42); greet("world"); println(result) end
main()
JULIA
    cat > "$TIERC_DIR/Main.scala" << 'SCALA'
// Scala test fixture
object Main {
  def compute(x: Int): Int = x * 2
  def greet(name: String): Unit = println(s"Hello, $name")
  def main(args: Array[String]): Unit = { val result = compute(42); greet("world"); println(result) }
}
SCALA
    cat > "$TIERC_DIR/Main.groovy" << 'GROOVY'
// Groovy test fixture
def compute(x) { x * 2 }
def greet(name) { println "Hello, $name" }
def main() { def result = compute(42); greet("world"); println result }
main()
GROOVY
    cat > "$TIERC_DIR/main.erl" << 'ERLANG'
%% Erlang test fixture
-module(main).
-export([main/0]).
compute(X) -> X * 2.
greet(Name) -> io:format("Hello, ~s~n", [Name]).
main() -> Result = compute(42), greet("world"), io:format("~p~n", [Result]).
ERLANG
    cat > "$TIERC_DIR/script.sh" << 'BASH'
#!/usr/bin/env bash
compute() { echo $(( $1 * 2 )); }
greet() { echo "Hello, $1"; }
main() { local result; result=$(compute 42); greet "world"; echo "$result"; }
main
BASH
    cat > "$TIERC_DIR/main.R" << 'RSCRIPT'
# R test fixture
compute <- function(x) x * 2
greet <- function(name) cat("Hello, ", name, "\n")
main <- function() { result <- compute(42); greet("world"); cat(result, "\n") }
main()
RSCRIPT
    cat > "$TIERC_DIR/script.ps1" << 'PS'
# PowerShell test fixture
$result = 42 * 2
Write-Output "Hello, world"
Write-Output $result
PS
    cat > "$TIERC_DIR/main.f90" << 'F90'
! Fortran test fixture
program main
integer :: result
result = compute(42)
call greet("world")
print *, result
contains
integer function compute(x)
integer, intent(in) :: x
compute = x * 2
end function compute
subroutine greet(name)
character(len=*), intent(in) :: name
print *, "Hello, ", name
end subroutine greet
end program main
F90
    cat > "$TIERC_DIR/main.v" << 'VERILOG'
// Verilog test fixture
module main;
integer result;
function integer compute;
input integer x;
begin compute = x * 2; end
endfunction
initial begin result = compute(42); $display("Result: %0d", result); end
endmodule
VERILOG
    cat > "$TIERC_DIR/main.sv" << 'SYSV'
// SystemVerilog test fixture
module main;
int result;
function int compute(int x); return x * 2; endfunction
initial begin result = compute(42); $display("Result: %0d", result); end
endmodule
SYSV
    cat > "$TIERC_DIR/data.json" << 'JSON'
{"name": "test", "version": "1.0", "dependencies": {"compute": 42}}
JSON
    echo "  [SETUP] Created 15 Tier C fixtures in $TIERC_DIR"
    # Lua
fi

# Parse parallel flag
JOBS=1
MANIFESTS=()
while [ $# -gt 0 ]; do
    case "$1" in
        -j) JOBS="$2"; shift 2 ;;
        -j?*) JOBS="${1#-j}"; shift ;;
        *) MANIFESTS+=("$1"); shift ;;
    esac
done

# Validate
if [ ${#MANIFESTS[@]} -eq 0 ]; then
    echo "Uso: $0 [-j N] <manifest-path>..."
    echo ""
    echo "Ejemplo:"
    echo "  $0 -j 4 sandbox/manifests/tier_b_*.yaml"
    exit 1
fi

if [ "$JOBS" -lt 1 ]; then
    JOBS=1
fi

# Run ID = timestamp UTC
RUN_ID="$(date -u +%Y%m%dT%H%M%S)"
RUN_DIR="$SANDBOX/results-runs/$RUN_ID"
mkdir -p "$RUN_DIR"

echo "════════════════════════════════════════════════════════════"
echo "  Sandbox run: $RUN_ID"
echo "  Output dir:  $RUN_DIR"
echo "  Manifests:   ${#MANIFESTS[@]} files"
echo "  Parallel:    $JOBS workers"
echo "════════════════════════════════════════════════════════════"

cd "$ROOT"

# ── Distribute manifests across N workers ──
# Each worker gets roughly equal manifests
WORKER_COUNT=$JOBS
MANIFEST_COUNT=${#MANIFESTS[@]}
MANIFESTS_PER_WORKER=$(( (MANIFEST_COUNT + WORKER_COUNT - 1) / WORKER_COUNT ))

PIDS=()
WORKER_IDS=()

for ((w=0; w<WORKER_COUNT; w++)); do
    START=$(( w * MANIFESTS_PER_WORKER ))
    if [ "$START" -ge "$MANIFEST_COUNT" ]; then
        break
    fi
    END=$(( START + MANIFESTS_PER_WORKER ))
    if [ "$END" -gt "$MANIFEST_COUNT" ]; then
        END=$MANIFEST_COUNT
    fi
    
    # Build the manifest slice for this worker
    WORKER_MANIFESTS=("${MANIFESTS[@]:START:END-START}")
    
    (
        # Each worker gets its own subdirectory to avoid mixing
        WORKER_RUN_DIR="$RUN_DIR/worker-$w"
        mkdir -p "$WORKER_RUN_DIR"
        
        DATABASE_URL="${DATABASE_URL:-postgres://cognicode:cognicode@localhost:5432/cognicode}" \
        RUST_LOG="${RUST_LOG:-error}" \
        nice -n 19 \
        "$ROOT/target/debug/sandbox-orchestrator" run \
            --results-dir "$WORKER_RUN_DIR" \
            "${WORKER_MANIFESTS[@]}" \
            2>&1 | sed "s/^/[worker $w] /"
    ) &
    PIDS+=($!)
    WORKER_IDS+=($w)
    echo "  Worker $w started: ${WORKER_MANIFESTS[*]}"
done

echo ""
echo "⏳ Waiting for all workers to complete..."

# Wait for all workers and collect exit codes
FAILURES=0
for pid in "${PIDS[@]}"; do
    wait "$pid" || FAILURES=$((FAILURES + 1))
done

# ── Merge results: copy worker results into main run dir ──
# The sandbox stores results as: {scenario_id}/{run_id}/result.json
# We need to merge them into the root results dir
for w in "${WORKER_IDS[@]}"; do
    WORKER_DIR="$RUN_DIR/worker-$w"
    if [ -d "$WORKER_DIR" ]; then
        # Move scenario dirs up one level
        for scenario_dir in "$WORKER_DIR"/*; do
            if [ -d "$scenario_dir" ]; then
                scenario_name=$(basename "$scenario_dir")
                # Skip summary files
                [ "$scenario_name" = "summary.json" ] && continue
                [ "$scenario_name" = "run.jsonl" ] && continue
                # Move into main results dir
                mkdir -p "$RUN_DIR/$scenario_name"
                for run_dir in "$scenario_dir"/*/; do
                    if [ -d "$run_dir" ]; then
                        cp -r "$run_dir" "$RUN_DIR/$scenario_name/"
                    fi
                done
            fi
        done
    fi
done

# ── Generate a merged summary ──
cd "$ROOT"
# Use the parallel-safe sandbox report command to aggregate
# (this reads all result.json files from the merged dir)
python3 "$SANDBOX/scripts/generate_html_report.py" \
    --results-dir "$RUN_DIR" \
    --output "$RUN_DIR/report.html"

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Campaign complete ($([ $FAILURES -eq 0 ] && echo 'ALL PASS' || echo "$FAILURES WORKER(S) FAILED"))"
echo "  Report: $RUN_DIR/report.html"
echo "  Total:  $(find "$RUN_DIR" -name 'result.json' | wc -l) scenarios"
echo "════════════════════════════════════════════════════════════"

# Cleanup worker dirs
for w in "${WORKER_IDS[@]}"; do
    rm -rf "$RUN_DIR/worker-$w"
done
