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

# Un `rustc` que no responde. Deja un rastro para que el test vea que se le
# pregunto, y sale sin decir que host es: es el caso de un toolchain roto, que
# antes se traducía en un veredicto sobre el target que no era sobre el host.
RUSTC_SILENT = """#!/usr/bin/env bash
: > "${RUSTC_MARKER:?the marker has to be reachable}"
exit 1
"""

# Un `rustc` que responde con el host y no instala nada.
RUSTC_HOSTS = """#!/usr/bin/env bash
echo "host: x86_64-unknown-linux-gnu"
"""


def host_target() -> str:
    """El triple del host, resuelto aqui y con el entorno real de este proceso.

    MEDIDO 2026-10-25. Resolverlo DENTRO de la guarda, con el HOME aislado que
    hace el aislamiento, hace que rustup intente instalar la toolchain desde la
    red: 117,7 s por llamada y `TimeoutExpired` a los 120 s. Resolverlo aqui,
    una vez, con el entorno real, cuesta 9 ms.

    Si `rustc -vV` no dice cual es el host, esto falla y lo dice. Adivinar un
    triple es la forma de que un veredicto de la guarda dependa de una suposicion
    en vez de de una medida.
    """
    out = subprocess.run(
        ["rustc", "-vV"], capture_output=True, text=True, timeout=60
    )
    for line in out.stdout.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise AssertionError(
        "'rustc -vV' no devolvio el triple del host:\n" + out.stdout + out.stderr
    )


def run_guard(
    target: str,
    *,
    host: str | None = None,
    resolve_host: bool = False,
    bin_dir: str | None = None,
    **env_extra: str,
) -> subprocess.CompletedProcess:
    """Ejecuta la guarda con un entorno controlado y devuelve su estado real.

    El host hay que darlo. La guarda lo resuelve sola con `rustc -vV`, y esa
    ruta, con el HOME aislado que hace este aislamiento, hace que rustup
    descargue la toolchain: 117,7 s por llamada, y `TimeoutExpired` a los 120 s.
    Para ejercitar esa ruta a proposito, `resolve_host=True`.

    `bin_dir` antepone un directorio al PATH. Lo usa el test que pone un `rustc`
    de mentira: sin el, la guarda usaria el rustc de verdad, que no es lo que
    el test quiere medir.
    """
    if resolve_host:
        if host is not None:
            raise ValueError(
                "resolve_host=True ya dice que la guarda resuelve el host: "
                "darlo ademas hace que el test no mida lo que dice medir"
            )
    elif host is None:
        raise ValueError(
            "el host hay que darlo: la guarda lo resuelve con `rustc -vV`, y con "
            "el HOME aislado eso hace que rustup descargue la toolchain. Si lo que "
            "se quiere es ejercitar esa ruta, pide resolve_host=True."
        )
    argv = [str(GUARD), target]
    if host is not None:
        argv.append(host)
    with tempfile.TemporaryDirectory() as raw:
        # El PATH se antepone el dir temporal pero conserva el real: la guarda
        # llama a `rustc -vV` para resolver el host, y con un PATH minimo el
        # target nativo parecia uno cruzado y se rechazaba a si mismo.
        prefix = f"{bin_dir}:" if bin_dir else ""
        env = {
            "PATH": f"{prefix}{raw}:{os.environ.get('PATH', '/usr/bin:/bin')}",
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
            argv,
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
    done = run_guard(host_target(), host=host_target())
    assert done.returncode == 0, (
        f"el target nativo debe pasar sin comprobacion cruzada.\n{done.stdout}"
    )
    assert "host" in done.stdout.lower(), done.stdout


def test_a_target_with_no_compiler_fails_and_says_which_variable() -> None:
    target = "aarch64-unknown-linux-gnu"
    done = run_guard(
        target,
        host=host_target(),
        CC_aarch64_unknown_linux_gnu="",
        CXX_aarch64_unknown_linux_gnu="",
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
        done = run_guard(target, host=host_target(), CC_aarch64_unknown_linux_gnu=cc)
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
    """Un C y un C++ que compilan el probe: la guarda pasa y dice cuales uso.

    El C++ tambien se configura porque para un target cruzado es
    precondicion, no un extra: ver
    `test_a_target_with_no_cxx_compiler_fails_and_says_which_variable`. Con las
    dos mitas en su sitio, la guarda tiene que nombrar las dos, para que un
    fallo posterior se pueda atribuir a un compilador y no al otro.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc = make_cc(raw, CC_OK, "fake-cc-ok")
        cxx = make_cc(raw, CC_OK, "fake-cxx-ok")
        done = run_guard(
            target,
            host=host_target(),
            CC_aarch64_unknown_linux_gnu=cc,
            CXX_aarch64_unknown_linux_gnu=cxx,
        )
    assert done.returncode == 0, (
        f"unos compiladores que compilan el probe deben pasar.\nstdout:\n{done.stdout}\n"
        f"stderr:\n{done.stderr}"
    )
    assert "fake-cc-ok" in done.stdout, (
        f"la guarda debe decir que compilador de C uso, para que un fallo "
        f"posterior se pueda atribuir.\n{done.stdout}"
    )
    assert "fake-cxx-ok" in done.stdout, (
        f"la guarda debe decir que compilador de C++ uso.\n{done.stdout}"
    )


def test_a_broken_cxx_compiler_fails() -> None:
    """Un C++ configurado y roto es fallo, y tiene que decirlo.

    Si el operador ha configurado uno, el build lo usara, asi que un C++ roto
    tiene que decirselo ahora y no veinte minutos despues.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc_ok = make_cc(raw, CC_OK, "fake-cc-ok")
        cxx_bad = make_cc(raw, CC_REJECTS, "fake-cxx-bad")
        bad = run_guard(
            target,
            host=host_target(),
            CC_aarch64_unknown_linux_gnu=cc_ok,
            CXX_aarch64_unknown_linux_gnu=cxx_bad,
        )
    assert bad.returncode != 0, f"un C++ roto debe fallar.\n{bad.stdout}"
    assert "C++" in bad.stdout, bad.stdout


def test_a_target_with_no_cxx_compiler_fails_and_says_which_variable() -> None:
    """Sin C++ esta lane no compila. Medido, no supuesto.

    MEDIDO 2026-10-03. La lane de v0.101.4 cayo en `binaries-aarch64` con

        error occurred in cc-rs: failed to find tool "aarch64-linux-gnu-g++"

    y este test, hasta ahora, PINABA LO CONTRARIO: afirmaba que un target sin
    C++ configurado debe pasar, con el motivo de que "no todos los toolchain lo
    traen". Ese motivo es cierto en general y falso aqui. `cc-rs` no eligio
    g++ por capricho: un build script del arbol lo pidio, asi que la ausencia
    de C++ no es una hipotesis sobre el toolchain, es un fallo conocido de
    este repositorio.

    El efecto medido del comportamiento anterior era una guarda que pasaba
    exactamente con el entorno que hacia fracasar el build veinte minutos mas
    tarde. Una guarda que no puede fallar sobre la condicion que dice cubrir no
    es una guarda: es una linea de log.

    Asi que la ausencia de C++ para un target cruzado es fallo, y el mensaje
    tiene que nombrar la variable que falta. Un fallo que no dice cual
    configurar es un fallo que se repite.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc_ok = make_cc(raw, CC_OK, "fake-cc-ok")
        absent = run_guard(
            target, host=host_target(), CC_aarch64_unknown_linux_gnu=cc_ok
        )
    assert absent.returncode != 0, (
        f"sin C++ esta lane no compila, y la guarda no puede decir que si.\n"
        f"{absent.stdout}"
    )
    assert "CXX_aarch64_unknown_linux_gnu" in absent.stdout, (
        f"el fallo tiene que nombrar la variable que falta, no solo que falta algo.\n"
        f"{absent.stdout}"
    )


def test_the_guard_does_not_ask_rustc_when_the_host_is_given() -> None:
    """Con el host dado, la guarda no necesita preguntar a rustc.

    MEDIDO 2026-10-25. La guarda resolvia el host con `rustc -vV` DENTRO de un
    HOME aislado, y ahi rustup no encuentra ninguna toolchain: lee el
    `rust-toolchain.toml` del repositorio e intenta instalar la suya desde la
    red. Medido: 117,7 s por llamada, y el `timeout=120` del contrato se
    agotaba. Con la red un poco mas lenta, el contrato fallaba y la etapa
    `contracts` del gate caia -- sin que el codigo del arbol tuviera nada que
    ver. Con el host dado, la guarda no toca rustc y no puede depender de la red.
    """
    with tempfile.TemporaryDirectory() as raw:
        marker = Path(raw) / "rustc-was-called"
        make_cc(raw, RUSTC_SILENT, "rustc")
        done = run_guard(
            "x86_64-unknown-linux-gnu",
            host="x86_64-unknown-linux-gnu",
            bin_dir=raw,
            RUSTC_MARKER=str(marker),
        )
        called = marker.exists()
    assert done.returncode == 0, (
        f"con el host dado, la guarda tiene que pasar sin preguntar a rustc.\n"
        f"{done.stdout}{done.stderr}"
    )
    assert not called, (
        "la guarda no puede preguntar a rustc cuando el host ya se le dio: esa "
        "pregunta es la que hace que rustup descargue la toolchain"
    )


def test_a_guard_that_cannot_resolve_the_host_refuses_to_give_a_verdict() -> None:
    """Sin host, y con un rustc que no responde, la guarda no puede veredicto.

    Antes, un `rustc -vV` que fallaba dejaba el host vacio, y el target nativo
    --el que se estaba comprobando-- se rechazaba como si fuera cruzado. Ese
    veredicto hablaba del compilador que no se llego a mirar y del toolchain que
    estaba roto, y decia "falta el compilador de C". Un fallo que no dice su
    causa es un fallo que se repite.
    """
    with tempfile.TemporaryDirectory() as raw:
        make_cc(raw, RUSTC_SILENT, "rustc")
        done = run_guard(
            "x86_64-unknown-linux-gnu",
            resolve_host=True,
            bin_dir=raw,
            RUSTC_MARKER=str(Path(raw) / "rustc-was-called"),
        )
    assert done.returncode != 0, (
        "sin host la guarda no puede dar un veredicto: no sabe si el target es "
        "el nativo\n" + done.stdout + done.stderr
    )
    combined = done.stdout + done.stderr
    assert "host" in combined.lower(), (
        f"el motivo tiene que decir que no se pudo resolver el host.\n{combined}"
    )
    assert "no C compiler" not in combined, (
        f"el motivo no puede ser que falte el compilador: el compilador no llego "
        f"a preguntarse.\n{combined}"
    )


def test_the_guard_resolves_the_host_when_it_is_not_given() -> None:
    """La ruta de resolver el host dentro de la guarda sigue viva.

    El contrato ya no la ejercita de forma implicita --le da el host--, asi que
    esta comprobacion la mantiene a proposito: un `rustc` de mentira responde
    con el host y la guarda lo usa.
    """
    with tempfile.TemporaryDirectory() as raw:
        make_cc(raw, RUSTC_HOSTS, "rustc")
        done = run_guard(
            "x86_64-unknown-linux-gnu", resolve_host=True, bin_dir=raw
        )
    assert done.returncode == 0, (
        f"un rustc que responde con el host tiene que bastarle a la guarda.\n"
        f"{done.stdout}{done.stderr}"
    )
    assert "host target" in done.stdout.lower(), (
        f"la guarda tiene que decir que el target es el host.\n{done.stdout}"
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
