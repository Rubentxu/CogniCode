#!/usr/bin/env bash
# scripts/ci/select-suites.sh
#
# CR-08 selector determinista de suites afectadas por un PR.
# Pinear contrato: dada una lista de paths modificados, devolver las
# suites que deberían correr en CI. Si la entrada es ambigua o vacía,
# fallback seguro = suite completa.
#
# Uso:
#   bash scripts/ci/select-suites.sh --paths "path1 path2 path3"
#   SELECT_PATHS="path1 path2" bash scripts/ci/select-suites.sh
#
# Output:
#   JSON en stdout: {"strategy": "changed|fallback", "reason": "...", "suites": [...]}
#   Exit 0 siempre.
#
# Estrategia:
#   - "core"     : cargo test -p cognicode-core
#   - "explorer" : cargo test -p cognicode-explorer --lib
#   - "mcp"      : cargo test -p cognicode-mcp --lib
#   - "cli"      : cargo test -p cognicode-cli --lib (no integration tests separados)
#   - "ladybug"  : cargo test -p cognicode-ladybug --lib
#   - "core_int" : cargo test -p cognicode-core --test <contractual>
#   - "merge"    : merge-gate completo (e2e + pineados)
#
# Reglas de mapeo (ordenadas):
#   1. Touch en `Cargo.toml` workspace, `Cargo.lock`, `.cargo/**`,
#      cualquier `crates/*/Cargo.toml`, `rust-toolchain*` -> fallback ALL.
#      (Un cambio en deps/registro afecta a TODOS los compiles).
#   2. Touch en `.github/workflows/**` -> fallback ALL.
#      (Los workflows son la pieza contractual de CI; un cambio puede
#      cambiar pines o steps y debe re-validarse todo).
#   3. Touch en `scripts/ci/**` -> fallback ALL.
#      (Lo nuevo es infraestructura de CI).
#   4. Touch en `crates/cognicode-core/**` -> ['core'].
#   5. Touch en `crates/cognicode-explorer/**` -> ['explorer', 'core']
#      (cognicode-explorer tiene codegen de arquitectura que recalibra
#       constraints en core; ambos deben validarse juntos).
#   6. Touch en `crates/cognicode-mcp/**` -> ['mcp', 'core'].
#      (mcp consume core; cambios rompen compat del contrato MCP).
#   7. Touch en `crates/cognicode-cli/**` -> ['cli'].
#   8. Touch en `crates/cognicode-ladybug/**` -> ['ladybug'].
#   9. Touch en `tests/**` o cualquier `*.rs` en root -> fallback ALL.
#   10. Touch en `docs/**` -> ['ladybug'] (cousin del vector e29/e30).
#       Conservative: docs no rompen binarios, pero el pupitre de
#       evidence si toca docs/architecture -> explorer+core.
#   11. Touch en `sandbox/**` -> [] (no se valida en PR-CI; vive en ci.yml local).
#   12. Paths vacíos o sólo whitespace -> fallback ALL (safe default).
#   13. Cualquier path desconocido -> fallback ALL.
#
# Test contractual:
#   crates/cognicode-cli/tests/qw08_crate_selector.rs
#
# Política L1.2: este selector NO se ejecuta directamente desde el job
# `test-pr` (no tenemos `git diff` natural en pull_request event); su
# existencia en scripts/ está documentada como pieza contractual del
# CR-08, y el test contractual pinea el mapeo. Un job que llame al
# selector vivirá en un cambio futuro (`dorny/paths-filter` o
# `tj-actions/changed-files`).
#
# Política de release: el selector NO modifica el release gate
# (`release.yml`); release siempre corre la suite completa. Aquí solo
# optimizamos PR-CI.

set -euo pipefail

# ---------- args ----------
PATHS=""
while [ $# -gt 0 ]; do
  case "$1" in
    --paths)
      shift
      if [ $# -gt 0 ]; then
        PATHS="$1"
        shift
      fi
      ;;
    --paths=*)
      PATHS="${1#--paths=}"
      shift
      ;;
    -h|--help)
      cat <<EOF
select-suites.sh — CR-08 selector determinista
Uso:
  --paths "a b c"     paths modificados (o env SELECT_PATHS)
                      salida JSON: {"strategy":..., "reason":..., "suites":[...]}
EOF
      exit 0
      ;;
    *)
      # backward compat: positional first arg
      if [ -z "$PATHS" ]; then
        PATHS="$1"
      fi
      shift
      ;;
  esac
done

# env override
if [ -z "$PATHS" ] && [ -n "${SELECT_PATHS:-}" ]; then
  PATHS="$SELECT_PATHS"
fi

# ---------- helpers ----------
# Solo imprime el primer campo de un match path (legacy). En realidad
# no hace falta nada exótico: bash builtin [[ =~ ]] es suficiente.
emit_json() {
  local strategy="$1"
  local reason="$2"
  shift 2
  # remaining args: suites
  local joined=""
  if [ $# -gt 0 ]; then
    local IFS=','
    joined="$*"
  fi
  if [ -z "$joined" ]; then
    printf '{"strategy":"%s","reason":"%s","suites":[]}\n' \
      "$strategy" "$reason"
  else
    printf '{"strategy":"%s","reason":"%s","suites":[%s]}\n' \
      "$strategy" "$reason" "$joined"
  fi
}

fallback() {
  local reason="$1"
  emit_json "fallback" "$reason" \
    "core" "explorer" "mcp" "cli" "ladybug" "merge"
}

noop() {
  # estrategia "noop" = path no impacta PR-CI (sandbox/meta).
  # Si ya hay razón noop acumulada, noop_ se concatena con `,`.
  # Si ya hay razón válida (e.g. core_changed), la preservamos
  # como "primary" y solo marcamos el sentinel al final.
  if [ -z "$reason" ]; then
    reason="noop_$1"
  elif [ "${reason#noop_}" = "$reason" ]; then
    # reason empieza a tener algo distinto a noop_; anclar noop
    # como sufijo
    reason="mixed:$reason+$1"
  else
    # reason ya es un conjunto noop_x+noop_y; concatenar
    reason="$reason+$1"
  fi
}

# Sentinel: si reason empieza por 'noop_' significa que en algún
# momento vimos un path sandbox/meta. El caller lo consultará abajo.

# ---------- main ----------
# Empty / whitespace-only -> fallback seguro
trimmed="$(printf '%s' "$PATHS" | tr -d '[:space:]')"
if [ -z "$trimmed" ]; then
  fallback "empty_paths"
  exit 0
fi

# Walk each path; first rule that matches wins (with union de suites).
suites=""
needs_all="false"
reason=""

for p in $PATHS; do
  case "$p" in
    # Regla 1: cambios en deps/registro => ALL
    Cargo.toml|Cargo.lock|.cargo/*|.cargo|Cargo.toml.new|Cargo.lock.new|rust-toolchain|rust-toolchain.toml|rust-toolchain.*)
      needs_all="true"
      reason="workspace_manifest_changed($p)"
      ;;
    crates/*/Cargo.toml|*/Cargo.toml)
      needs_all="true"
      reason="crate_manifest_changed($p)"
      ;;
    # Regla 2: workflows => ALL
    .github/workflows/*|.github/workflows|.github/*)
      needs_all="true"
      reason="workflow_changed($p)"
      ;;
    # Regla 3: scripts CI => ALL
    scripts/ci/*|scripts/ci|scripts/ci/*.sh)
      needs_all="true"
      reason="ci_script_changed($p)"
      ;;
    # Regla 4: core
    crates/cognicode-core/*|crates/cognicode-core)
      case " $suites " in
        *" core "*) ;;
        *) suites="$suites core" ;;
      esac
      [ -z "$reason" ] && reason="core_changed($p)"
      ;;
    # Regla 5: explorer (también dispara core por codegen arquitectura)
    crates/cognicode-explorer/*|crates/cognicode-explorer)
      case " $suites " in
        *" explorer "*) ;;
        *) suites="$suites explorer" ;;
      esac
      case " $suites " in
        *" core "*) ;;
        *) suites="$suites core" ;;
      esac
      [ -z "$reason" ] && reason="explorer_changed($p)"
      ;;
    # Regla 6: mcp
    crates/cognicode-mcp/*|crates/cognicode-mcp)
      case " $suites " in
        *" mcp "*) ;;
        *) suites="$suites mcp" ;;
      esac
      case " $suites " in
        *" core "*) ;;
        *) suites="$suites core" ;;
      esac
      [ -z "$reason" ] && reason="mcp_changed($p)"
      ;;
    # Regla 7: cli
    crates/cognicode-cli/*|crates/cognicode-cli)
      case " $suites " in
        *" cli "*) ;;
        *) suites="$suites cli" ;;
      esac
      [ -z "$reason" ] && reason="cli_changed($p)"
      ;;
    # Regla 8: ladybug
    crates/cognicode-ladybug/*|crates/cognicode-ladybug)
      case " $suites " in
        *" ladybug "*) ;;
        *) suites="$suites ladybug" ;;
      esac
      [ -z "$reason" ] && reason="ladybug_changed($p)"
      ;;
    # Regla 9: tests dir => ALL
    tests/*|tests|tests.rs)
      needs_all="true"
      reason="test_root_changed($p)"
      ;;
    # Regla 10: docs => ['ladybug']
    docs/architecture/*|docs/architecture|docs/roadmap/*|docs/roadmap|docs/prf/*|docs/prf|docs/*|docs)
      case " $suites " in
        *" ladybug "*) ;;
        *) suites="$suites ladybug" ;;
      esac
      [ -z "$reason" ] && reason="docs_changed($p)"
      ;;
    # Regla 11: sandbox / compat / metadata file => marca noop
    # sandbox vive en ci.yml local-only; no impacta PR-CI.
    # README/LICENSE/.gitignore/etc. son metadata que no tocan
    # código ni pines contractuales. Acumulamos como "noop" para
    # emitir al final si no hay suites en juego.
    sandbox/*|sandbox|.compat/*|.compat|README.md|LICENSE|LICENSE.*|.gitignore|.gitattributes|.editorconfig|.markdownlint.yaml|.markdownlintignore)
      noop "doc_or_sandbox_meta_changed($p)"
      ;;
    # Regla 13: root *.rs (NO contiene /) => ALL.
    # Los *.rs con slash ya habrían hecho match con reglas
    # específicas (4-8). Este es el caso "tomato.rs" suelto en
    # root sin crate declarado.
    *.rs)
      case "$p" in
        */*) needs_all="true"; reason="unmatched_slashed_rs($p)" ;;
        *)   needs_all="true"; reason="root_dotrs_changed($p)" ;;
      esac
      ;;
    # catch-all: unknown pattern => fallback
    *)
      needs_all="true"
      reason="unknown_path($p)"
      ;;
  esac
done

if [ "$needs_all" = "true" ]; then
  fallback "$reason"
  exit 0
fi

# Trim leading space, build comma-joined list
suites="${suites# }"

# Si tras todo el loop tenemos una razón noop y NO se acumuló
# suite alguna, es un PR-CI noop (no correr nada).
if [ -z "$suites" ] && [ -n "$reason" ] && [ "${reason#noop_}" != "$reason" ]; then
  emit_json "noop" "${reason#noop_}"
  exit 0
fi

if [ -z "$suites" ]; then
  if [ -n "$reason" ]; then
    fallback "$reason"
  else
    fallback "no_rule_matched"
  fi
  exit 0
fi

# Strategy "changed" emite lista en formato JSON array.
# Usar una lista de strings separada por comas.
arr="$(printf '%s' "$suites" | tr ' ' ',')"
printf '{"strategy":"changed","reason":"%s","suites":["%s"]}\n' \
  "$reason" "${arr//,/\",\"}"
