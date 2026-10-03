#!/usr/bin/env bash
# Can this target actually compile native code, or only Rust?
#
# MEDIDO 2026-10-03. La stage `toolchain-for-$target` de
# `release-candidate.pipeline.kts` comprobaba una sola cosa:
#
#     rustup target list --installed | grep -qx '<target>'
#
# y su propio comentario decia que "a cross target also needs a linker for it.
# Building for a target whose binaries cannot link produces a candidate that
# looks built and is not". El comentario describe una garantia que la stage no
# hacia.
#
# El arbol tiene crates con build script de C y C++ —`ring`, `tree-sitter`,
# `link-cplusplus`—, asi que la diferencia entre "el target esta instalado" y
# "el target compila" no es academica. La lane de v0.101.4 fallo en
# `binaries-aarch64` veinte minutos despues de pasar esa stage:
#
#     error occurred in cc-rs: failed to find tool "aarch64-linux-gnu-g++"
#     error occurred in cc-rs: command did not execute successfully (status
#       code exit status: 1): zig-cc-aarch64 ... --target=aarch64-unknown-linux-gnu
#       -> unable to parse target query 'aarch64-unknown-linux-gnu':
#          UnknownOperatingSystem
#
# Tres fallos, un mensaje cada uno, y ninguno de los tres decia "a este target
# le falta el compilador de C". El primero era un `CXX` sin definir; los otros
# dos, que `cc-rs` anade `--target=<triple de Rust>` y el compilador cruzado
# no lo parseaba.
#
# Preguntar al compilador cuesta milisegundos y responde la pregunta que de
# verdad importa. Preguntar a cargo cuesta veinte minutos y responde con el
# nombre de un crate que nadie estaba mirando.
#
# Este script no decide que un target sea buildable —solo el build lo hace—.
# Decide que su toolchain nativa responde, que es la precondicion que faltaba.
#
# Uso: check-cross-toolchain.sh <target-triple>
set -uo pipefail

target="${1:?usage: check-cross-toolchain.sh <target-triple>}"
host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"

if [ "$target" = "$host" ]; then
    echo "  $target is the host target; the native toolchain is the build's own"
    exit 0
fi

work="$(mktemp -d)"
# Se sale del directorio antes de borrar: el mismo motivo por el que existe
# `preflight-cleanup.sh`. Si `TMPDIR` estuviera dentro del arbol, borrar el cwd
# desde dentro de si mismo es exactamente la operacion que un envoltorio con
# comprobaciones de seguridad se niega a hacer, y entonces este script
# devolveria 64 en lugar de su veredicto real.
trap 'cd / && rm -rf "$work"' EXIT
# La variante con guiones bajos es la que `cc-rs` lee como variable de shell.
# La variante con guiones solo puede existir en el entorno —un nombre de
# variable de bash no admite guiones—, asi que se lee con `printenv` y no con
# expansion indirecta, que aborta con "nombre de variable invalido".
underscored="${target//-/_}"

resolve() { # <kind> <env-with-underscores> <env-with-dashes> <fallback-prefix>
    local kind="$1" under="$2" dashed="$3" prefix="$4" value
    value="${!under:-}"
    [ -n "$value" ] || value="$(printenv "$dashed" 2>/dev/null || true)"
    if [ -n "$value" ]; then
        printf '%s' "$value"
        return 0
    fi
    if command -v "${prefix}gcc" >/dev/null 2>&1; then
        command -v "${prefix}gcc"
        return 0
    fi
    if [ "$kind" = cxx ] && command -v "${prefix}g++" >/dev/null 2>&1; then
        command -v "${prefix}g++"
        return 0
    fi
    return 1
}

fail() {
    echo "FAIL: this target cannot compile native code, and the build needs it."
    echo "  target : $target"
    echo "  reason : $1"
    echo "  The crates 'ring', 'tree-sitter' and 'link-cplusplus' have C/C++ build"
    echo "  scripts, so a missing or non-functional cross compiler fails this build"
    echo "  long after this stage said the target was fine."
    echo "  For a zig-based cross toolchain, the wrapper has to translate the Rust"
    echo "  triple into the one zig parses: cc-rs appends"
    echo "  --target=$target and a wrapper that only bakes its own -target is"
    echo "  overridden by it."
    exit 1
}

# --- C ----------------------------------------------------------------------
printf 'int main(void){return 0;}\n' > "$work/probe.c"
if ! cc_bin=$(resolve c "CC_$underscored" "CC_$target" "${target%%-*}-linux-gnu-"); then
    fail "no C compiler for $target: set CC_$underscored, or put a cross gcc on PATH"
fi
if ! "$cc_bin" -O0 --target="$target" -c "$work/probe.c" -o "$work/probe.o" >"$work/cc.log" 2>&1; then
    echo "  tried: $cc_bin --target=$target (see below)"
    sed 's/^/    /' "$work/cc.log" | head -5
    fail "the C compiler for $target is present but rejects the target triple"
fi
echo "  C   : $cc_bin compiles a probe for $target"

# --- C++ --------------------------------------------------------------------
# Solo se exige si hay un C++ configurado: no todos los toolchain lo traen, y
# exigirlo sin motivo seria un gate que bloquea por una herramienta que este
# proyecto puede no necesitar. Si hay uno configurado y no funciona, eso si es
# un fallo — el build lo encontraria igual.
if cxx_bin=$(resolve cxx "CXX_$underscored" "CXX_$target" "${target%%-*}-linux-gnu-"); then
    printf '#include <string>\nint main(){std::string s="x";return (int)s.size()-1;}\n' > "$work/probe.cpp"
    if ! "$cxx_bin" -O0 --target="$target" -c "$work/probe.cpp" -o "$work/probe-cxx.o" >"$work/cxx.log" 2>&1; then
        echo "  tried: $cxx_bin --target=$target (see below)"
        sed 's/^/    /' "$work/cxx.log" | head -5
        fail "a C++ compiler is configured for $target but does not work"
    fi
    echo "  C++ : $cxx_bin compiles a probe for $target"
else
    echo "  C++ : none configured for $target; required only by C++ build scripts"
fi

echo "  native toolchain for $target answers"
