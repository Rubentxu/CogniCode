#!/usr/bin/env bash
# install-cosign.sh — deja el cosign fijado en su sitio, verificado.
#
# QUE HACE
# --------
# Descarga el binario oficial de la version fijada, comprueba su suma contra la
# que esta declarada en `cosign-tool.sh`, y lo deja en el directorio de
# herramientas. Si ya esta instalado y su suma es la dectada, no hace nada: la
# operacion es idempotente y una lane no tiene por que pagar la descarga en
# cada corrida.
#
# POR QUE EXISTE, Y POR QUE NO ES UN `curl | sh`
# -------------------------------------------------
# El binario de un tercero se verifica por su suma, no por su procedencia. La
# suma esta escrita en este repositorio, y la release que la publico esta
# identificada por su version: cambiar la version sin cambiar la suma deja el
# instalador rejecting el binario, que es exactamente lo que tiene que pasar.
#
# El coste es explicito: la lane necesita red para descargar la herramienta. Es
# un coste que se paga una vez por maquina, no en cada firma, y no cambia lo que
# una release afirma de si misma — eso lo decide el ADR de procedencia.
#
# Uso:
#   bash scripts/ci/install-cosign.sh
#   bash scripts/ci/install-cosign.sh --check              # solo dice si esta y con que suma
#   bash scripts/ci/install-cosign.sh --version 3.1.3      # y se niega si no es la declarada
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/ci/cosign-tool.sh
. "${SCRIPT_DIR}/cosign-tool.sh"

die() {
    printf 'FAIL: %s\n' "$1" >&2
    shift
    for line in "$@"; do
        printf '      %s\n' "${line}" >&2
    done
    exit 1
}

mode="install"
requested=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --check) mode="check"; shift ;;
        --version)
            requested="${2:-}"
            if [ "$#" -ge 2 ]; then shift 2; else shift; fi
            ;;
        -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
        *) die "unknown argument '$1'" "usage: install-cosign.sh [--check] [--version X]" ;;
    esac
done

# La lane dice que version instala; la herramienta dice cual usa. Si no son la
# misma, el lane y los contratos estarian midiendo herramientas distintas, y
# eso se para aqui — antes de descargar nada.
if [ -n "${requested}" ] && [ "${requested}" != "${COGNICODE_COSIGN_VERSION}" ]; then
    die "the requested cosign is not the declared one" \
        "requested: ${requested}" \
        "declared : ${COGNICODE_COSIGN_VERSION}" \
        "The lane signs with the declared version. Installing another one" \
        "would leave the lane and the contracts measuring different tools."
fi

platform="$(cosign_tool_platform)" \
    || die "plataforma no soportada: $(uname -s)/$(uname -m)" \
           "El lane de release construye para linux-x86_64."
asset="$(cosign_tool_asset)"
expected="$(cosign_tool_sha256)"
target="${COGNICODE_COSIGN_BIN}"

if [ -x "${target}" ]; then
    actual="$(sha256sum "${target}" | cut -d' ' -f1)"
    if [ "${actual}" = "${expected}" ]; then
        echo "cosign ${COGNICODE_COSIGN_VERSION} ya esta instalado: ${target}"
        exit 0
    fi
    if [ "${mode}" = "check" ]; then
        die "el cosign de ${target} no tiene la suma fijada" \
            "esperada : ${expected}" \
            "encontrada: ${actual}" \
            "Reinstalalo con: bash scripts/ci/install-cosign.sh"
    fi
    echo "el cosign de ${target} no tiene la suma fijada; se reinstala" >&2
fi

if [ "${mode}" = "check" ]; then
    die "el cosign fijado (${COGNICODE_COSIGN_VERSION}) no esta instalado" \
        "Se espera en: ${target}" \
        "Instalalo con: bash scripts/ci/install-cosign.sh"
fi

url="https://github.com/sigstore/cosign/releases/download/v${COGNICODE_COSIGN_VERSION}/${asset}"
echo "descargando ${url}"
tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT
curl -sSfL -o "${tmp}/cosign" "${url}" \
    || die "no se pudo descargar ${url}" \
           "El lane necesita red para fijar la herramienta. Una vez instalada, ya no."

actual="$(sha256sum "${tmp}/cosign" | cut -d' ' -f1)"
if [ "${actual}" != "${expected}" ]; then
    die "la suma del binario descargado no es la fijada" \
        "esperada  : ${expected}" \
        "encontrada: ${actual}" \
        "Un binario cuya suma no coincide no se instala, y no se ejecuta."
fi

chmod +x "${tmp}/cosign"
mkdir -p "$(dirname "${target}")"
mv -f "${tmp}/cosign" "${target}"

echo "instalado cosign ${COGNICODE_COSIGN_VERSION}: ${target}"
echo "  suma verificada: ${expected}"
echo "  plataforma      : ${platform}"
