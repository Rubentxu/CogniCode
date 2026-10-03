#!/usr/bin/env python3
"""Como corre la lane un binario de un target que no es el host.

MEDIDO 2026-10-03. La stage `binary-smoke-$target` invoca el binario tal cual:

    "${'$'}dir/cogh" --version

En x86_64 eso es ejecutarlo. En aarch64 el kernel no puede, asi que entra
binfmt_misc, que lanza `qemu-aarch64-static` sin opciones, y qemu busca el
cargador que el binario lleva horneado:

    qemu-aarch64-static: Could not open '/lib/ld-linux-aarch64.so.1': No such
    file or directory

que es el fallo mas caro de leer que produce esta lane: no dice que falta, dice
que el emulador no encontro un fichero del que el emulador depende. Y la lane no
llego a verlo hasta ahora, porque `binaries-aarch64` siempre habia fallado antes:
la v0.101.5 es la primera que construye el binario y luego intenta ejecutarlo.

`archive-standalone-$target` tiene el mismo problema —descomprime y ejecuta
`bin/cogh`— asi que la pieza es compartida, y por eso vive en un script con
contrato y no en el cuerpo de una stage.

La regla que fija este contrato: la lane dice COMO va a ejecutar un binario
extranjero, y si no puede, lo dice antes de intentarlo y nombra lo que falta.

MEDIDO tambien: este contrato se escribio primero como `unittest.TestCase` y
`python3 scripts/ci/test_run_target_binary.py` lo daba en verde, porque su
`main()` era el de unittest. El runner de la suite usa `dir(module)` y solo ve
funciones `test_*` a nivel de modulo, asi que|reporto `exposes no test_*
function` y la suite salio 1. Un contrato que solo se puede correr de una forma
es un contrato que no se esta ejecutando.
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNNER = REPO_ROOT / "scripts" / "ci" / "run-target-binary.sh"
CANDIDATE = REPO_ROOT / "release-candidate.pipeline.kts"

HOST = "x86_64-unknown-linux-gnu"
FOREIGN = "aarch64-unknown-linux-gnu"


def fake_binary(directory: str) -> str:
    path = Path(directory) / "prog"
    path.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
    path.chmod(0o755)
    return str(path)


def fake_qemu(directory: str, log: str) -> str:
    """Un qemu de mentira que anota como le llamaron y devuelve 0."""
    path = Path(directory) / "qemu-aarch64-static"
    path.write_text(
        "#!/usr/bin/env bash\n"
        f'printf "%s\\n" "$*" > "{log}"\n'
        "exit 0\n",
        encoding="utf-8",
    )
    path.chmod(0o755)
    return str(path)


def make_sysroot(directory: str, *, with_loader: bool = True) -> str:
    """Un prefijo de sysroot, con o sin el cargador que el emulador necesita."""
    prefix = Path(directory) / "sysroot"
    (prefix / "lib").mkdir(parents=True)
    if with_loader:
        (prefix / "lib" / "ld-linux-aarch64.so.1").write_bytes(b"\x7fELF")
    return str(prefix)


def run_runner(args: list[str], **env_extra: str) -> subprocess.CompletedProcess:
    with tempfile.TemporaryDirectory() as raw:
        env = {
            "PATH": f"{raw}:{os.environ.get('PATH', '/usr/bin:/bin')}",
            "HOME": raw,
            "TMPDIR": raw,
        }
        env.update({k: v for k, v in env_extra.items() if v})
        return subprocess.run(
            [str(RUNNER), *args],
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
        )


def test_the_runner_script_exists_and_is_executable() -> None:
    assert RUNNER.is_file(), f"no existe {RUNNER}"
    assert os.access(RUNNER, os.X_OK), f"{RUNNER} no es ejecutable"


def test_a_native_target_is_run_directly() -> None:
    """El caso nativo no puede pasar por qemu: seria una comprobacion falsa."""
    with tempfile.TemporaryDirectory() as raw:
        prog = fake_binary(raw)
        done = subprocess.run(
            [str(RUNNER), HOST, prog, "--version"],
            capture_output=True,
            text=True,
            timeout=120,
            env={"PATH": os.environ.get("PATH", "/usr/bin:/bin")},
        )
    assert done.returncode == 0, done.stderr
    assert "qemu" not in done.stdout + done.stderr, (
        f"un binario nativo no necesita emulador.\n{done.stdout}{done.stderr}"
    )


def test_a_foreign_target_goes_through_qemu_with_a_sysroot_prefix() -> None:
    with tempfile.TemporaryDirectory() as raw:
        log = Path(raw) / "qemu.args"
        prog = fake_binary(raw)
        qemu = fake_qemu(raw, str(log))
        prefix = make_sysroot(raw)
        done = run_runner(
            [FOREIGN, prog, "--version"],
            QEMU_aarch64_unknown_linux_gnu=qemu,
            QEMU_LD_PREFIX=prefix,
        )
        # El log se lee DENTRO del `with`: fuera, el directorio temporal ya no
        # existe y la aserto siguiente veria una cadena vacia.
        seen = log.read_text(encoding="utf-8") if log.exists() else ""
    assert done.returncode == 0, done.stderr
    assert "-L" in seen, f"qemu debe recibir el prefijo del sysroot: {seen!r}"
    assert prefix in seen, f"y el prefijo debe ser el descubierto: {seen!r}"
    assert "--version" in seen, f"y el comando del binario: {seen!r}"


def test_a_prefix_without_the_loader_is_refused() -> None:
    """Un `-L` a un arbol sin cargador devuelve justo el error que esto evita."""
    with tempfile.TemporaryDirectory() as raw:
        log = Path(raw) / "qemu.args"
        prog = fake_binary(raw)
        qemu = fake_qemu(raw, str(log))
        prefix = make_sysroot(raw, with_loader=False)
        done = run_runner(
            [FOREIGN, prog, "--version"],
            QEMU_aarch64_unknown_linux_gnu=qemu,
            QEMU_LD_PREFIX=prefix,
        )
    assert done.returncode != 0, done.stdout
    assert not log.exists(), (
        "no debe delegar en el emulador un prefijo que ya se sabe incompleto"
    )
    assert "ld-linux-aarch64.so.1" in done.stdout + done.stderr, (
        f"el fallo tiene que nombrar el fichero que falta.\n{done.stdout}{done.stderr}"
    )


def test_a_foreign_target_without_a_runner_says_what_is_missing() -> None:
    """El fallo de precondicion tiene que nombrar la precondicion."""
    with tempfile.TemporaryDirectory() as raw:
        prog = fake_binary(raw)
        done = run_runner([FOREIGN, prog, "--version"], QEMU_LD_PREFIX=raw)
    assert done.returncode != 0, done.stdout
    combined = (done.stdout + done.stderr).lower()
    assert "qemu" in combined, f"debe nombrar el emulador: {combined!r}"
    assert "aarch64" in combined, f"debe nombrar el target: {combined!r}"


def test_both_stages_that_execute_binaries_use_the_runner() -> None:
    """Si la stage ejecuta el binario por su cuenta, el arreglo no sirve."""
    text = CANDIDATE.read_text(encoding="utf-8")
    for stage in ("binary-smoke-$target", "archive-standalone-$target"):
        start = text.index(f'stage("{stage}")')
        nxt = text.find('stage("', start + 10)
        body = text[start : nxt if nxt != -1 else len(text)]
        assert "run-target-binary.sh" in body, (
            f"la stage {stage} ejecuta un binario de {FOREIGN}: tiene que decir "
            f"como, no esperar que binfmt_misc lo resuelva por ella"
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
    if not tests:
        print("FAIL: este modulo no expone ninguna funcion test_*")
        return 1
    failures = 0
    for name in tests:
        try:
            getattr(module, name)()
        except AssertionError as exc:
            failures += 1
            print(f"  FAIL {name}")
            for line in str(exc).splitlines():
                print(f"       {line}")
        else:
            print(f"  PASS {name}")
    print(f"  -> {len(tests) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
