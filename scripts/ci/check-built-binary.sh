#!/usr/bin/env bash
# Se ha construido ESTE binario desde ESTE arbol?
#
# MEDIDO 2026-10-03. La lane de v0.101.5, ya con aarch64 entero en verde, cayo
# en `skill-bundles` con
#
#     error: unrecognized subcommand 'skills'
#
# La stage lo habia pedido correctamente y el subcomando existe en el arbol
# (7d074fef es ancestro del tag). El problema es de donde salio el binario:
#
#     $ ls --time-style=full-iso "$TARGET_DIR/release/cognicode-release"
#     -rwxr-xr-x 2 rubentxu rubentxu 1494424 2026-10-03 20:13:00
#
# La lane corrio a las 21:30. El binario es de las 20:13, de un checkout
# distinto, y `cargo build --release --bin cognicode-release` no lo toco.
#
# Por que no: `~/.cargo/config.toml` de esta maquina fija
# `build.target-dir = /var/home/rubentxu/cargo-targets`, compartido por varios
# checkouts y agentes concurrentes. Cargo decide si hay que reconstruir
# comparando mtimes, y los ficheros de ESTE checkout son mas antiguos —los
# escribio git al crearlo— que un binario que otro arbol construyo despues.
# Freshness comparada entre dos arboles que no comparten historial: no dice nada.
#
# MEDIDO TAMBIEN, y este es el motivo de la segunda version: la guarda original
# comparaba el binario contra TODOS los `.rs` de `crates/`, y eso tambien es
# falso. MEDIDO 2026-10-03, en la lane de v0.101.5: `ide.rs` —que no entra en
# `cognicode-release`— hacia fallar la stage con un binario perfectamente
# fresco, y el mensaje senalaba un fichero que no tenia nada que ver. Un guard
# que rechaza lo que no debe entrena a ignorar su veredicto.
#
# Y MEDIDO 2026-10-24, en la linea v0.101.30, al intentar corregirlo: la lista
# de fuentes que construye `cognicode-release` NO son cuatro ficheros del CLI.
# Cargo dice que son 415, repartidos en cuatro crates:
#
#     368  crates/cognicode-core
#      32  crates/cognicode-graph-algos
#      11  crates/cognicode-cli
#       3  crates/cognicode-macros
#
# Una lista escrita a mano con los cuatro ficheros de `src/bin/release.rs` y sus
# modulos `#[path]` habria sido verde en falso: un cambio en cualquiera de los
# 368 ficheros de `cognicode-core` deja el binario viejo de verdad, y esa guarda
# no lo habria visto. Una lista a mano deriva; y deriva hacia el lado que no
# falla.
#
# POR QUE ESTO USA EL DEP-INFO DE CARGO
#
# Rustc escribe, junto a cada binario, un fichero `.d` con la lista exacta de
# ficheros que compilo. Cargo lo escribe siempre que construye el binario, en
# el mismo directorio, con la misma operacion. Es la respuesta de cargo a la
# pregunta "que construye esto", y por eso no puede quedarse corta ni larga:
#
#     $TARGET_DIR/release/cognicode-release.d
#
# Ese fichero ademas nombra el ARBOL desde el que se construyo, porque sus
# rutas son absolutas. Asi que la pregunta original —"¿este binario es de este
# arbol?"— tiene respuesta directa, sin proxy de mtimes y sin lista a mano.
#
# Lo que este script no hace, y conviene decir: no sabe por que cargo no
# reconstruyo. Lo que sabe es si el binario que hay es de este arbol y si esta
# al dia respecto a lo que lo construye. Con un `CARGO_TARGET_DIR` propio por
# lane la respuesta es que si, porque cargo no reutiliza entre arboles. Ese es
# el arreglo de maquina; este es el que avisa cuando no se ha hecho.
#
# Uso: check-built-binary.sh <binario> [raiz-del-arbol]
# Exit: 0 si el binario es de este arbol y no es anterior a sus fuentes; 1 si no.
set -uo pipefail

binary="${1:?usage: check-built-binary.sh <binario> [repo-root]}"
root="${2:-$(cd "$(dirname "$0")/../.." && pwd)}"

if [ ! -f "$binary" ]; then
    echo "FAIL: no existe el binario $binary." >&2
    echo "  La stage que lo construye corre antes de esta; fallo o no se ejecuto." >&2
    exit 1
fi

# El dep-info va junto al binario, con el mismo nombre y extension `.d`.
depfile="$binary.d"

# El parseo va en python porque normalizar rutas con `..` y comparar prefijos
# en shell es donde se cuelan los errores. Aqui no se decide nada: solo se
# leen las rutas y se contestan tres preguntas.
verdict=$(python3 - "$depfile" "$root" "$binary" <<'PY'
import os
import sys

depfile, root, binary = sys.argv[1], os.path.abspath(sys.argv[2]), sys.argv[3]

if not os.path.isfile(depfile):
    print("STATUS=no-depinfo")
    print("DEPFILE=" + depfile)
    sys.exit(0)

try:
    raw = open(depfile, encoding="utf-8", errors="replace").read()
except OSError as exc:
    print("STATUS=unreadable")
    print("DETAIL=" + str(exc))
    sys.exit(0)

# Formato makefile: `objetivo: dep1 dep2 ...`, con continuaciones de linea.
raw = raw.replace("\\\n", " ")
head, sep, rest = raw.partition(":")
if not sep:
    print("STATUS=unparsable")
    sys.exit(0)

deps = [d for d in rest.split() if d]

under, outside, gitmeta = [], [], []
for dep in deps:
    norm = os.path.normpath(dep)
    # Las entradas de `.git/` son metadatos del checkout, no fuentes: en un
    # worktree apuntan al `.git` del repo principal, no al de este arbol, y
    # mezclarlas haria que la comparacion de arboles no significara nada.
    if os.sep + ".git" + os.sep in norm or norm.endswith(os.sep + ".git"):
        gitmeta.append(norm)
        continue
    if not os.path.isfile(norm):
        # Un directorio en el dep-info no es una fuente: rustc lo lista para
        # descubrir modulos, y su mtime no significa nada para el binario.
        continue
    if norm == os.path.abspath(binary):
        continue
    if norm == root or norm.startswith(root + os.sep):
        under.append(norm)
    else:
        outside.append(norm)

if not under:
    # Ninguna fuente de este arbol. El binario lo construyo otro checkout.
    trees = {}
    for dep in deps:
        norm = os.path.normpath(dep)
        if os.sep + ".git" + os.sep in norm:
            continue
        if "/crates/" in norm:
            t = norm.split("/crates/")[0]
            trees[t] = trees.get(t, 0) + 1
    other = max(trees.items(), key=lambda kv: kv[1])[0] if trees else "otro arbol"
    print("STATUS=other-tree")
    print("OTHER=" + other)
    print("SOURCES=" + str(len(deps)))
    sys.exit(0)

if outside:
    print("STATUS=mixed")
    print("OTHER=" + outside[0])
    print("SOURCES=" + str(len(deps)))
    sys.exit(0)

# Todas las fuentes son de este arbol. Ahora la frescura de verdad: el binario
# no puede ser anterior a la fuente mas nueva que lo construye.
newest, newest_path = 0, ""
for src in under:
    try:
        m = os.stat(src).st_mtime
    except OSError:
        continue
    if m > newest:
        newest, newest_path = m, src

binary_mtime = os.stat(binary).st_mtime
print("STATUS=" + ("stale" if newest > binary_mtime else "fresh"))
print("COUNT=" + str(len(under)))
print("NEWEST=" + newest_path)
print("NEWEST_REL=" + os.path.relpath(newest_path, root))
print("BINARY_MTIME=" + str(int(binary_mtime)))
print("NEWEST_MTIME=" + str(int(newest)))
PY
)

status=$(printf '%s\n' "$verdict" | sed -n 's/^STATUS=//p')
detail=$(printf '%s\n' "$verdict" | sed -n 's/^DETAIL=//p')
other=$(printf '%s\n' "$verdict" | sed -n 's/^OTHER=//p')
count=$(printf '%s\n' "$verdict" | sed -n 's/^COUNT=//p')
newest_rel=$(printf '%s\n' "$verdict" | sed -n 's/^NEWEST_REL=//p')
sources=$(printf '%s\n' "$verdict" | sed -n 's/^SOURCES=//p')

case "$status" in
fresh)
    echo "  ok: $binary es de este arbol y no es anterior a ninguna de sus $count fuente(s)"
    exit 0
    ;;

no-depinfo)
    echo "FAIL: no hay dep-info de cargo para este binario." >&2
    echo "  Se esperaba $(printf '%s' "$depfile").d" >&2
    echo "  Sin el, no se puede afirmar nada: un binario sin dep-info no lo" >&2
    echo "  construyo cargo desde aqui, o el directorio de artefactos se limpio" >&2
    echo "  y lo que hay es una copia." >&2
    exit 1
    ;;

unparsable)
    echo "FAIL: el dep-info de cargo no se puede leer como lista de dependencias." >&2
    echo "  $depfile" >&2
    exit 1
    ;;

unreadable)
    echo "FAIL: no se pudo leer el dep-info de cargo: $detail" >&2
    echo "  $depfile" >&2
    exit 1
    ;;

other-tree)
    echo "FAIL: $binary NO se construyo desde este arbol." >&2
    echo "  arbol del binario : $other" >&2
    echo "  arbol de la lane   : $root" >&2
    echo "  fuentes           : $sources" >&2
    echo >&2
    echo "  Cargo decide reconstruir comparando mtimes, y el directorio de" >&2
    echo "  artefactos de esta maquina lo comparten varios checkouts. Un binario" >&2
    echo "  construido por otro checkout puede ser mas nuevo que las fuentes de" >&2
    echo "  este, y entonces cargo lo da por fresco y esta lane ejecuta una" >&2
    echo "  herramienta que no salio de este arbol." >&2
    echo >&2
    echo "  Que hacer: darle a la lane su propio CARGO_TARGET_DIR, para que las" >&2
    echo "  huellas de cargo sean de un arbol solo." >&2
    exit 1
    ;;

mixed)
    echo "FAIL: el dep-info mezcla fuentes de dos arboles." >&2
    echo "  fuente de otro arbol: $other" >&2
    echo "  arbol de la lane     : $root" >&2
    echo >&2
    echo "  Eso significa que el binario se construyo con ficheros de dos" >&2
    echo "  checkouts. No se puede afirmar de que arbol es." >&2
    exit 1
    ;;

stale)
    echo "FAIL: $binary es mas antiguo que una fuente que lo construye." >&2
    echo "  fuente   : $newest_rel" >&2
    echo "  binario  : $(date -r "$binary" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo '?')" >&2
    echo >&2
    echo "  Que hacer: reconstruir el binario desde este arbol." >&2
    exit 1
    ;;

*)
    echo "FAIL: veredicto desconocido de la guarda: '$status'" >&2
    exit 1
    ;;
esac
