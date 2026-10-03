#!/usr/bin/env bash
# ============================================================================
# preflight-cleanup.sh — la limpieza del preflight no puede cambiar el veredicto.
# ----------------------------------------------------------------------------
# Se carga (source) desde `preflight-clean-clone.sh`. Define una sola funcion.
#
# MEDIDO 2026-10-03. El preflight de `v0.101.2` certificó correctamente y a
# continuación la lane falló:
#
#     → cargo test resultado: passed=5934 failed=0 ignored=30
#     → battery dentro de tolerancia
#     → PREFLIGHT PASS
#     mavis-trash: refusing to trash protected path '.../cognicode-preflight-O2ZYbs'
#     mavis-trash: '...' is the parent of the current working directory
#     Pipeline finished with FAILURE: shell exited with code 64
#
# La causa era una sola linea, el trap de salida:
#
#     trap 'rm -rf "$WORK_DIR"' EXIT
#
# El script hace `cd "$WORK_DIR/clone"` en el stage 5, asi que cuando corre el
# trap el directorio de trabajo esta DENTRO del directorio que el trap intenta
# borrar. En un entorno donde `rm` envuelve el borrado con una comprobacion de
# seguridad, esa comprobacion se niega —es lo correcto: borrar el directorio que
# contiene tu propio cwd no puede ser una operacion corriente— y devuelve 64.
# El estado de salida del trap sustituye al del script, y un PASS se convierte en
# un fallo.
#
# Hay dos defectos distintos y ambos importan:
#
#   1. El trap borra el directorio desde dentro de si mismo. Salir antes no es
#      cortesia, es lo que hace la operacion posible.
#   2. Una limpieza puede fallar. Fallar la limpieza NO es fallar la
#      certificacion. Un gate que muere en su propia limpieza no distingue
#      "el codigo esta roto" de "no consegui borrar una carpeta temporal", y en
#      cuanto ocurre lo segundo el gate entero deja de decir nada.
#
# Por eso vive en su propio fichero y no como una linea mas del trap: asi un
# contrato puede ejecutarla en milisegundos y demostrar las dos propiedades,
# sin clonar 4G para poder observar un `rm`.
# ============================================================================

# Uso:  trap 'cognicode_preflight_cleanup "$WORK_DIR"' EXIT
#
# `$?` tiene que leerse en la PRIMERA sentencia: cualquier comando anterior ya
# habria sobrescrito el estado de salida que se quiere conservar.
cognicode_preflight_cleanup() {
    local incoming_status=$?
    local work_dir="${1:-}"

    if [ -n "$work_dir" ] && [ -d "$work_dir" ]; then
        # Salir del directorio antes de borrarlo. `cd /` es el destino mas
        # simple y no depende de que el arbol de trabajo siga existiendo.
        cd / 2>/dev/null || cd "$HOME" 2>/dev/null || true

        # El borrado no puede ser el veredicto. Si falla —envoltura de
        # seguridad, permisos, un fichero abierto— se dice y se sigue con el
        # estado que el script ya habia decidido.
        if ! rm -rf -- "$work_dir" 2>/dev/null; then
            printf 'aviso: no se pudo limpiar el clon temporal %s\n' \
                "$work_dir" >&2
            printf '       la certificacion no depende de la limpieza, y este\n' >&2
            printf '       directorio se puede borrar a mano sin riesgo.\n' >&2
        fi
    fi

    # El estado con el que el script ya habia decidido. Nunca el de la limpieza.
    exit "$incoming_status"
}
