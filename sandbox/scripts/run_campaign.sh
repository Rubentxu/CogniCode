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
    # Lua
    printf '-- Lua test fixture\nfunction greet(name)\n    return "Hello, " .. name\nend\nlocal function compute(x)\n    return x * 2\nend\nfunction main()\n    local result = compute(42)\n    print(greet("world"))\n    return result\nend\nmain()\n' > "$TIERC_DIR/main.lua"
    # Zig
    printf '// Zig test fixture\nconst std = @import("std");\n\nfn compute(x: i32) i32 {\n    return x * 2;\n}\n\nfn greet(name: []const u8) !void {\n    const stdout = std.io.getStdOut().writer();\n    try stdout.print("Hello, {s}\\n", .{name});\n}\n\npub fn main() !void {\n    const result = compute(42);\n    try greet("world");\n    _ = result;\n}\n' > "$TIERC_DIR/main.zig"
    # Dart
    printf '// Dart test fixture\nint compute(int x) {\n  return x * 2;\n}\n\nvoid greet(String name) {\n  print("Hello, $name");\n}\n\nvoid main() {\n  var result = compute(42);\n  greet("world");\n  print(result);\n}\n' > "$TIERC_DIR/main.dart"
    # Haskell
    printf '-- Haskell test fixture\nmodule Main where\n\ncompute :: Int -> Int\ncompute x = x * 2\n\ngreet :: String -> IO ()\ngreet name = putStrLn ("Hello, " ++ name)\n\nmain :: IO ()\nmain = do\n    let result = compute 42\n    greet "world"\n    print result\n' > "$TIERC_DIR/Main.hs"
    # Julia
    printf '# Julia test fixture\nfunction compute(x)\n    return x * 2\nend\n\nfunction greet(name)\n    println("Hello, ", name)\nend\n\nfunction main()\n    result = compute(42)\n    greet("world")\n    println(result)\nend\n\nmain()\n' > "$TIERC_DIR/main.jl"
    # Scala
    printf '// Scala test fixture\nobject Main {\n  def compute(x: Int): Int = x * 2\n\n  def greet(name: String): Unit = println(s"Hello, $name")\n\n  def main(args: Array[String]): Unit = {\n    val result = compute(42)\n    greet("world")\n    println(result)\n  }\n}\n' > "$TIERC_DIR/Main.scala"
    # Groovy
    printf '// Groovy test fixture\ndef compute(x) {\n    x * 2\n}\n\ndef greet(name) {\n    println "Hello, $name"\n}\n\ndef main() {\n    def result = compute(42)\n    greet("world")\n    println result\n}\n\nmain()\n' > "$TIERC_DIR/Main.groovy"
    # Erlang
    printf '%% Erlang test fixture\n-module(main).\n-export([main/0]).\n\ncompute(X) -> X * 2.\n\ngreet(Name) -> io:format("Hello, ~s~n", [Name]).\n\nmain() ->\n    Result = compute(42),\n    greet("world"),\n    io:format("~p~n", [Result]).\n' > "$TIERC_DIR/main.erl"
    # Bash
    printf '#!/usr/bin/env bash\n# Bash test fixture\n\ncompute() {\n    echo $(( $1 * 2 ))\n}\n\ngreet() {\n    echo "Hello, $1"\n}\n\nmain() {\n    local result\n    result=$(compute 42)\n    greet "world"\n    echo "$result"\n}\n\nmain\n' > "$TIERC_DIR/script.sh"
    # R
    printf '# R test fixture\ncompute <- function(x) { x * 2 }\ngreet <- function(name) { cat("Hello, ", name, "\\n") }\nmain <- function() { result <- compute(42); greet("world"); cat(result, "\\n") }\nmain()\n' > "$TIERC_DIR/main.R"
    # PowerShell
    printf '# PowerShell test fixture\nfunction Compute($x) { return $x * 2 }\nfunction Greet($name) { Write-Output "Hello, $name" }\n$result = Compute(42)\nGreet("world")\nWrite-Output $result\n' > "$TIERC_DIR/script.ps1"
    # Fortran
    printf '! Fortran test fixture\nprogram main\n    implicit none\n    integer :: result\n    result = compute(42)\n    call greet("world")\n    print *, result\ncontains\n    integer function compute(x)\n        integer, intent(in) :: x\n        compute = x * 2\n    end function compute\n    subroutine greet(name)\n        character(len=*), intent(in) :: name\n        print *, "Hello, ", name\n    end subroutine greet\nend program main\n' > "$TIERC_DIR/main.f90"
    # Verilog
    printf '// Verilog test fixture\nmodule main;\n    integer result;\n\n    function integer compute;\n        input integer x;\n        begin\n            compute = x * 2;\n        end\n    endfunction\n\n    initial begin\n        result = compute(42);\n        $display("Result: %0d", result);\n    end\nendmodule\n' > "$TIERC_DIR/main.v"
    # SystemVerilog
    printf '// SystemVerilog test fixture\nmodule main;\n    int result;\n\n    function int compute(int x);\n        return x * 2;\n    endfunction\n\n    initial begin\n        result = compute(42);\n        $display("Result: %0d", result);\n    end\nendmodule\n' > "$TIERC_DIR/main.sv"
    # JSON
    printf '{\n  "name": "test",\n  "version": "1.0",\n  "dependencies": {\n    "compute": 42\n  }\n}\n' > "$TIERC_DIR/data.json"
    echo "  [SETUP] Created 15 Tier C fixtures in $TIERC_DIR"
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
