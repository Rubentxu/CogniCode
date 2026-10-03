#!/usr/bin/env bash
# Se ha construido ESTE binario desde las fuentes que lo construyen?
#
# MEDIDO 2026-10-03. La lane de v0.101.5 ejecuto una `cognicode-release` de otro
# checkout, y lo supo veinte minutos despues, en `skill-bundles`:
#
#     error: unrecognized subcommand 'skills'
#
# El subcomando existe en el arbol. El problema es de donde salio el binario:
# `~/.cargo/config.toml` de esta maquina fija un `build.target-dir` compartido,
# cargo decide por mtimes, y las fuentes de este checkout son mas antiguas que
# un binario que otro checkout construyo despues. Freshness comparada entre
# arboles sin historial comun: no dice nada.
#
# MEDIDO TAMBIEN, y este es el motivo de la segunda version: la guarda original
# comparaba el binario contra TODOS los `.rs` de `crates/`, y eso tambien es
# falso. `cognicode-release` se construye de
#
#     crates/cognicode-cli/src/bin/release.rs
#     crates/cognicode-cli/src/cmd/release_contract.rs   (via #[path])
#     crates/cognicode-cli/src/cmd/release_factory.rs    (via #[path])
#
# y de nada mas. Una edicion en `ide.rs` —que no llega al binario— hacia fallar
# una stage con el binario perfectamente fresco, con un mensaje que señalaba un
# fichero que no tenia nada que ver. MEDIDO: asi paso, en la lane de v0.101.5.
#
# Un guard que rechaza lo que no debe no es un guard conservador: es un guard
# roto, porque entrena a ignorar su veredicto. Por eso aqui el llamante dice
# QUE fuentes construyen el binario, en vez de que el script lo adivine
#-barriendo el arbol entero, que es como se equivoca.
#
# Lo que este script no hace, y conviene decir: no sabe de que checkout salio
# un artefacto cacheado. Lo que sabe es si el binario es mas antiguo que las
# fuentes que lo producen. Con un `CARGO_TARGET_DIR` propio por lane, la
# respuesta es que si, porque cargo no reutiliza entre arboles. Ese es el
# arreglo de maquina; este es el que avisa cuando no se ha hecho.
#
# MEDIDO, escribiendo el contrato: el llamante puede nombrar una fuente
# absoluta, y concatenarla bajo la raiz la convertia en `raiz//var/...`, que no
# existe. El fallo era honesto ("la fuente nombrada no existe") pero mentia
# sobre la causa, y el mensaje señalaba una ruta que nadie escribio.
#
# Uso: check-built-binary.sh <binario> <raiz> <fuente>...
#   <fuente> es absoluta, o relativa a <raiz>.
# Exit: 0 si el binario no es anterior a ninguna fuente nombrada; 1 si lo es.
set -uo pipefail

binary="${1:?usage: check-built-binary.sh <binary> <repo-root> <source>...}"
root="${2:?falta la raiz del arbol}"
shift 2
sources=("$@")

if [ ! -f "$binary" ]; then
    echo "FAIL: no existe el binario $binary." >&2
    echo "  La stage que lo construye corre antes de esta; fallo o no se ejecuto." >&2
    exit 1
fi

if [ "${#sources[@]}" -eq 0 ]; then
    # Fallo cerrado, no por cortesia: una guarda a la que no se le dice que
    # vigilar no vigila nada, y es exactamente el falso verde de siempre.
    echo "FAIL: no se nombro ninguna fuente que construir este binario." >&2
    echo "  Sin una lista, compararlo contra todo el arbol produce falsos" >&2
    echo "  positivos: MEDIDO, comparar contra todos los .rs hacia fallar la" >&2
    echo "  stage con un binario fresco porque habia cambiado un fichero que no" >&2
    echo "  entra en el binario." >&2
    exit 1
fi

older=""
for source in "${sources[@]}"; do
    case "$source" in
        /*) path="$source" ;;
        *)  path="$root/$source" ;;
    esac
    if [ ! -f "$path" ]; then
        echo "FAIL: la fuente nombrada no existe: $path" >&2
        echo "  Si no existe, no se puede afirmar nada sobre el binario." >&2
        exit 1
    fi
    if [ "$path" -nt "$binary" ]; then
        older="$source"
        break
    fi
done

if [ -n "$older" ]; then
    echo "FAIL: $binary es mas antiguo que una fuente que lo construye." >&2
    echo "  fuente   : $older" >&2
    echo "  binario  : $(date -r "$binary" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo '?')" >&2
    echo >&2
    echo "  Cargo decide reconstruir comparando mtimes. Con un directorio de" >&2
    echo "  artefactos compartido entre checkouts, un binario construido por" >&2
    echo "  otro puede ser mas nuevo que las fuentes de este, y entonces esta" >&2
    echo "  lane ejecuta una herramienta que no salio de este arbol —que es lo" >&2
    echo "  que la v0.101.5 hizo, y lo supo veinte minutos mas tarde, blaming un" >&2
    echo "  subcomando que este arbol si tiene." >&2
    echo >&2
    echo "  Que hacer: darle a la lane su propio CARGO_TARGET_DIR, para que las" >&2
    echo "  huellas de cargo sean de un arbol solo." >&2
    exit 1
fi

echo "  ok: $binary no es anterior a ninguna de las ${#sources[@]} fuente(s) que lo construyen"
