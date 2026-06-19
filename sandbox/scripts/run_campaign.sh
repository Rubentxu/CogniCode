#!/usr/bin/env bash
# run_campaign.sh — Ejecutar una corrida per-run con reporte HTML aislado
#
# Uso:
#   bash sandbox/scripts/run_campaign.sh <manifest-path>...
#
# Crea un directorio sandbox/results-runs/<run-id>/ con los resultados
# y el reporte HTML. No contamina sandbox/results/ con corridas nuevas.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SANDBOX="$ROOT/sandbox"

# Run ID = timestamp UTC
RUN_ID="$(date -u +%Y%m%dT%H%M%S)"
RUN_DIR="$SANDBOX/results-runs/$RUN_ID"

# Validate arguments
if [ $# -lt 1 ]; then
    echo "Uso: $0 <manifest-path>..."
    echo ""
    echo "Ejemplo:"
    echo "  $0 sandbox/manifests/callgraph/rust.yaml"
    echo "  $0 sandbox/manifests/tier_b_*.yaml"
    exit 1
fi

# Create run directory
mkdir -p "$RUN_DIR"
echo "════════════════════════════════════════════════════════════"
echo "  Sandbox run: $RUN_ID"
echo "  Output dir:  $RUN_DIR"
echo "  Manifests:   $@"
echo "════════════════════════════════════════════════════════════"
echo ""

# Run the sandbox
cd "$ROOT"
DATABASE_URL="${DATABASE_URL:-postgres://cognicode:cognicode@localhost:5432/cognicode}" \
RUST_LOG="${RUST_LOG:-info}" \
    "$ROOT/target/debug/sandbox-orchestrator" run \
        --results-dir "$RUN_DIR" \
        "$@"

# Generate the per-run report
echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Generating per-run report"
echo "════════════════════════════════════════════════════════════"
python3 "$SANDBOX/scripts/generate_html_report.py" \
    --results-dir "$RUN_DIR" \
    --output "$RUN_DIR/report.html"

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Done"
echo "  Report: $RUN_DIR/report.html"
echo "  Open:   xdg-open $RUN_DIR/report.html"
echo "════════════════════════════════════════════════════════════"
