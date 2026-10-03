#!/usr/bin/env python3
"""Un target instalado no es un target que compile.

MEDIDO 2026-10-03. La stage `toolchain-for-$target` de
`release-candidate.pipeline.kts` comprobaba una sola cosa —que el target
estuviera en `rustup target list --installed`— mientras su propio comentario
decia que un target cuyas binaries no pueden enlazar "produces a candidate that
looks built and is not". El comentario describia una garantia que la stage no
hacia.

La lane de `v0.101.4` loENSEÑO de la peor manera: la stage pasó, y
`binaries-aarch64` fallo veinte minutos despues con tres errores distintos y
ninguno de ellos diciendo "a este target le falta el compilador de C":

    error occurred in cc-rs: failed to find tool "aarch64-linux-gnu-g++"
        -> CXX_aarch64_unknown_linux_gnu no estaba definido
    error occurred in cc-rs: command did not execute successfully
        ... zig-cc-aarch64 ... "--target=aarch64-unknown-linux-gnu"
        -> unable to parse target query 'aarch64-unknown-linux-gnu':
           UnknownOperatingSystem

Los dos ultimos son la misma causa: `cc-rs` anade `--target=<triple de Rust>` y
un wrapper que solo hornea su propio `-target` queda **pisado** por ese
argumento. El wrapper tiene que traducir el triple, no solo lurking.

Es la misma forma que los otros dos hallazgos de esta sesion: una guarda cuya
condicion de disparo se cumple en el caso normal, y cuyo camino de error nadie
recorre porque nunca se ha ejecutado.

## Que se comprueba

Este contrato ejecuta `scripts/ci/check-cross-toolchain.sh` de verdad, con
compiladores de mentira en el PATH y en el entorno:

- **Target nativo**: no exige nada; el toolchain del build es el suyo.
- **Sin compilador**: falla, y el motivo dice que variable definir.
- **Compilador que existe pero rechaza el triple**: falla tambien. Este es el
  caso que la stage anterior no podia ver: un compilador instalado que no
  compila para el target se comporta, desde fuera, igual que uno que no existe.
- **Compilador que funciona**: pasa, y dice cual fue.
- Un compilador de C++ configurado que no funciona tambien es fallo, porque el
  build lo encontraria igual; pero la ausencia de C++ no se exige, porque no
  todos los toolchain lo traen y no todos los proyectos lo necesitan.

Y la stage del pipeline llama al script, que es lo que hace que la guarda exista
en el sitio donde se anuncia.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUARD = REPO_ROOT / "scripts" / "ci" / "check-cross-toolchain.sh"
CANDIDATE = REPO_ROOT / "release-candidate.pipeline.kts"

# Compilador de mentira que ACCEPTa el triple: escribe un objeto y sale 0.
CC_OK = """#!/usr/bin/env bash
set -euo pipefail
out=""
prev=""
for a in "$@"; do
  if [ "$prev" = "-o" ]; then out="$a"; fi
  prev="$a"
done
[ -n "$out" ] && : > "$out"
exit 0
"""

# Compilador de mentira que RECHAZA el triple, como hacia zig sin traduccion.
CC_REJECTS = """#!/usr/bin/env bash
set -euo pipefail
for a in "$@"; do
  case "$a" in
    --target=aarch64-unknown-linux-gnu)
      echo "error: unable to parse target query 'aarch64-unknown-linux-gnu': UnknownOperatingSystem" >&2
      exit 1 ;;
  esac
done
exit 0
"""


def host_target() -> str:
    out = subprocess.run(
        ["rustc", "-vV"], capture_output=True, text=True, timeout=60
    )
    for line in out.stdout.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    return "x86_64-unknown-linux-gnu"


def run_guard(target: str, **env_extra: str) -> subprocess.CompletedProcess:
    """Ejecuta la guarda con un entorno controlada y devuelve su estado real."""
    with tempfile.TemporaryDirectory() as raw:
        # El PATH se antepone el dir temporal pero conserva el real: la guarda
        # llama a `rustc -vV` para resolver el host, y con un PATH minimo el
        # target nativo parecia uno cruzado y se rechazaba a si mismo.
        env = {
            "PATH": f"{raw}:{os.environ.get('PATH', '/usr/bin:/bin')}",
            "HOME": raw,
            "TMPDIR": raw,
        }
        for name in list(env_extra):
            if env_extra[name] == "":
                env.pop(name, None)
        env.update({k: v for k, v in env_extra.items() if v != ""})
        # Se llama por ruta absoluta: el PATH de la guarda es deliberadamente
        # minimo y no debe decides si el propio script se encuentra.
        return subprocess.run(
            [str(GUARD), target],
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
        )


def make_cc(directory: str, body: str, name: str = "fake-cc") -> str:
    """Un compilador de mentira, en SU PROPIO fichero.

    Dos llamadas al mismo nombre se pisan: la segunda reescribe la primera y
    ambos tests acaban usando el mismo compilador. Por eso el nombre es un
    parametro y no una constante.
    """
    path = Path(directory) / name
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)
    return str(path)


def test_the_guard_exists_and_is_executable() -> None:
    assert GUARD.is_file(), f"no existe {GUARD}"
    assert os.access(GUARD, os.X_OK), f"{GUARD} no es ejecutable"


def test_a_native_target_is_not_cross_checked() -> None:
    """Preguntar por el toolchain nativo solo añadiria ruido."""
    done = run_guard(host_target())
    assert done.returncode == 0, (
        f"el target nativo debe pasar sin comprobacion cruzada.\n{done.stdout}"
    )
    assert "host" in done.stdout.lower(), done.stdout


def test_a_target_with_no_compiler_fails_and_says_which_variable() -> None:
    target = "aarch64-unknown-linux-gnu"
    done = run_guard(
        target, CC_aarch64_unknown_linux_gnu="", CXX_aarch64_unknown_linux_gnu=""
    )
    assert done.returncode != 0, (
        f"sin compilador de C el target no compila y la guarda debe decirlo.\n"
        f"{done.stdout}"
    )
    assert "CC_aarch64_unknown_linux_gnu" in done.stdout, (
        f"el motivo tiene que decir que variable definir, no solo que algo "
        f"falta.\n{done.stdout}"
    )


def test_a_compiler_that_rejects_the_triple_fails() -> None:
    """El caso que la stage anterior no podia ver.

    Un compilador instalado que no parsea el triple se comporta, desde fuera,
    igual que uno que no existe — salvo que la stage de rustup lo daba por
    bueno. Este es exactamente el fallo de v0.101.4.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc = make_cc(raw, CC_REJECTS)
        done = run_guard(target, CC_aarch64_unknown_linux_gnu=cc)
    assert done.returncode != 0, (
        f"un compilador que rechaza el triple debe fallar la guarda.\n{done.stdout}"
    )
    assert "rejects the target triple" in done.stdout, (
        f"el motivo tiene que distinguir 'no hay compilador' de 'el "
        f"compilador no compila para este target'.\n{done.stdout}"
    )
    # Y debe mostrar el error del propio compilador: sin el, quien depure esto
    # tiene que adivinar que paso.
    assert "UnknownOperatingSystem" in done.stdout, (
        f"la salida del compilador es la unica evidencia del por que.\n{done.stdout}"
    )


def test_a_working_compiler_passes_and_names_itself() -> None:
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc = make_cc(raw, CC_OK)
        done = run_guard(target, CC_aarch64_unknown_linux_gnu=cc)
    assert done.returncode == 0, (
        f"un compilador que compila el probe debe pasar.\nstdout:\n{done.stdout}\n"
        f"stderr:\n{done.stderr}"
    )
    assert "fake-cc" in done.stdout, (
        f"la guarda debe decir que compilador uso, para que un fallo posterior "
        f"se pueda atribuir.\n{done.stdout}"
    )


def test_a_broken_cxx_compiler_fails_but_its_absence_does_not() -> None:
    """Un C++ configurado y roto es fallo; no tenerlo no lo es.

    Exigir C++ sin motivo seria un gate que bloquea por una herramienta que
    este proyecto puede no necesitar. Pero si el operador ha configurado uno, el
    build lo usara, asi que un C++ roto tiene que decirselo ahora.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc_ok = make_cc(raw, CC_OK, "fake-cc-ok")
        cxx_bad = make_cc(raw, CC_REJECTS, "fake-cxx-bad")
        bad = run_guard(
            target, CC_aarch64_unknown_linux_gnu=cc_ok, CXX_aarch64_unknown_linux_gnu=cxx_bad
        )
        absent = run_guard(target, CC_aarch64_unknown_linux_gnu=cc_ok)
    assert bad.returncode != 0, f"un C++ roto debe fallar.\n{bad.stdout}"
    assert "C++" in bad.stdout, bad.stdout
    assert absent.returncode == 0, (
        f"sin C++ configurado debe pasar: no todos los toolchain lo traen.\n"
        f"{absent.stdout}"
    )


def test_the_candidate_stage_actually_calls_the_guard() -> None:
    """La guarda tiene que estar donde se anuncia, o no es una guarda."""
    text = CANDIDATE.read_text(encoding="utf-8")
    start = text.index('stage("toolchain-for-$target")')
    end = text.index('stage("binaries-$target")', start)
    body = text[start:end]
    assert "check-cross-toolchain.sh" in body, (
        "la stage toolchain-for-$target debe llamar a la guarda: el target "
        "instalado por rustup no demuestra que el target compile codigo nativo"
    )
    assert "rustup target list --installed" in body, (
        "la comprobacion de rustup se queda: solo comprobaba una mitad de la "
        "precondicion, no la sustituye"
    )


def main() -> int:
    module = sys.modules[__name__]
    tests = sorted(
        (
            name
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda name: getattr(module, name).__code__.co_firstlineno,
    )
    assert tests, (
        "ningun test_ encontrado: este main() no ejecutaria nada y reportaria "
        "PASS. Un contrato vacio es un fallo, no una linea verde."
    )
    failures: list[str] = []
    for test in tests:
        try:
            getattr(module, test)()
            print(f"  PASS {test}")
        except Exception as error:  # noqa: BLE001 - un contrato reporta, no propaga
            print(f"  FAIL {test}: {error}")
            failures.append(f"{test}: {error}")
    if failures:
        print(f"FAIL - {len(failures)} fallo(s)")
        return 1
    print(
        "PASS - un target instalado que no compila codigo nativo no se cuela "
        "como un target listo, que es lo que la stage prometia desde el "
        "principio."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
