#!/usr/bin/env python3
"""La limpieza del clon temporal no puede convertir un PASS en un fallo.

MEDIDO 2026-10-03. El preflight de `v0.101.2` certificó y después la lane
falló. El log completo dice, en este orden:

    → cargo test resultado: passed=5934 failed=0 ignored=30
    → battery dentro de tolerancia
    → Recibo emitido: /tmp/preflight-receipt-d04013c9cded.json
    → PREFLIGHT PASS
    mavis-trash: refusing to trash protected path '.../cognicode-preflight-O2ZYbs'
    mavis-trash: '...' is the parent of the current working directory
    Pipeline finished with FAILURE: shell exited with code 64

El recibo dice `"result": "PASS"`. El proceso salió con 1. Todo el trabajo de
certificar —un clon limpio, 5934 tests, la comparacion con el baseline— se
habia hecho bien, y la lane reporto fallo por no haber borrado una carpeta.

La causa era una linea:

    trap 'rm -rf "$WORK_DIR"' EXIT

El script hace `cd "$WORK_DIR/clone"` en el stage 5, de modo que el trap corre
con el directorio de trabajo **dentro** del directorio que borra. En un entorno
donde `rm` envuelve el borrado con una comprobacion de seguridad —y con
razon: borrar el directorio que contiene tu propio cwd no es una operacion
corriente— la comprobacion se niega y devuelve 64. El estado del trap sustituye
al del script.

## Por que un contrato y no un comentario

El comentario anterior explicaba la intencion del trap y era correcto, y el
gate seguia roto. Es la misma forma que el fallo de `--abbrev-ref`: una guarda
cuya condicion de disparo se cumple en el caso normal y cuyo fallo nadie ve
porque nunca se ha ejecutado el camino de error.

Aqui la propiedad se puede observar en milisegundos. El preflight entero cuesta
veinte minutos y 4G de clon; el comportamiento del trap se puede reproducir con
un `rm` de mentira en el PATH. Por eso la limpieza vive en
`scripts/ci/preflight-cleanup.sh` y este contrato la ejecuta de verdad, en vez
de leerla.

## Que se comprueba

- El `rm` de mentira **rechaza** borrar un directorio que contenga el cwd, que
  es exactamente la regla que fallo. Con el arreglo, la limpieza se produce
  porque sale del directorio antes de borrar.
- Si el borrado falla de todos modos, el estado de salida del script **no**
  cambia: un PASS sigue siendo PASS.
- Un estado de salida distinto de cero se conserva: un fallo real sigue siendo
  un fallo. Un arreglo que hiciese la limpieza inocua no puede convertir un
  fallo de certificacion en un PASS.
- El preflight usa esa limpieza y no una linea de trap propia, para que el
  arreglo no se pueda deshacer en silencio.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
HELPER = REPO_ROOT / "scripts" / "ci" / "preflight-cleanup.sh"
PREFLIGHT = REPO_ROOT / "scripts" / "ci" / "preflight-clean-clone.sh"

# Un `rm` que se comporta como la envoltura que produjo el fallo: se niega a
# borrar un directorio que sea ancestro del cwd, y devuelve 64. Sin esto, el
# contrato probaria un camino que el entorno real no recorre.
FAKE_RM = """#!/usr/bin/env bash
# argv: [-rf] [--] <path>
target=""
for arg in "$@"; do
  case "$arg" in
    -*) continue ;;
    *) target="$arg" ;;
  esac
done
[ -n "$target" ] || exit 0

# La regla que fallo: no se borra un directorio ancestro del cwd.
case "$(pwd -P)/" in
  "$(cd "$target" 2>/dev/null && pwd -P)"/*)
    echo "FAKE_RM: refusing to trash protected path '$target'" >&2
    echo "FAKE_RM: '$target' is the parent of the current working directory" >&2
    exit 64
    ;;
esac
# Y sin --force, `rm` real tambien fallaria sobre un arbol de solo-lectura.
if [ ! -w "$(dirname "$target")" ]; then
  echo "FAKE_RM: '$target' is not writable, refusing" >&2
  exit 64
fi
exec /usr/bin/rm "$@"
"""

# Reproduce el preflight entero en su parte que importa: el trap corre con el
# cwd dentro del clon, que es el estado en el que se-behavioró el fallo.
PREFLIGHT_SHAPE = """#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="{helper_dir}"
source "$SCRIPT_DIR/preflight-cleanup.sh"

WORK_DIR="$(mktemp -d)"
trap 'cognicode_preflight_cleanup "$WORK_DIR"' EXIT

# El stage 5 del preflight real: el proceso se situa dentro del clon.
mkdir -p "$WORK_DIR/clone"
cd "$WORK_DIR/clone"
exit {status}
"""


def run_shape(status: int, fake_rm: bool = True) -> subprocess.CompletedProcess:
    """Ejecuta la forma del preflight y devuelve su estado de salida real."""
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        (root / "clone").mkdir()
        (root / "bin").mkdir()

        if fake_rm:
            rm = root / "bin" / "rm"
            rm.write_text(FAKE_RM, encoding="utf-8")
            rm.chmod(0o755)

        script = root / "preflight.sh"
        script.write_text(
            PREFLIGHT_SHAPE.format(
                helper_dir=str(HELPER.parent), status=status
            ),
            encoding="utf-8",
        )
        script.chmod(0o755)

        return subprocess.run(
            [str(script)],
            capture_output=True,
            text=True,
            timeout=60,
            env={
                "PATH": f"{root / 'bin'}:/usr/bin:/bin",
                "HOME": str(root),
                "TMPDIR": str(root),
            },
        )


# --- Tests ------------------------------------------------------------------


def test_a_passing_preflight_stays_passing_when_cleanup_refuses() -> None:
    """La regresión exacta: exit 0 que salía con 64.

    Sin el arreglo, el `rm` de mentira rechaza el borrado porque el cwd está
    dentro del objetivo, y el estado del trap sustituye al del script.
    """
    done = run_shape(0)
    assert done.returncode == 0, (
        f"un preflight que certifica tiene que salir 0, y salió {done.returncode}. "
        f"La limpieza se esta comiendo el veredicto.\n"
        f"stdout: {done.stdout[-400:]}\nstderr: {done.stderr[-400:]}"
    )
    assert "refusing" not in done.stderr, (
        "el `rm` de mentira se niego a borrar, lo que significa que la limpieza "
        "no salio del directorio antes de intentar el borrado"
    )


def test_a_failing_preflight_stays_failing() -> None:
    """El otro sentido, y el que un arreglo ingenuo rompe.

    Si la limpieza se hiciera inocua a base de `|| true` sobre el `exit`, un
    fallo de certificacion pasaria por alto. Un gate que solo sabe decir PASS
    no es un gate.
    """
    done = run_shape(7)
    assert done.returncode == 7, (
        f"un preflight que sale 7 tiene que seguir saliendo 7, y salio "
        f"{done.returncode}: la limpieza esta alterando un fallo real"
    )


def test_the_temp_clone_is_actually_removed() -> None:
    """Sin esto, la corrección podría ser simplemente no borrar nada.

    Un `|| true` que se tragara el borrado dejaría 4G por ejecución en el disco.
    La limpieza tiene que ocurrir Y no poder fallar el veredicto.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        rm = root / "bin" / "rm"
        rm.parent.mkdir(parents=True, exist_ok=True)
        rm.write_text(FAKE_RM, encoding="utf-8")
        rm.chmod(0o755)
        script = root / "preflight.sh"
        script.write_text(
            PREFLIGHT_SHAPE.format(helper_dir=str(HELPER.parent), status=0),
            encoding="utf-8",
        )
        script.chmod(0o755)
        # Un TMPDIR propio para poder mirar lo que queda.
        work = root / "tmp"
        work.mkdir()
        done = subprocess.run(
            [str(script)],
            capture_output=True,
            text=True,
            timeout=60,
            env={
                "PATH": f"{rm.parent}:/usr/bin:/bin",
                "HOME": str(root),
                "TMPDIR": str(work),
            },
        )
        assert done.returncode == 0, done.stderr[-400:]
        leftovers = list(work.iterdir())
        assert not leftovers, (
            f"quedó {leftovers} en el TMPDIR: la limpieza no se ejecuta de verdad, "
            "y 4G por ejecución no es un precio aceptable para no fallar"
        )


def test_the_preflight_uses_the_shared_cleanup() -> None:
    """El arreglo no se puede deshacer en silencio desde el preflight.

    Si el preflight vuelve a declarar su propio trap, el contrato anterior
    sigue pasando —porque prueba el helper— y el gate vuelve a estar roto.
    """
    text = PREFLIGHT.read_text(encoding="utf-8")
    assert "preflight-cleanup.sh" in text, (
        "preflight-clean-clone.sh ya no carga scripts/ci/preflight-cleanup.sh"
    )
    assert 'trap \'cognicode_preflight_cleanup "$WORK_DIR"\' EXIT' in text, (
        "el trap del preflight no es el del helper compartido"
    )
    # Solo las lineas vivas: el comentario que explica el defecto cita el trap
    # viejo a proposito, y buscarlo en todo el fichero haria que este contrato
    # se pusiera en rojo al documentar el fallo.
    live = [
        line.strip()
        for line in text.splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    assert not any(
        line.startswith("trap") and 'rm -rf' in line and "cognicode_preflight_cleanup" not in line
        for line in live
    ), (
        "el preflight ha vuelto al trap que borra el directorio desde dentro de "
        "si mismo y convierte un PASS en exit 64"
    )


def test_the_helper_reads_the_status_before_anything_else() -> None:
    """`local incoming=$?` tiene que ser la primera sentencia.

    Si algo se ejecuta antes —un `echo`, una expansion, un `[`— el estado que se
    conserva es el de ese comando, no el del script, y el helper deja de ser
    transparente sin que nada falle de forma visible.
    """
    lines = [
        line.strip()
        for line in HELPER.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    start = lines.index("cognicode_preflight_cleanup() {")
    body = [line for line in lines[start + 1 :] if line != "}"]
    first = body[0]
    assert first.startswith("local incoming_status=$?"), (
        f"la primera sentencia de la funcion es `{first}`, y tiene que capturar "
        "el estado de salida entrante antes de ejecutar nada"
    )


def test_the_removal_failure_is_not_structurally_swallowed() -> None:
    """Un `|| true` sobre el borrado hace invisible un clones de 4G por corrida.

    No se comprueba el texto del aviso —eso sería fijar prosa—, sino la forma:
    si el borrado va con `|| true`, su condicion nunca es falsa, el aviso no se
    imprime nunca y la fuga de disco no deja rastro. La limpieza tiene que poder
    fallar SIN poder cambiar el veredicto; "no se puede ni notar" no es lo mismo.
    """
    live = [
        line.strip()
        for line in HELPER.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    offenders = [
        line
        for line in live
        if "rm -rf" in line and "|| true" in line and "if ! rm" not in line
    ]
    assert not offenders, (
        f"el borrado se traga su propio fallo y deja de avisar: {offenders}. "
        "Un clon de 4G por corrida sin un aviso es un fallo silencioso"
    )
    assert any(line.startswith("if ! rm") for line in live), (
        "el borrado tiene que ir dentro de una condicion para poder avisar de "
        "su propio fallo sin propagarlo"
    )


def main() -> int:
    tests = [value for name, value in sorted(globals().items()) if name.startswith("test_")]
    failures: list[str] = []
    for test in tests:
        try:
            test()
        except Exception as error:  # noqa: BLE001 - un contrato reporta, no propaga
            failures.append(f"{test.__name__}: {error}")
    if failures:
        print(f"FAIL - {len(failures)} fallo(s):")
        for failure in failures:
            print(failure)
        return 1
    print(
        "PASS - la limpieza sale del directorio antes de borrarlo y no puede "
        "convertir un PASS en un fallo ni un fallo en un PASS."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
