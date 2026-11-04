#!/usr/bin/env bash
# docs-readers.sh — los tests de Rust que leen la documentacion del repositorio.
#
# Imprime una linea por cada test de integracion que hay que ejecutar cuando un
# cambio solo toca documentacion. `<crate> <target>` nombra un binario entero;
# `<crate> <target> <modulo>` nombra un modulo dentro de un binario consolidado
# (`tests/<target>/<modulo>.rs`), que se ejecuta con ese nombre como filtro:
#
#   1. Los que abren documentos del repositorio por ruta (`repo_root().join(…)`
#      sobre `docs/`, `ROADMAP`, `ACTION-REGISTER`…). Lista fija de abajo.
#   2. Ademas, cualquier fichero de `crates/*/tests/*.rs` que nombre uno de los
#      ficheros cambiados por su nombre (`JOURNAL.md`, `16-ACTION-REGISTER.md`…).
#      Esto cubre al lector que la lista fija no conozca todavia.
#
# Uso: bash scripts/ci/docs-readers.sh [<sha>] [<base>]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

sha="${1:-HEAD}"
base="${2:-origin/main}"

STATIC_READERS=(
    "cognicode-cli it advisory_ignore_backing_contract"
    "cognicode-cli it action_register_identity_contract"
    "cognicode-cli it backtick_provenance_contract"
    "cognicode-cli it gitignore_negation_contract"
    "cognicode-core it b6_w5_distribution_certification"
    "cognicode-core it b6_w6_architectural_close"
    "cognicode-core it roadmap_claims_contract"
    "cognicode-core it w0_post_pruning_orthodoxy"
)

{
    printf '%s\n' "${STATIC_READERS[@]}"
    merge_base="$(git merge-base "$base" "$sha")"
    git diff --name-only "$merge_base" "$sha" | while IFS= read -r changed; do
        [ -z "$changed" ] && continue
        name="$(basename "$changed")"
        # Un nombre generico (README.md) aparece en cientos de fixtures que
        # escriben su propio README en un directorio temporal; no lee el del repo.
        case "$name" in README.md|README.es.md) continue ;; esac
        grep -lF -- "$name" crates/*/tests/*.rs crates/*/tests/*/*.rs 2>/dev/null || true
    done | sed -nE -e 's#^crates/([^/]+)/tests/([^/]+)\.rs$#\1 \2#p' \
                   -e 's#^crates/([^/]+)/tests/([^/]+)/([^/]+)\.rs$#\1 \2 \3#p'
} | while read -r crate target module; do
    # Only a directory with a `main.rs` is a binary of its own; `common/`
    # holds modules the binaries share, and `main` is the root, not a reader.
    if [ -n "$module" ]; then
        [ -f "crates/$crate/tests/$target/main.rs" ] || continue
        [ "$module" = "main" ] && continue
    fi
    echo "$crate $target${module:+ $module}"
done | grep -v '^spike-' | sort -u
