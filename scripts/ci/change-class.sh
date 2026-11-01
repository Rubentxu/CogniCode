#!/usr/bin/env bash
# change-class.sh — dice si un cambio es solo documentacion o toca producto.
#
# Imprime `docs` o `code` y sale con 0. `publish-merge-gate.sh` lo usa para
# elegir el carril que respalda el estado `merge-gate`: un cambio `docs` corre
# `docs-gate.pipeline.kts`, cualquier otro corre `merge-gate.pipeline.kts`.
#
# Falla cerrado: ante la duda, `code`. Es `docs` solo si TODOS los ficheros
# cambiados estan en la lista de abajo y la lista no esta vacia. Un diff vacio,
# un error de git o un fichero que no encaja dan `code`, y `code` es el gate
# completo de siempre.
#
# Por que un `.md` no necesita compilar: ningun crate hace `include_str!` de un
# `.md` (los `include_str!` del workspace son YAML de `bundled/` y
# `product/tools.json`, y ninguno de los dos esta en la lista). Los tests que
# leen documentacion la leen en tiempo de ejecucion, y el carril de docs los
# ejecuta.
#
# Fuera de la lista aunque sean Markdown: todo lo que vive en `crates/`,
# `apps/`, `skills/`, `bundles/`, `product/`, `integrations/` o `sandbox/`,
# porque ahi un `.md` puede viajar dentro de un artefacto publicado.
#
# Uso:
#   bash scripts/ci/change-class.sh [<sha>] [<base>]     # base: origin/main
#   bash scripts/ci/change-class.sh --files <f1> <f2>...  # clasifica una lista

set -uo pipefail

is_docs_path() {
    case "$1" in
        docs/*|odd/*|evidence/*|plans/*|sdd/*|sddk/*) return 0 ;;
        goal-compass.json) return 0 ;;
        */*) return 1 ;;   # cualquier otro directorio: producto
        *.md) return 0 ;;  # Markdown en la raiz
        *) return 1 ;;
    esac
}

classify() {
    local n=0 f
    while IFS= read -r f; do
        [ -z "$f" ] && continue
        n=$((n + 1))
        if ! is_docs_path "$f"; then
            echo code
            return
        fi
    done
    if [ "$n" -eq 0 ]; then
        echo code
    else
        echo docs
    fi
}

if [ "${1:-}" = "--files" ]; then
    shift
    printf '%s\n' "$@" | classify
    exit 0
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT" || { echo code; exit 0; }

sha="${1:-HEAD}"
base="${2:-origin/main}"

merge_base="$(git merge-base "$base" "$sha" 2>/dev/null)" || { echo code; exit 0; }
files="$(git diff --name-only "$merge_base" "$sha" 2>/dev/null)" || { echo code; exit 0; }
printf '%s\n' "$files" | classify
