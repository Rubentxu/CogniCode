#!/usr/bin/env bash
# scripts/ci/build-volume.sh
#
# La regla compartida: ¿cabe el build en el volumen donde va a pasar?
#
# MEDIDO 2026-10-21, dos veces, en el mismo corte de release.
#
# La primera: una corrida del `merge-gate` murio en la etapa
# `cognicode-ladybug` con `Disk quota exceeded`, 32 de 56 etapas. El gate
# aisla su compilacion en `/tmp/cognicode-merge-gate-<id>/target`, y en esta
# maquina `/tmp` es un tmpfs de 48G que no cabe el build debug mas release del
# workspace.
#
# La segunda: el gate de candidato de `v0.101.25` murio dos veces por lo
# mismo. Una en el enlazador, con `ld: Bus error` y `rustc-LLVM ERROR: IO
# failure on output stream: Disk quota exceeded`. Otra, en la etapa
# `host-build-and-verify`, con `FAIL: .../release/cognicode was not built`,
# porque esa etapa verificaba los binarios release antes de construirlos y el
# directorio de destino estaba vacio.
#
# En los dos casos el fallo llego tarde y hablo del sintoma. El volumen no
# cabe es una condicion que se puede comprobar en un segundo, y comprobada
# dice la causa y la salida.
#
# Esta regla vive aqui y en ningun otro sitio. El preflight y el publicador
# del merge-gate la llaman; los dos tienen el mismo problema y la misma
# respuesta.
#
# Uso:
#   source scripts/ci/build-volume.sh
#   require_build_volume /ruta/del/build 32212254720
#
# `require_build_volume` no aborta: devuelve 1 y escribe el mensaje. Quien
# llama decide como terminar, porque uno tiene un `fail()` con recibo y el otro
# un error de entorno.

# El minimo por defecto. MEDIDO: el clon limpio de este workspace ocupa mas de
# 20G solo con el perfil debug, con treinta y pico gramaticas de tree-sitter.
# El default es un margen por encima de esa medida, porque un margen que se
# queda corto solo cambia el momento del fallo, no el fallo.
#
# Se puede bajar con PREFLIGHT_MIN_FREE_BYTES cuando el workspace sea mas
# pequeno. Que lo baje es decision del operador: un default que se baja solo
# es un default que no vigila nada.
BUILD_VOLUME_MIN_FREE_BYTES="${PREFLIGHT_MIN_FREE_BYTES:-32212254720}"  # 30 GiB

# Bytes libres en el volumen que monta `dir`.
build_volume_free_bytes() {
  df -PB1 "$1" 2>/dev/null | awk 'NR==2 {print $4}'
}

# El volumen donde el build va a pasar.
#
# `PREFLIGHT_WORK_DIR` manda sobre `TMPDIR`, que a su vez manda sobre `/tmp`.
# El orden importa: `TMPDIR` es la convencion del sistema y `mktemp -d` la
# respeta sola; `PREFLIGHT_WORK_DIR` es el nombre explicito de esta regla y
# tiene que ganar cuando esta puesto.
build_volume_root() {
  if [ -n "${PREFLIGHT_WORK_DIR:-}" ]; then
    printf '%s' "$PREFLIGHT_WORK_DIR"
  else
    printf '%s' "${TMPDIR:-/tmp}"
  fi
}

# Comprueba el volumen. Devuelve 0 si cabe, 1 si no cabe, y escribe el
# motivo en stderr cuando no cabe.
require_build_volume() {
  local dir="${1:-}"
  local need="${2:-$BUILD_VOLUME_MIN_FREE_BYTES}"
  local avail

  if [ -z "$dir" ]; then
    dir="$(build_volume_root)"
  fi
  if [ ! -d "$dir" ]; then
    mkdir -p "$dir" 2>/dev/null || true
  fi

  avail="$(build_volume_free_bytes "$dir" || true)"
  if [ -z "$avail" ]; then
    printf 'no se puede medir el espacio libre de %s\n' "$dir" >&2
    return 1
  fi

  if [ "$avail" -ge "$need" ]; then
    printf 'volumen %s: %s bytes libres, hacen falta %s\n' "$dir" "$avail" "$need"
    return 0
  fi

  {
    printf 'FAIL: el volumen de trabajo no tiene sitio para el build.\n'
    printf '  volumen:   %s\n' "$dir"
    printf '  libre:     %s bytes\n' "$avail"
    printf '  necesario: %s bytes\n' "$need"
    printf '  El build debug del workspace no cabe, y el enlazador muere con\n'
    printf '  "Disk quota exceeded" a mitad, sin decir que el problema es el disco.\n'
    printf '  Mueve la compilacion a un volumen con sitio:\n'
    printf '    PREFLIGHT_WORK_DIR=/ruta/con/sitio bash scripts/ci/preflight-clean-clone.sh\n'
    printf '  o exporta TMPDIR antes de la corrida. Si el workspace es mas pequeno\n'
    printf '  y el minimo no corresponde, ajusta PREFLIGHT_MIN_FREE_BYTES.\n'
  } >&2
  return 1
}
