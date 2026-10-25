#!/usr/bin/env bash
# cosign-tool.sh — el cosign que este repositorio firma con, y solo ese.
#
# EL HUECO
# ---------
# `cosign attest-blob` cambio de comportamiento en la v3.1.4: `--tlog-upload` ya
# no existe, el registro de transparencia paso a ser obligatorio, y cada llamada
# pide aceptar los terminos del servicio de sigstore.
#
# MEDIDO, con y sin terminal:
#
#   * sin terminal, el prompt se resuelve como `Error: user declined the prompt`
#     y el comando sale con rc=1;
#   * con terminal, respondiendo `y`, firma y **publica la huella en el registro
#     publico de Rekor**;
#   * la aceptacion no se cachea: el prompt vuelve en cada llamada.
#
# Un lane no tiene terminal. MEDIDO: un step de PipelineK arranca con
# `STDIN: NO TTY` y `STDOUT: NO TTY`.
#
# Por eso la v3.1.4 no puede firmar en este lane, y por eso el lane fija la
# version que el ADR de procedencia ya midio: la v3.1.3, que firma offline, sin
# red y sin registro, con la clave que acompaña a la release.
#
# QUE HACE ESTE FICHERO
# ---------------------
# Declara la version fijada, la suma oficial de cada plataforma y donde vive
# el binario. Es la unica fuente: el generador, el contrato y el instalador
# leen de aqui, para que no puedan discrepar sobre que version se firma.
#
# La version se cambia aqui y en ningun otro sitio, y el contrato
# `test_provenance_lane_pins_its_cosign.py` lo vigila.
#
# Uso:
#   . scripts/ci/cosign-tool.sh            # define las variables
#   scripts/ci/cosign-tool.sh --version    # la version fijada
#   scripts/ci/cosign-tool.sh --path       # la ruta del binario; sale 3 si no esta
#   scripts/ci/cosign-tool.sh --sha256     # la suma fijada de esta plataforma
#   scripts/ci/cosign-tool.sh --asset      # el nombre del asset oficial
#
# MEDIDO: el `cosign` del PATH de esta maquina es la v3.1.4 (rpm instalado el
# 2026-10-23 06:12). Ese es el que rompe el lane, y por eso no se usa.

# La version fijada. La que el ADR de procedencia midio como firmada offline y
# sin registro de transparencia.
COGNICODE_COSIGN_VERSION="3.1.3"

# Las sumas oficiales de la release (`cosign_checksums.txt` de la release
# v3.1.3). El instalador verifica contra la de su plataforma antes de dejar el
# binario en su sitio: un binario cuya suma no sea esta no se instala.
COGNICODE_COSIGN_SHA256_linux_amd64="4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71"
COGNICODE_COSIGN_SHA256_linux_arm64="c5d324e091826b0d7a78eb16fef316450b4eb9aaec045611c08ba06f5e73220a"
COGNICODE_COSIGN_SHA256_darwin_amd64="2347488e5d5b25336644024dfeca5601b190e91197a71a917bda44744aff106c"
COGNICODE_COSIGN_SHA256_darwin_arm64="5cf948c2f4dfe59687bdd0b8523709067383e03982cc543475c8a7dc70e92a76"

# Donde vive la herramienta. Fuera del repositorio: es un binario de terceros,
# no una fuente del proyecto, y no tiene por que viajar con el codigo.
COGNICODE_COSIGN_TOOLS_DIR="${COGNICODE_TOOLS_DIR:-${HOME}/.cache/cognicode/tools}"
COGNICODE_COSIGN_BIN="${COGNICODE_COSIGN_TOOLS_DIR}/cosign/${COGNICODE_COSIGN_VERSION}/cosign"

# La plataforma, en el nombre que usa la release.
cosign_tool_platform() {
    local os arch
    case "$(uname -s)" in
        Linux)  os="linux" ;;
        Darwin) os="darwin" ;;
        *) return 1 ;;
    esac
    case "$(uname -m)" in
        x86_64|amd64)  arch="amd64" ;;
        aarch64|arm64) arch="arm64" ;;
        *) return 1 ;;
    esac
    printf '%s-%s\n' "${os}" "${arch}"
}

# El nombre del asset oficial de esta plataforma.
cosign_tool_asset() {
    local platform
    platform="$(cosign_tool_platform)" || return 1
    printf 'cosign-%s\n' "${platform}"
}

# La suma fijada de esta plataforma.
cosign_tool_sha256() {
    local platform
    platform="$(cosign_tool_platform)" || return 1
    case "${platform}" in
        linux-amd64)  printf '%s\n' "${COGNICODE_COSIGN_SHA256_linux_amd64}" ;;
        linux-arm64)  printf '%s\n' "${COGNICODE_COSIGN_SHA256_linux_arm64}" ;;
        darwin-amd64) printf '%s\n' "${COGNICODE_COSIGN_SHA256_darwin_amd64}" ;;
        darwin-arm64) printf '%s\n' "${COGNICODE_COSIGN_SHA256_darwin_arm64}" ;;
        *) return 1 ;;
    esac
}

# La ruta del binario fijado, si esta instalado. No hay vuelta al `cosign` del
# PATH: caer en el es volver al defecto que este fichero existe para evitar.
cosign_tool_path() {
    if [ -x "${COGNICODE_COSIGN_BIN}" ]; then
        printf '%s\n' "${COGNICODE_COSIGN_BIN}"
        return 0
    fi
    return 3
}

cosign_tool_missing_message() {
    printf 'el cosign fijado (%s) no esta instalado en %s\n' \
        "${COGNICODE_COSIGN_VERSION}" "${COGNICODE_COSIGN_BIN}" >&2
    printf 'instálalo con: bash scripts/ci/install-cosign.sh\n' >&2
}

# Modo CLI. Fuenteste, este fichero solo define variables.
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    case "${1:-}" in
        --version)
            printf '%s\n' "${COGNICODE_COSIGN_VERSION}"
            ;;
        --path)
            if ! cosign_tool_path; then
                cosign_tool_missing_message
                exit 3
            fi
            ;;
        --sha256)
            cosign_tool_sha256 || {
                printf 'plataforma no soportada: %s\n' "$(uname -s)/$(uname -m)" >&2
                exit 2
            }
            ;;
        --asset)
            cosign_tool_asset || {
                printf 'plataforma no soportada: %s\n' "$(uname -s)/$(uname -m)" >&2
                exit 2
            }
            ;;
        *)
            printf 'uso: cosign-tool.sh --version|--path|--sha256|--asset\n' >&2
            exit 2
            ;;
    esac
fi
