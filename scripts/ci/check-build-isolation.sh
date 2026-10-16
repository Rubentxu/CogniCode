#!/usr/bin/env bash
# check-build-isolation.sh — refuse to run the gate when its verdict would not be attributable.
#
# MEDIDO 2026-10-15. Seis checkouts de este repositorio — el principal y cinco
# worktrees — compilan al mismo `/var/home/rubentxu/cargo-targets/`, porque
# `build.target-dir` está en la config global de cargo y no por repositorio.
# Cargo decide frescura por mtime, y **un mtime no dice de qué checkout salió un
# binario**. La consecuencia se vio dos veces en el mismo día, y en las dos
# direcciones:
#
#   * Un test paniqueó en `core_gate_coverage_contract.rs:900` nombrando
#     `the_rustdoc_gate_runs_in_the_merge_authority`, que **no existe en este
#     árbol**: el fichero en disco tiene 861 líneas y contiene la versión
#     relajada. El binario era de otro checkout.
#
#   * `a014_capabilities_json_carries_cli_version_and_source_commit` falló con
#     `source_commit (d5082848…) must match git rev-parse HEAD (d73e7b08…)`.
#     `d73e7b08` es el HEAD de `worktree/test-harness`.
#
# Las dos son **fallos de harness, no de producto**, y las dos se diagnostican
# rápido solo porque el mensaje traía la prueba. Un gate que no puede atribuir
# su veredicto tiene dos salidas honestas: negarse, o declarar que no lo puede
# atribuir. Este script es la primera; el recibo de la corrida es la segunda.
#
# POR QUÉ FALLA Y NO AVISA
# ------------------------
# Avisar sería inútil: el aviso sale en la misma pantalla que el resultado, y
# quien lee «71 etapas, 0 fallos» no llega al aviso. Un `|| echo ADVISORY` en
# medio de la lane es exactamente el patrón que el gate restaurado dejó de
# usar. Aquí o el veredicto es atribuible, o no hay veredicto.
#
# CUÁNDO NO SE DISPARA
# -------------------
# * Si el repo tiene un solo checkout, no hay con quién contaminarse.
# * Si hay varios pero **ningún proceso** de otro checkout está compilando, el
#   resultado es atribuible y el gate puede seguir.
#
# Uso:
#   bash scripts/ci/check-build-isolation.sh
#
# Salida: 0 si el veredicto será atribuible, 1 si no, con el motivo en stdout.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

self_root="$(pwd -P)"

# MEDIDO 2026-10-15. Una corrida que fija `CARGO_TARGET_DIR` tiene destino
# propio, así que los otros cinco checkouts **no pueden** escribir en él: la
# contaminación que este script detecta es imposible por construcción. Sin esta
# salida, el preflight rechazaba precisamente el remedio que recomienda — y
# publicar un gate que nunca puede pasar es peor que no tener preflight.
#
# No se acepta cualquier valor: tiene que ser un destino explícito y distinto
# del compartido. Si la variable viene vacía, o apunta al mismo destino de
# siempre, no cuenta como aislamiento y se sigue por el camino normal —
# si no, sería una forma de apagar la comprobación declarando su propio
# destino.
isolated_target="${CARGO_TARGET_DIR:-}"

target_dir="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | sed -E 's/.*"target_directory":"([^"]*)".*/\1/')"
# MEDIDO 2026-10-15: la primera versión de esta comprobación era
# `if [ -z "$t" ] || [ "$t" = "$t" ]`, creyendo que la segunda cláusula detectaba
# un fallo de extracción. Es una comparación de una variable consigo misma:
# **siempre verdadera**, así que el script salía por la rama de «un solo
# checkout» en cada ejecución y no podía negarse nunca. Verde siempre, que es
# exactamente el defecto que este script existe para no tener.
case "$target_dir" in
    /*) ;;
    *)
        # No se pudo leer. No es motivo para bloquear —si no se puede ni nombrar
        # el destino, no hay nada que aislar— pero se dice en voz alta para que
        # un fallo posterior no se lea como un verde limpio.
        echo "build isolation: could not read target_directory from cargo metadata; assuming single checkout"
        exit 0
        ;;
esac

# El aislamiento declarado manda sobre todo lo que viene después: si esta
# corrida escribe en un destino que los otros checkouts no comparten, su
# veredicto es atribuible por construcción, haya quien compile o no.
if [ -n "$isolated_target" ] && [ "$isolated_target" != "$target_dir" ]; then
    echo "build isolation: this run writes to its own target dir ($isolated_target),"
    echo "build isolation: which sibling checkouts cannot reach; verdict is attributable"
    exit 0
fi

# Todos los checkouts de este repo, y cuál es el que está corriendo esto.
others=()
while read -r wt; do
    wt_abs="$(cd "$wt" 2>/dev/null && pwd -P)" || continue
    [ -n "$wt_abs" ] || continue
    [ "$wt_abs" = "$self_root" ] && continue
    others+=("$wt_abs")
done < <(git worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p')

if [ "${#others[@]}" -eq 0 ]; then
    echo "build isolation: single checkout, verdict is attributable"
    exit 0
fi

# Hay más de un checkout: comparten destino. Ahora, ¿está alguno compilando?
# Se busca el proceso compilador y se mira si su línea de comandos pertenece a
# OTRO checkout. Un `cargo` sin ruta no se puede atribuir, así que también
# cuenta como suspeito: lainguishable es peor que ignorable.
suspects=()
while read -r pid args; do
    [ -n "$pid" ] || continue
    for other in "${others[@]}"; do
        case "$args" in
            *"$other"*)
                suspects+=("pid $pid ($(basename "$other"))")
                break
                ;;
        esac
    done
done < <(ps -eo pid=,args= 2>/dev/null | grep -E '(^|[ /])(cargo|rustc)( |$)' | grep -v grep)

# `cargo` o `rustc` corriendo sin ruta atribuible a ESTE repo. No se cuenta
# como bloqueo, y el motivo es medido: la primera versión de este script lo
# contaba y entonces **no pasaba nunca en una máquina con trabajo** — el
# negativo discriminante (dos checkouts, nadie compilando) salía con 1 porque
# contaba los builds de otros proyectos de la misma máquina. Un gate que no
# puede pasar es un gate que nadie lee.
#
# Lo que sí bloquea es lo que se puede **atribuir**: un build de otro checkout
# de este mismo repo, que es la causa reproducida de los dos fallos.
anonymous=$(ps -eo pid=,args= 2>/dev/null \
    | grep -E '(^|[ /])(cargo|rustc)( |$)' \
    | grep -v grep \
    | grep -vF "$self_root" \
    | wc -l)

if [ "${#suspects[@]}" -eq 0 ]; then
    echo "build isolation: ${#others[@]} other checkout(s) of this repo, none compiling; verdict is attributable"
    [ "$anonymous" -gt 0 ] && \
        echo "build isolation: note — $anonymous cargo/rustc process(es) on this machine belong to other projects; not counted, they are not this repo's worktrees"
    exit 0
fi

cat >&2 <<EOF
build isolation: REFUSING — the verdict would not be attributable.

  repo root        : $self_root
  shared target dir: $target_dir
  other checkouts  : ${#others[@]}
    $(printf '%s\n' "${others[@]}")

  ${#suspects[@]} build process(es) attributed to another checkout of this repo:
$(for s in ${suspects+"${suspects[@]}"}; do echo "    $s"; done)

MEDIDO 2026-10-15: dos corridas de este gate fallaron por esto y no por el
código. Un test paniqueó en la línea 900 de un fichero de 861, y otro
comprobó \`source_commit (d5082848…) must match git rev-parse HEAD (d73e7b08…)\`,
que es el HEAD de otro worktree. Un gate que ejecuta el binario de cualquiera
de los ${#others[@]} checkouts no es un gate.

Para obtener un veredicto atribuible, cualquiera de estas dos:

  1. Aislar el destino de esta corrida:
       CARGO_TARGET_DIR=/tmp/cognicode-release-target bash scripts/ci/publish-merge-gate.sh <sha>
     No toca ningún fichero de configuración; es una variable de un proceso.

  2. Serializar las sesiones: esperar a que los otros checkouts terminen de
     compilar, y volver a lanzar.

No se marca esta corrida como PASS ni como fallo del producto. Es FAIL por
entorno, y por eso este script sale con 1 en vez de dejar que la lane continúe.
EOF
exit 1