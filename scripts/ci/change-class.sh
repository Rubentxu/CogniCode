#!/usr/bin/env bash
# change-class.sh — `docs` si el cambio es solo documentacion, `code` si no.
#
# No clasifica por su cuenta: pregunta a `scripts/ci/impact.py`, que es la
# unica definicion de que es documentacion, que es codigo y que puede
# cambiarlo todo. Este script existe para quien solo necesita la respuesta
# binaria (el primer paso de `docs-gate.pipeline.kts`). Falla cerrado: si
# impact.py no responde, `code`.
#
# Uso:
#   bash scripts/ci/change-class.sh [<sha>] [<base>]     # base: origin/main
#   bash scripts/ci/change-class.sh --files <f1> <f2>...

set -uo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [ "${1:-}" = "--files" ]; then
    shift
    tier="$(python3 "$REPO_ROOT/scripts/ci/impact.py" tier --files "$@" 2>/dev/null)" || tier=""
else
    tier="$(python3 "$REPO_ROOT/scripts/ci/impact.py" tier --sha "${1:-HEAD}" --base "${2:-origin/main}" 2>/dev/null)" || tier=""
fi
if [ "$tier" = "docs" ]; then echo docs; else echo code; fi
