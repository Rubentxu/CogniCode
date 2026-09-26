#!/usr/bin/env bash
# scripts/ci/preflight-clean-clone.sh
#
# QW-04 preflight: certifica que el repositorio puede compilarse desde
# clean clone + clean tree, sin depender de archivos locales no versionados.
#
# Uso:
#   bash scripts/ci/preflight-clean-clone.sh [SHA]
#     SHA: SHA a certificar (default: HEAD)
#
# Exit codes:
#   0  — preflight PASS (clean clone compila + tests verde)
#   1  — preflight FAIL (algún check falló)
#   2  — entorno no apto (git/cargo no disponible, sin red, etc.)
#
# Estrategia:
#   1. Clona el repo en tempdir (sparse para acelerar)
#   2. Checkout al SHA indicado
#   3. Verifica que el tree está limpio
#   4. Ejecuta check-bin-tracking.sh (QW-03) sobre el clone
#   5. Ejecuta cargo check --workspace --all-targets
#   6. Ejecuta cargo test --workspace (sin --include-ignored)
#   7. Compara battery con el baseline (control_plane canonical state)
#   8. Emite recibo con SHA, hash del tree, resultados
#
# Output:
#   - Log completo en /tmp/preflight-clean-clone.<timestamp>.log
#   - Recibo JSON en /tmp/preflight-receipt-<sha>.json
#
# Costo esperado: 8-15 minutos en CI runner estándar (cold cache).
# En warm cache: 3-5 minutos.

set -euo pipefail

# --- Configuración -----------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

TARGET_SHA="${1:-HEAD}"
if [ "$TARGET_SHA" = "HEAD" ]; then
  TARGET_SHA=$(git rev-parse HEAD)
fi

TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
LOG_FILE="/tmp/preflight-clean-clone.${TIMESTAMP}.log"
RECEIPT_FILE="/tmp/preflight-receipt-${TARGET_SHA:0:12}.json"

# Baseline de tests esperado (controlado por el operador).
# Si cambia, requiere re-baseline explícito.
# 5565 = workspace cargo test con debug bin.
# 5572 = workspace cargo test con release bin (los 7 tests
#        prf_cli_01_exhaustive_uat requieren bin release pre-existente).
# Por defecto usamos el baseline más estricto (5572) porque el preflight
# ejecuta cargo build --release antes de los tests.
BASELINE_PASSED="${PREFLIGHT_BASELINE_PASSED:-5572}"
BASELINE_FAILED="${PREFLIGHT_BASELINE_FAILED:-0}"
BASELINE_IGNORED="${PREFLIGHT_BASELINE_IGNORED:-30}"

# --- Helpers -----------------------------------------------------------------

log() {
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG_FILE"
}

fail() {
  log "FAIL: $*"
  if [ -n "${RECEIPT_FILE:-}" ]; then
    cat > "$RECEIPT_FILE" << EOF
{
  "sha": "$TARGET_SHA",
  "timestamp": "$TIMESTAMP",
  "result": "FAIL",
  "stage_failed": "$*",
  "log_file": "$LOG_FILE"
}
EOF
  fi
  exit 1
}

require_tool() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "herramienta requerida no disponible: $1"
  fi
}

# --- Pre-chequeos ------------------------------------------------------------

require_tool git
require_tool cargo
require_tool perl
require_tool python3

# CR-00c: neutralizar el gitignore GLOBAL del operador para que ningún archivo
# requerido por el gate pueda existir o desaparecer silenciosamente según
# la configuración del usuario. Solo afecta a esta sesión (export, no se
# persiste). Si el operador ha configurado `core.excludesfile` o un archivo
# `~/.config/git/ignore`, el preflight lo ignora completamente.
#
# Verificación rápida: `git config --show-origin --get core.excludesfile`
# apuntaría a un path del operador; aquí lo anulamos.
if [ -n "${GIT_CONFIG_GLOBAL:-}" ] && [ "$GIT_CONFIG_GLOBAL" != "/dev/null" ]; then
  log "AVISO: GIT_CONFIG_GLOBAL pre-existente no anulado ($GIT_CONFIG_GLOBAL); el gate puede estar sujeto a gitignore externo del operador"
fi
export GIT_CONFIG_GLOBAL=/dev/null
log "GIT_CONFIG_GLOBAL=/dev/null (gitignore global del operador neutralizado)"

log "Preflight clean-clone preflight (QW-04, 2026-09-26)"
log "SHA a certificar: $TARGET_SHA"
log "Baseline: passed=$BASELINE_PASSED failed=$BASELINE_FAILED ignored=$BASELINE_IGNORED"

# Limpiar log file
: > "$LOG_FILE"

# --- Stage 1: clone ----------------------------------------------------------

WORK_DIR="$(mktemp -d -t cognicode-preflight-XXXXXX)"
trap 'rm -rf "$WORK_DIR"' EXIT

log "Stage 1/7: clone a $WORK_DIR (sparse, blobs only)"

if ! git clone --no-tags --depth 1 --filter=blob:none \
     --branch "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo main)" \
     "$REPO_ROOT" "$WORK_DIR/clone" 2>>"$LOG_FILE"; then
  fail "git clone falló (ver $LOG_FILE)"
fi

cd "$WORK_DIR/clone"
git fetch --depth 1 origin "$TARGET_SHA" 2>>"$LOG_FILE" || true
if ! git checkout "$TARGET_SHA" 2>>"$LOG_FILE"; then
  fail "git checkout $TARGET_SHA falló"
fi
log "  → clone OK en SHA $(git rev-parse HEAD)"

# --- Stage 2: tree clean ------------------------------------------------------

log "Stage 2/7: verificación de tree limpio"

if ! git diff --quiet HEAD 2>/dev/null; then
  fail "tree no limpio (cambios sin commit)"
fi

if [ -n "$(git status --porcelain)" ]; then
  fail "tree no limpio (archivos sin trackear o staged)"
fi
log "  → tree limpio OK"

# --- Stage 3: QW-03 guard -----------------------------------------------------

log "Stage 3/7: QW-03 guard (bin source tracking)"

if ! bash "$WORK_DIR/clone/scripts/ci/check-bin-tracking.sh" >>"$LOG_FILE" 2>&1; then
  fail "QW-03 guard falló (bin source untracked o no versionado)"
fi
log "  → QW-03 guard OK"

# --- Stage 4: cargo check -----------------------------------------------------

log "Stage 4/7: cargo check --workspace --all-targets"

# IMPORTANTE: este target_dir debe estar DENTRO del clon (`$WORK_DIR/clone/target`).
# El test `prf_cli_01_exhaustive_uat` calcula el path del binario con
# `parent().parent().join("target/release/cognicode")` desde `crates/cognicode-cli/`,
# asumiendo el `target/` del repo_root del clon. Si redirigimos fuera del clon
# (p.ej. `$WORK_DIR/target`), los 7 UAT fallan con `NotFound` aunque el
# build sea OK. El `trap rm -rf "$WORK_DIR"` borra el clon al EXIT, así que
# los artefactos no contaminan el sistema. Trade-off: ~2-3 min extra de cold
# build si se re-ejecuta el preflight; aceptable a cambio de reproducibilidad
# real del contrato de los tests.
CARGO_TARGET_DIR="$WORK_DIR/clone/target"
export CARGO_TARGET_DIR

if ! cargo check --workspace --all-targets --locked 2>>"$LOG_FILE"; then
  fail "cargo check falló"
fi
log "  → cargo check OK"

# --- Stage 4b: cargo build --release --bins ---------------------------------
# Algunos tests de integración (notablemente prf_cli_01_exhaustive_uat)
# buscan binarios en `target/release/<name>` (no debug). Sin este
# build, fallan con "No such file or directory" en clean clone.
# El preflight debe ejecutar cargo build --release --bins antes de los
# tests para que la batería completa sea reproducible desde Git.

log "Stage 4b/7: cargo build --release --workspace --bins"

if ! cargo build --release --workspace --bins --locked 2>>"$LOG_FILE"; then
  log "  → tail del build output:"
  tail -30 "$LOG_FILE"
  fail "cargo build --release --bins falló"
fi
log "  → cargo build --release --bins OK"

# --- Stage 5: cargo test (sin --include-ignored) -----------------------------

log "Stage 5/7: cargo test --workspace (sin --include-ignored)"

TEST_OUTPUT_FILE="$WORK_DIR/test-output.txt"
if ! cargo test --workspace --locked -- \
     --skip ignored >>"$TEST_OUTPUT_FILE" 2>&1; then
  log "  → tail del test output:"
  tail -30 "$TEST_OUTPUT_FILE" | tee -a "$LOG_FILE"
  fail "cargo test falló"
fi

# Parsear resultados.
TEST_RESULT=$(grep -E "^test result:" "$TEST_OUTPUT_FILE" | \
  awk '{p+=$4; f+=$6; i+=$8} END {printf "passed=%d failed=%d ignored=%d", p, f, i}')
log "  → cargo test resultado: $TEST_RESULT"

OBSERVED_PASSED=$(echo "$TEST_RESULT" | sed -n 's/.*passed=\([0-9]*\).*/\1/p')
OBSERVED_FAILED=$(echo "$TEST_RESULT" | sed -n 's/.*failed=\([0-9]*\).*/\1/p')
OBSERVED_IGNORED=$(echo "$TEST_RESULT" | sed -n 's/.*ignored=\([0-9]*\).*/\1/p')

# --- Stage 6: comparación con baseline ---------------------------------------

log "Stage 6/7: comparación con baseline"

TOLERANCE="${PREFLIGHT_TOLERANCE:-2}"  # ±2 tests por churn legítimo

PASSED_DIFF=$((OBSERVED_PASSED - BASELINE_PASSED))
FAILED_DIFF=$((OBSERVED_FAILED - BASELINE_FAILED))
IGNORED_DIFF=$((OBSERVED_IGNORED - BASELINE_IGNORED))

log "  baseline: passed=$BASELINE_PASSED failed=$BASELINE_FAILED ignored=$BASELINE_IGNORED"
log "  observed: passed=$OBSERVED_PASSED failed=$OBSERVED_FAILED ignored=$OBSERVED_IGNORED"
log "  diff:     passed=$PASSED_DIFF failed=$FAILED_DIFF ignored=$IGNORED_DIFF (tolerancia ±$TOLERANCE)"

if [ "${PASSED_DIFF#-}" -gt "$TOLERANCE" ]; then
  fail "regresión de tests passed: delta=$PASSED_DIFF > tolerancia=$TOLERANCE"
fi

if [ "$FAILED_DIFF" -gt 0 ]; then
  fail "regresión de tests failed: delta=$FAILED_DIFF"
fi

if [ "$OBSERVED_FAILED" -gt 0 ]; then
  fail "tests failed en clean clone: $OBSERVED_FAILED"
fi

log "  → battery dentro de tolerancia"

# --- Stage 7: recibo ---------------------------------------------------------

log "Stage 7/7: emisión de recibo"

TREE_HASH=$(git rev-parse "HEAD^{tree}")
cat > "$RECEIPT_FILE" << EOF
{
  "sha": "$TARGET_SHA",
  "tree_hash": "$TREE_HASH",
  "timestamp": "$TIMESTAMP",
  "result": "PASS",
  "stages": {
    "clone": "OK",
    "tree_clean": "OK",
    "qw03_guard": "OK",
    "cargo_check": "OK",
    "cargo_test": "OK",
    "baseline_comparison": "OK"
  },
  "baseline": {
    "passed": $BASELINE_PASSED,
    "failed": $BASELINE_FAILED,
    "ignored": $BASELINE_IGNORED
  },
  "observed": {
    "passed": $OBSERVED_PASSED,
    "failed": $OBSERVED_FAILED,
    "ignored": $OBSERVED_IGNORED
  },
  "delta": {
    "passed": $PASSED_DIFF,
    "failed": $FAILED_DIFF,
    "ignored": $IGNORED_DIFF,
    "tolerance": $TOLERANCE
  },
  "log_file": "$LOG_FILE"
}
EOF

log "Recibo emitido: $RECEIPT_FILE"
log "Log completo: $LOG_FILE"
log "PREFLIGHT PASS"

exit 0
