#!/usr/bin/env bash
# Ejecuta un binario del release para un target dado, y dice COMO.
#
# MEDIDO 2026-10-03. Las stages `binary-smoke-$target` y
# `archive-standalone-$target` invocaban el binario tal cual:
#
#     "$dir/cogh" --version
#
# En el target del host eso es ejecutarlo. En un target extranjero el kernel no
# puede, entra binfmt_misc, y lo que sale es esto:
#
#     qemu-aarch64-static: Could not open '/lib/ld-linux-aarch64.so.1': No such
#     file or directory
#
# que no dice lo que pasa. El emulador no ha encontrado el cargador dinamico
# que el binario lleva horneado, y eso no es un fallo del binario: es que la
# maquina no tiene la libc invitada de ese target. Presentado asi, una
# precondicion pendiente de la maquina parece un binario roto, y cuesta una
# tarde entera de diagnostico.
#
# MEDIDO tambien: la lane nunca habia llegado a ver esto. `binaries-aarch64`
# siempre habia fallado antes, y la v0.101.5 es la primera que construye el
# binario y luego intenta ejecutarlo.
#
# Que hace este script:
#
#   1. Si el target es el del host, ejecuta el comando directamente. Un qemu
#      alrededor de un binario nativo seria una comprobacion mas, no mas
#      evidencia.
#   2. Si no, busca el emulador (`qemu-<arquitectura>-static`, con
#      `QEMU_<TRIPLE_CON_GUIONES_BAJOS>` para sobreescribirlo) y el prefijo del
#      sysroot (`QEMU_LD_PREFIX`, o los sitios donde lo instala cada
#      distribucion), y ejecuta a traves del emulador con `-L`.
#   3. Si falta cualquiera de los dos, falla ANTES de intentarlo y nombra lo que
#      falta y de donde se saca.
#
# El sysroot no se instala desde aqui a proposito: instalar paquetes en la
# maquina es una decision del operador, no de un script que corre en una lane.
# Este dice que se necesita, no lo resuelve.
#
# Uso: run-target-binary.sh <target-triple> <comando> [args...]
set -uo pipefail

target="${1:?usage: run-target-binary.sh <target-triple> <command> [args...]}"
shift
[ "$#" -ge 1 ] || { echo "FAIL: no hay comando que ejecutar para ${target}" >&2; exit 2; }

host="$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
if [ -z "$host" ]; then
    echo "FAIL: no se pudo leer el target del host de 'rustc -vV'." >&2
    echo "      Sin el no se puede decidir si ${target} es cruzado." >&2
    exit 2
fi

# El host no necesita emulador ni sysroot, y es el caso que mas se repite: la
# mitad de los binarios de un release se ejecutan aqui.
if [ "$target" = "$host" ]; then
    exec "$@"
fi

arch="${target%%-*}"
underscored="${target//-/_}"

# Emulador. La variante con guiones solo puede existir en el entorno —un nombre
# de variable de bash no admite guiones— asi que se lee con printenv y no con
# expansion indirecta, que aborta con "nombre de variable invalido".
qemu_var_dashed="QEMU_${target}"
qemu_var_under="QEMU_${underscored}"
qemu_bin="${!qemu_var_under:-}"
[ -n "$qemu_bin" ] || qemu_bin="$(printenv "$qemu_var_dashed" 2>/dev/null || true)"
if [ -z "$qemu_bin" ]; then
    if command -v "qemu-${arch}-static" >/dev/null 2>&1; then
        qemu_bin="$(command -v "qemu-${arch}-static")"
    elif command -v "qemu-${arch}" >/dev/null 2>&1; then
        qemu_bin="$(command -v "qemu-${arch}")"
    fi
fi

if [ -z "$qemu_bin" ]; then
    cat >&2 <<EOF
FAIL: el target ${target} no es el del host (${host}) y no hay emulador para
      ejecutarlo en esta maquina.
  hace falta : qemu-${arch}-static en PATH
  o         : ${qemu_var_under} / ${qemu_var_dashed} apuntando a el

  Sin emulador no se puede probar el binario, y un binario que no se ejecuta es
  un binario cuya superficie no se ha visto. En la instalacion de Fedora el
  paquete es 'qemu-user-static-${arch}'.
EOF
    exit 1
fi

# Prefijo del sysroot: el directorio bajo el cual el emulador busca las
# ficheros que el binario cita con rutas absolutas, /lib/ld-linux-aarch64.so.1
# entre ellos. Por eso se anade el 'usr': lo que el paquete instala en
# <raiz>/usr/lib tiene que aparecer como <prefijo>/lib.
prefix="${QEMU_LD_PREFIX:-}"
if [ -z "$prefix" ]; then
    for candidate in \
        "/usr/${arch}-redhat-linux/sys-root/fc44/usr" \
        "/usr/${arch}-linux-gnu" \
        "/usr/sysroots/${arch}-linux" \
        "/usr/${arch}-linux-gnu-sysroot"
    do
        if [ -e "${candidate}/lib/ld-linux-${arch}.so.1" ]; then
            prefix="$candidate"
            break
        fi
    done
fi

if [ -z "$prefix" ] || [ ! -e "${prefix}/lib/ld-linux-${arch}.so.1" ]; then
    cat >&2 <<EOF
FAIL: hay emulador para ${target} (${qemu_bin}) pero no la libc invitada que
      necesita para arrancar.
  busca    : ${prefix:-<sin prefijo>}/lib/ld-linux-${arch}.so.1
  o        : QEMU_LD_PREFIX=<raiz del sysroot> apuntando a un arbol que tenga
             lib/ld-linux-${arch}.so.1

  Sin ese fichero el emulador responde 'Could not open /lib/ld-linux-${arch}.so.1',
  que habla de un fichero del emulador y no de lo que falta. En la instalacion
  de Fedora el paquete es 'sysroot-${arch}-fc44-glibc'; sin privileges de
  instalacion se puede descargar y extraer, y apuntar aqui QEMU_LD_PREFIX al
  directorio 'usr' de su interior.
EOF
    exit 1
fi

exec "$qemu_bin" -L "$prefix" "$@"
