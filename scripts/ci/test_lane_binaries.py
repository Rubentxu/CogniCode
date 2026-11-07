#!/usr/bin/env python3
"""Los binarios de la lane: de que arbol salen y como se ejecutan.

Dos contratos sobre los scripts de la lane que tocan un binario construido,
fusionados en un fichero. Los dos fabrican arboles y binarios de mentira en un
directorio temporal, ejecutan el script real y comprueban su veredicto.

## Parte 1: el binario es de ESTE arbol (`check-built-binary.sh`)

Que una lane certificara con binarios de SU arbol, no de otro checkout.

MEDIDO 2026-10-03. La lane de v0.101.5 cayo en `skill-bundles`:

    error: unrecognized subcommand 'skills'

El subcomando existe en el arbol —7d074fef es ancestro del tag— y la stage lo
pide bien. El fallo es de donde salio el binario: `/var/home/rubentxu/
cargo-targets/release/cognicode-release` era de las 20:13, construido desde otro
checkout, y la lane corrio a las 21:30. `cargo build` no lo toco porque compara
mtimes y las fuentes de este checkout son mas antiguas que ese binario.

Es la misma clase que el resto de los hallazgos de esta sesion: una
precondicion que el build da por cumplida y nadie comprueba. Aqui la pregunta
es "¿este binario es de este arbol?", que es la que sostiene que la lane
certifica el codigo que dice certificar.

`~/.cargo/config.toml` de esta maquina fija `build.target-dir` a un directorio
compartido por varios checkouts, y `scripts/ci/target-dir.sh` lo resuelve a
proposito. Que ese directorio se comparta es un hecho asumido; que un binario
de ahi sea de otro arbol no lo es, y hasta ahora no se miraba.

Y hay un segundo defecto, MEDIDO el 2026-10-03 en la misma lane: la guarda
comparaba el binario contra TODOS los `.rs` de `crates/`, y `ide.rs` —que no
entra en `cognicode-release`— hacia fallar la stage con un binario perfectamente
fresco. Un guard que rechaza lo que no debe entrena a ignorar su veredicto.

La correccion no es una lista de fuentes escrita a mano: MEDIDO el 2026-10-24,
`cognicode-release` se construye de 415 ficheros repartidos en cuatro crates
(368 en `cognicode-core`), y una lista de los cuatro ficheros del CLI habria
sido verde en falso ante un cambio en `cognicode-core`. Por eso la guarda lee
el dep-info que escribe rustc junto al binario, que es la respuesta de cargo y
no puede quedarse corta.

## Parte 2: como se ejecuta un binario de otro target (`run-target-binary.sh`)

Como corre la lane un binario de un target que no es el host.

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
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


# --- Parte 1: frescura del binario -------------------------------------------

CHECK = REPO_ROOT / "scripts" / "ci" / "check-built-binary.sh"


def make_tree(directory: str, name: str = "repo") -> tuple[Path, Path, Path]:
    """Un arbol de mentira: las fuentes que construyen el binario, y el binario.

    Devuelve (root, source, unrelated):
      root      — el arbol
      source    — un fichero que SI construye el binario
      unrelated — un fichero del mismo crate que NO lo construye
    """
    root = Path(directory) / name
    crate = root / "crates" / "thing"
    crate.mkdir(parents=True)
    source = crate / "lib.rs"
    source.write_text("fn main() {}\n", encoding="utf-8")
    unrelated = crate / "otro.rs"
    unrelated.write_text("pub fn otro() {}\n", encoding="utf-8")
    binary = root / "target" / "release" / "thing"
    binary.parent.mkdir(parents=True)
    binary.write_text("#!/bin/sh\n", encoding="utf-8")
    binary.chmod(0o755)
    return root, source, unrelated


def write_dep_info(binary: Path, sources: list[Path]) -> Path:
    """El dep-info que rustc escribe junto al binario.

    Formato makefile: `objetivo: dep1 dep2 ...`, con rutas absolutas —y por eso
    el dep-info nombra tambien el arbol desde el que se construyo.
    """
    depfile = binary.with_name(binary.name + ".d")
    depfile.write_text(
        str(binary) + ": " + " ".join(str(s) for s in sources) + "\n",
        encoding="utf-8",
    )
    return depfile


def run_check(binary: Path, root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", str(CHECK), str(binary), str(root)],
        capture_output=True,
        text=True,
        timeout=120,
    )


def test_the_check_exists() -> None:
    assert CHECK.is_file(), f"no existe {CHECK}"
    assert os.access(CHECK, os.X_OK), f"{CHECK} no es ejecutable"


def test_a_binary_built_from_this_tree_passes() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        write_dep_info(binary, [source])
        now = time.time()
        os.utime(binary, (now, now))
        os.utime(source, (now - 10, now - 10))
        done = run_check(binary, root)
    assert done.returncode == 0, done.stdout + done.stderr


def test_a_source_that_does_not_build_the_binary_cannot_trip_it() -> None:
    """MEDIDO 2026-10-03: la guarda de la primera version comparaba contra
    todos los `.rs` de `crates/` y rechazaba la stage porque habia cambiado
    `ide.rs`, que no entra en `cognicode-release`. Un guard que rechaza lo que
    no debe entrena a ignorar su veredicto."""
    with tempfile.TemporaryDirectory() as raw:
        root, source, unrelated = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        write_dep_info(binary, [source])
        now = time.time()
        # El binario es fresco respecto a lo que lo construye, y `unrelated` es
        # mas nuevo que el: la stage tiene que pasar igual.
        os.utime(binary, (now, now))
        os.utime(source, (now - 10, now - 10))
        os.utime(unrelated, (now + 60, now + 60))
        done = run_check(binary, root)
    assert done.returncode == 0, (
        f"un fichero que no construye el binario no puede rechazarlo.\n"
        f"{done.stdout}{done.stderr}"
    )


def test_a_binary_older_than_a_source_is_rejected() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        write_dep_info(binary, [source])
        old = time.time() - 3600
        os.utime(binary, (old, old))
        done = run_check(binary, root)
    assert done.returncode != 0, done.stdout
    combined = done.stdout + done.stderr
    assert "fuente" in combined, (
        f"el mensaje tiene que decir que fuente lo deja viejo, no solo que pasa.\n{combined}"
    )


def test_a_binary_built_from_another_tree_is_rejected() -> None:
    """MEDIDO 2026-10-03: el defecto original. El binario lo construyo otro
    checkout, cargo lo dio por fresco, y la lane ejecuto una herramienta que no
    salio de este arbol."""
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw, name="este-arbol")
        other_root, other_source, _ = make_tree(raw, name="otro-arbol")
        binary = root / "target" / "release" / "thing"
        # El dep-info nombra OTRO arbol: el binario es de ahi.
        write_dep_info(binary, [other_source])
        now = time.time()
        os.utime(binary, (now, now))
        os.utime(other_source, (now - 10, now - 10))
        done = run_check(binary, root)
    assert done.returncode != 0, (
        "un binario de otro arbol tiene que ser rechazado\n" + done.stdout + done.stderr
    )
    combined = done.stdout + done.stderr
    assert "otro-arbol" in combined, (
        f"el mensaje tiene que nombrar el arbol del que si es el binario.\n{combined}"
    )


def test_a_dep_info_mixing_two_trees_is_rejected() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw, name="este-arbol")
        _, other_source, _ = make_tree(raw, name="otro-arbol")
        binary = root / "target" / "release" / "thing"
        write_dep_info(binary, [source, other_source])
        now = time.time()
        os.utime(binary, (now, now))
        os.utime(source, (now - 10, now - 10))
        os.utime(other_source, (now - 10, now - 10))
        done = run_check(binary, root)
    assert done.returncode != 0, (
        "un dep-info con fuentes de dos arboles no puede dar un veredicto de "
        "frescura\n" + done.stdout + done.stderr
    )


def test_a_missing_dep_info_fails_closed() -> None:
    """Un binario sin dep-info no lo construyo cargo desde aqui. Fallar
    cerrado, no pasar: asumir es lo que produjo el falso verde."""
    with tempfile.TemporaryDirectory() as raw:
        root, _, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        now = time.time()
        os.utime(binary, (now, now))
        done = run_check(binary, root)
    assert done.returncode != 0, (
        "sin dep-info la guarda tiene que negarse a dar un veredicto\n"
        + done.stdout + done.stderr
    )
    assert "dep-info" in (done.stdout + done.stderr)


def test_a_missing_binary_is_not_a_pass() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        write_dep_info(binary, [source])
        binary.unlink()
        done = run_check(binary, root)
    assert done.returncode != 0, "un binario ausente no es un binario fresco"
    assert "no existe" in (done.stdout + done.stderr)


def test_git_metadata_of_the_checkout_is_not_a_source() -> None:
    """En un worktree, el dep-info apunta al `.git` del repo principal. Si eso
    contara como fuente, la comparacion de arboles no significaria nada: dos
    worktrees del mismo repo tienen `.git` comun."""
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        git_head = root / ".git" / "HEAD"
        git_head.parent.mkdir(parents=True)
        git_head.write_text("ref: refs/heads/main\n", encoding="utf-8")
        write_dep_info(binary, [source, git_head])
        now = time.time()
        os.utime(binary, (now, now))
        os.utime(source, (now - 10, now - 10))
        # El `.git/HEAD` es mas nuevo que el binario: si contara como fuente,
        # la guarda rechazaria, y seria un falso positivo.
        os.utime(git_head, (now + 60, now + 60))
        done = run_check(binary, root)
    assert done.returncode == 0, (
        f"los metadatos de git no son fuentes del binario.\n{done.stdout}{done.stderr}"
    )


# --- Parte 2: ejecucion de un binario de otro target --------------------------

RUNNER = REPO_ROOT / "scripts" / "ci" / "run-target-binary.sh"

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
        # The rustup proxy needs RUSTUP_HOME to find the toolchain when HOME
        # is overridden (the proxy's default of $HOME/.rustup would point
        # at this tempfile). Without it, `rustc -vV` hangs and the runner
        # script reports a host it could not read. Prefer the parent env
        # value, then fall back to the standard location so the proxy
        # resolves in <100 ms the same way it does on a developer machine.
        rustup_home = os.environ.get("RUSTUP_HOME") or os.path.expanduser("~/.rustup")
        env["RUSTUP_HOME"] = rustup_home
        cargo_home = os.environ.get("CARGO_HOME") or os.path.expanduser("~/.cargo")
        env["CARGO_HOME"] = cargo_home
        env.update({k: v for k, v in env_extra.items() if v})
        return subprocess.run(
            [str(RUNNER), *args],
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
        )


def _repo_scripts() -> list[Path]:
    """Every shell script that belongs to THIS repository.

    `rglob` over the repository root also walks `.claude/worktrees/`, where an
    agent's checkout holds its own copy of every script.
    """
    skip = {".git", "node_modules", "target", ".claude", "leases", "staging"}
    return [
        path
        for path in REPO_ROOT.rglob("*.sh")
        if not any(part in skip for part in path.relative_to(REPO_ROOT).parts)
    ]


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


def test_the_runner_is_the_only_way_to_execute_a_foreign_binary() -> None:
    """Un binario de otro target no se ejecuta solo: hay que decir como.

    Antes de 2026-11-04 esto exigia que dos stages de la lane del candidato
    usaran `run-target-binary.sh`. Las lanes se retiraron; el runner se queda,
    y la propiedad que queda —la que este contrato sigue vigilando— es que no
    haya otra via de ejecutar un binario de un target extranjero que dependa
    de que `binfmt_misc` lo resuelva por su cuenta.

    Un binario que se ejecuta sin decir como es un binario que funciona en la
    maquina del que lo probo y falla en otra, sin decir por que.
    """
    # NOMBRAR el target no es ejecutar un binario de ese target: `install.sh`
    # mapea `aarch64` a su triple, `stage-platform-payloads.sh` lo declara
    # en una tabla y `check-cross-toolchain.sh` lo cita en un mensaje de error.
    # Ninguno de los tres ejecuta un binario foreign.
    #
    # Lo que si es un defecto es EJECUTAR un binario de otro target sin decir
    # como: un binario que se ejecuta sin saber quien lo ejecuta es un binario
    # que funciona en la maquina del que lo probo y falla en otra.
    offenders = []
    for path in _repo_scripts():
        if path.name == "run-target-binary.sh":
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        # Una invocacion directa de un binario del target: el path del
        # binario con el triple en el, ejecutado sin el runner.
        direct = re.search(
            r"^\s*(?:exec\s+)?[\w./${}-]*" + re.escape(FOREIGN) + r"[\w./${}-]*",
            text,
            re.MULTILINE,
        )
        if direct and "run-target-binary.sh" not in text:
            offenders.append(path)
    assert not offenders, (
        f"hay scripts que ejecutan un binario de {FOREIGN} sin pasar por "
        f"run-target-binary.sh: {offenders}. Un binario de otro target tiene "
        f"que decir como se ejecuta, no esperar que binfmt_misc lo resuelva."
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
