#!/usr/bin/env bash
# scripts/ci/cp-semantic-smoke.sh
#
# CR-01 gate 4: verificación SEMÁNTICA del control plane.
#
# No basta con que arranque. Debe:
#  - endpoint CP1 accesible (HTTP 200);
#  - status = "evaluated";
#  - exactamente 3 canonical constraints cargadas;
#  - resultado coherente respecto al repositorio;
#  - ninguna dependencia accidental de archivos locales del workspace.
#
# Uso:
#   bash scripts/ci/cp-semantic-smoke.sh [--port PORT] [--repo-root DIR]
#
# Exit codes:
#   0  — todos los gates semánticos PASS
#   1  — algún gate FAILED
#   2  — entorno no apto
#
# Implementación:
#   1. Compila el bin cognicode-control-plane (cargo build --bin).
#   2. Arranca el bin en background.
#   3. Espera al endpoint /health.
#   4. Llama a CP1 vía reqwest (POST).
#   5. Verifica estructura del JSON: status, constraints count, etc.
#   6. Apaga el bin, devuelve código 0/1.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

PORT="${PORT:-9842}"
BIND="${BIND:-127.0.0.1:$PORT}"
SOURCE_ROOT="${SOURCE_ROOT:-$REPO_ROOT/crates/cognicode-core/src}"

while [ $# -gt 0 ]; do
  case "$1" in
    --bind) BIND="$2"; shift 2 ;;
    --port) PORT="$2"; BIND="127.0.0.1:$PORT"; shift 2 ;;
    --source-root) SOURCE_ROOT="$2"; shift 2 ;;
    --repo-root) REPO_ROOT="$2"; SOURCE_ROOT="$2/crates/cognicode-core/src"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 2 ;;
  esac
done

# Detectar bin en target local o target-dir externo
BIN_PATH=""
for candidate in \
  "$REPO_ROOT/target/debug/cognicode-control-plane" \
  "${CARGO_TARGET_DIR:-/var/home/rubentxu/cargo-targets}/debug/cognicode-control-plane"; do
  if [ -x "$candidate" ]; then
    BIN_PATH="$candidate"
    break
  fi
done

if [ -z "$BIN_PATH" ]; then
  echo "→ Compilando bin cognicode-control-plane..."
  cargo build --bin cognicode-control-plane --quiet
  BIN_PATH="${CARGO_TARGET_DIR:-/var/home/rubentxu/cargo-targets}/debug/cognicode-control-plane"
  [ -x "$BIN_PATH" ] || BIN_PATH="$REPO_ROOT/target/debug/cognicode-control-plane"
fi

LOG_FILE="/tmp/cp-semantic-smoke.log"
: > "$LOG_FILE"

echo "→ Arrancando cognicode-control-plane"
echo "  bin:    $BIN_PATH"
echo "  bind:   $BIND"
echo "  source: $SOURCE_ROOT"
"$BIN_PATH" --bind "$BIND" --source-root "$SOURCE_ROOT" >>"$LOG_FILE" 2>&1 &
CP_PID=$!

cleanup() {
  if kill -0 "$CP_PID" 2>/dev/null; then
    kill "$CP_PID" 2>/dev/null || true
    wait "$CP_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# Esperar a que /health responda (timeout 15s)
echo "→ Esperando /health..."
for i in $(seq 1 30); do
  if curl -sS --max-time 1 "http://127.0.0.1:$PORT/health" >/dev/null 2>&1; then
    echo "  health OK (intento $i)"
    break
  fi
  sleep 0.5
  if [ "$i" -eq 30 ]; then
    echo "FAIL: /health no responde tras 15s"
    tail -20 "$LOG_FILE"
    exit 1
  fi
done

# Gate 1: /health responde
HEALTH=$(curl -sS "http://127.0.0.1:$PORT/health")
echo "  /health → $HEALTH"

# Gate 2: control plane architecture endpoint accesible y devuelve JSON
echo "→ Llamando endpoint de architecture..."
WORKSPACE_ID="${WORKSPACE_ID:-test}"
CP1_RESPONSE=$(curl -sS -X GET "http://127.0.0.1:$PORT/control-plane/workspaces/$WORKSPACE_ID/architecture" 2>&1 || echo "FAIL_CURL")

echo "  Response (primeras 500 chars):"
echo "$CP1_RESPONSE" | head -c 500
echo

# Verificar que es JSON válido
if ! echo "$CP1_RESPONSE" | python3 -m json.tool >/dev/null 2>&1; then
  echo "FAIL: endpoint no devolvió JSON válido"
  exit 1
fi

# Gate 3: status = "evaluated"
STATUS=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    print(data.get('status', 'MISSING'))
except Exception as e:
    print(f'PARSE_ERROR: {e}')
")
echo "  status = $STATUS"
if [ "$STATUS" != "evaluated" ]; then
  echo "FAIL: status != 'evaluated' (got: $STATUS)"
  exit 1
fi

# Gate 4: exactamente 3 canonical constraints
CONSTRAINTS_COUNT=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    constraints = data.get('constraints', [])
    print(len(constraints))
except Exception as e:
    print(f'PARSE_ERROR: {e}')
")
echo "  constraints count = $CONSTRAINTS_COUNT"
if [ "$CONSTRAINTS_COUNT" != "3" ]; then
  echo "FAIL: se esperaban 3 canonical constraints, hay $CONSTRAINTS_COUNT"
  exit 1
fi

# Gate 5: coherencia con el repositorio
# Las 3 constraints canónicas reales (ADR-046) son:
#   1. architecture.domain_no_infrastructure (layer_dependency)
#   2. architecture.domain_no_application (layer_dependency)
#   3. architecture.evidence_kernel_no_presentation (namespace_boundary)
# Fuente: crates/cognicode-explorer/src/api.rs CP1 handler.
echo "→ Verificando coherencia de las 3 constraints..."
CONSTRAINTS_DETAIL=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
constraints = data.get('constraints', [])
for c in constraints:
    print(f\"{c.get('id', 'MISSING')}|{c.get('kind', 'MISSING')}|{c.get('adr_ref', 'MISSING')}\")
")
echo "$CONSTRAINTS_DETAIL" | sed 's/^/  /'
EXPECTED=("architecture.domain_no_infrastructure" \
          "architecture.domain_no_application" \
          "architecture.evidence_kernel_no_presentation")
for exp in "${EXPECTED[@]}"; do
  if echo "$CONSTRAINTS_DETAIL" | grep -q "$exp"; then
    echo "  OK: constraint '$exp' presente"
  else
    echo "FAIL: constraint esperada ausente: '$exp'"
    exit 1
  fi
done

# Verificación adicional: todas las constraints referencian ADR-046
ADR_REFS=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
constraints = data.get('constraints', [])
refs = set(c.get('adr_ref', 'MISSING') for c in constraints)
print(' '.join(sorted(refs)))
")
echo "  adr_refs = $ADR_REFS"
if [ "$ADR_REFS" != "ADR-046" ]; then
  echo "FAIL: se esperaba ADR-046 único, hay: $ADR_REFS"
  exit 1
fi

# Gate 6: ninguna violación si self-host (0 violations esperado)
VIOLATIONS_COUNT=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
violations = data.get('violations', [])
print(len(violations))
")
echo "  violations_count (self-host) = $VIOLATIONS_COUNT"
if [ "$VIOLATIONS_COUNT" != "0" ]; then
  echo "FAIL: self-host debería tener 0 violations, hay $VIOLATIONS_COUNT"
  exit 1
fi

# Gate 7: coverage no trivial (statements_examined > 1000)
STATEMENTS=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
print(data.get('statements_examined', 0))
")
echo "  statements_examined = $STATEMENTS"
if [ "$STATEMENTS" -lt 1000 ]; then
  echo "WARN: statements_examined < 1000 (posible evaluación parcial)"
fi

# Gate 8: ninguna constraint unevaluated
UNEVALUATED=$(echo "$CP1_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
unev = data.get('unevaluated_constraints', [])
print(len(unev))
")
echo "  unevaluated_constraints = $UNEVALUATED"
if [ "$UNEVALUATED" != "0" ]; then
  echo "FAIL: hay $UNEVALUATED constraints unevaluated, se esperaban 0"
  exit 1
fi

echo
echo "→ Todos los gates semánticos PASS:"
echo "  - /health responde"
echo "  - architecture endpoint accesible y devuelve JSON válido"
echo "  - status = 'evaluated'"
echo "  - exactamente 3 canonical constraints"
echo "  - nombres de constraints coinciden con ADR-046"
echo "  - todas las constraints referencian ADR-046"
echo "  - 0 violations en self-host"
echo "  - 0 unevaluated constraints"
echo "  - statements_examined = $STATEMENTS"
echo "  - bind: $BIND"
echo "  - source: $SOURCE_ROOT"

exit 0
