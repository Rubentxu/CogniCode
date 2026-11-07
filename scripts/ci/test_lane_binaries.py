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

## Parte 3. Contratos de `preflight-clean-clone.sh`

Dos contratos que vigilan el mismo sujeto (`scripts/ci/preflight-clean-clone.sh`)
por sus dos costuras baratas, `--print-passed-verdict` y `--print-clone-ref`.
Se fusionaron en uno (TASK-74) conservando todas las comprobaciones.

# I. El ratchet de `passed` debe fallar cuando los tests desaparecen, no cuando crecen

MEDIDO 2026-10-03, certificando el tag v0.101.0. La batería en el clon limpio
dio `passed=5934 failed=0 ignored=30` contra un baseline de
`passed=5579 failed=0 ignored=37`, y el preflight lo rechazó:

    diff:     passed=355 failed=0 ignored=-7 (tolerancia ±2)
    FAIL: regresión de tests passed: delta=355 > tolerancia=2

Cero tests fallando, y el gate lo llamó regresión. La condición era

    if [ "${PASSED_DIFF#-}" -gt "$TOLERANCE" ]; then

`${PASSED_DIFF#-}` quita el signo, así que comparaba el **valor absoluto**: +355 y
-355 se trataban igual. El mensaje y la aritmética decían cosas distintas, y las
dos lecturas son defendibles por separado, que es exactamente el problema:

- Si la intención era "el número de tests no debería moverse mucho", el mensaje
  dice otra cosa — y en un repositorio cuya agenda declarada es añadir tests, un
  guard que falla cuando la suite crece está rojo por el trabajo que se le pidió
  hacer. El `ignored` cayendo de 37 a 30 encaja: siete tests que estaban
  `#[ignore]` se ejecutan ahora.
- Si la intención era "no perdemos tests", el fallo depende solo de tests que
  desaparecen, y `OBSERVED_FAILED > 0` cubre el otro caso dos líneas más abajo.

Se conserva la lectura que el mensaje describe. Una regresión es que el número de
tests **baje**.

## Lo que este contrato NO hace

No re-baselinea. El baseline es del operador, y el propio script lo dice
(`PREFLIGHT_BASELINE_*`, "requiere re-baseline explícito"). Con un baseline por
debajo de la realidad este ratchet queda débil: una caída grande de la suite real
seguiría dentro de tolerancia mientras el total no baje del baseline. Arreglar el
signo no arregla eso, y este contrato no finge que sí — sólo comprueba la
dirección del guard.

# II. El preflight debe sobrevivir a ejecutarse desde un checkout de tag

MEDIDO 2026-10-03. La lane `release-candidate` callmurio en 0.3s al certificar
el tag v0.101.0, en el stage `clean-clone`, con:

    fatal: Rama remota HEAD no encontrada en upstream origin

La causa era una sola linea, y la linea tenia un fallback escrito para cubrir
justo el caso que no cubria:

    --branch "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo main)"

En un checkout detached — que es lo que se obtiene al hacer `git checkout
<tag>`, o sea como se corta una release — `git rev-parse --abbrev-ref HEAD`
imprime la cadena literal `HEAD` y **sale con codigo 0**. El `|| echo main`
depende de que el comando falle, y no falla. Asi que se pasaba `--branch HEAD` a
`git clone`, que busca una rama llamada `HEAD`, no la encuentra, y muere.

Que una release no se pueda certificar desde un tag no es un defecto de entorno:
es el caso normal de este gate. Un gate que solo se puede ejecutar desde una rama
es un gate que no se puede usar para publicar.

## Por que un contrato y no un comentario

El comentario anterior ya explicaba la intencion del fallback y era correcto, y
el gate seguia roto. Un fallback cuya condicion de disparo nunca se cumple no es
un fallback: es prosa que parece una guarda. Este contrato comprueba la
propiedad, que es la que importa: **la referencia que el clon va a nombrar tiene
que existir, en cualquier estado del checkout.**

## Que se comprueba

- Desde una rama, la resolucion devuelve el nombre de la rama. Sin esto, la
  correccion mas simple —devolver siempre vacio— dejaria el clon sin
  optimizacion y pasaria igual.
- Desde un checkout detached, no devuelve nada, y en particular no devuelve la
  cadena `HEAD`.
- Lo que la resolucion devuelve se puede pasar a `git clone --branch`. Esto es
  la propiedad de extremo a extremo: la resolucion y el comando que la consume,
  juntos.
- La regla esta escrita una sola vez. El stage 1 llama a `resolve_clone_ref`, y
  no hay una segunda resolucion `--abbrev-ref` repartida por el script: dos
  copias de la regla es una de ellas equivocada dentro de un mes.
- El seam es barato. `--print-clone-ref` existe para que este contrato no
  tenga que pagar los 8-15 minutos del preflight. Si alguien lo quita, o el
  script lo ejecuta entero antes de responder, este test pasaria a tardar 15
  minutos y a dejar de vigilarse. Se mide el tiempo.

Run:
    python3 scripts/ci/test_preflight_contracts.py
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


# --- Parte 3: preflight-clean-clone.sh (ratchet de `passed` y referencia del clon) ---

SCRIPT = REPO_ROOT / "scripts" / "ci" / "preflight-clean-clone.sh"
PASSED_SEAM = "--print-passed-verdict"
CLONE_REF_SEAM = "--print-clone-ref"

# El baseline que encontró el fallo, y el observado real de aquel certifying run.
MEASURED_BASELINE = 5579
MEASURED_OBSERVED = 5934

# El preflight completo declara 8-15 minutos en su cabecera. Un seam tiene que
# ser mucho mas barato que eso, y con holgura: si el limite se acerca al
# documento, el seam ha dejado de ser un seam.
SEAM_BUDGET_SECONDS = 30

# Y lo que cuesta cuando el seam NO existe. Sin el seam, el script interpreta la
# bandera como un SHA, sigue por su curso y clona el repositorio entero de
# verdad: medido, un contrato en rojo encendia 5 clones de 1.6G y sus logs en
# /tmp antes de que este test pudiera decir nada. Con el seam ausente se falla
# por inspeccion, sin ejecutar nada.
PROBE_TIMEOUT_SECONDS = 20

def script_code() -> str:
    """The script's executable lines, comments removed.

    A comment that quotes a command is indistinguishable from code that runs it,
    in both directions — a comment can satisfy an assertion about behaviour, and
    can break one about text. Same reason `pipeline_authority.strip_line_comments`
    exists.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def verdict(observed: int, baseline: int = MEASURED_BASELINE) -> str:
    """'OK', 'REGRESSION', or 'PASSED_SEAM-ABSENT'.

    The seam is checked by inspection first. Without that, a script that lacks
    it reads the flag as a SHA and runs the whole preflight — a real clone of
    this repository, twenty minutes of build — before the contract could say
    anything. A contract whose red state costs more than the gate it watches is a
    contract nobody runs on a PR.
    """
    code = script_code()
    if PASSED_SEAM not in code:
        return "PASSED_SEAM-ABSENT"
    if f"passed_count_regressed()" not in code:
        return "PASSED_SEAM-ABSENT"

    started = time.monotonic()
    try:
        done = subprocess.run(
            ["bash", str(SCRIPT), PASSED_SEAM, str(observed)],
            cwd=str(REPO_ROOT),
            capture_output=True,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
            env={
                **dict(__import__("os").environ),
                "PREFLIGHT_BASELINE_PASSED": str(baseline),
                "PREFLIGHT_TOLERANCE": "2",
            },
        )
    except subprocess.TimeoutExpired:
        raise AssertionError(
            f"{PASSED_SEAM} did not answer within {PROBE_TIMEOUT_SECONDS}s. The seam "
            "exists so this contract does not have to run a clean clone; if it "
            "runs one, this test silently becomes a 20-minute test"
        ) from None
    elapsed = time.monotonic() - started
    if elapsed > SEAM_BUDGET_SECONDS:
        raise AssertionError(f"{PASSED_SEAM} took {elapsed:.1f}s, over the seam budget")

    first = done.stdout.strip().splitlines()[-1] if done.stdout.strip() else ""
    if first not in ("OK", "REGRESSION"):
        raise AssertionError(
            f"{PASSED_SEAM} printed {first!r} instead of OK|REGRESSION.\n"
            f"stdout: {done.stdout!r}\nstderr: {done.stderr!r}"
        )
    return first


def test_more_passing_tests_is_not_a_regression() -> None:
    """The regression, on the numbers that caused it.

    355 more tests passing, zero failing, is the direction this repository has
    been moving on purpose.
    """
    got = verdict(MEASURED_OBSERVED)
    assert got == "OK", (
        f"growing the suite by {MEASURED_OBSERVED - MEASURED_BASELINE} passing "
        f"tests is reported as {got}. The guard compares the absolute value, so "
        "a project that adds tests is gated by the work it was asked to do. A "
        "regression is tests disappearing, not tests arriving"
    )


def test_fewer_passing_tests_is_a_regression() -> None:
    """The other direction still gates, or the fix is just 'never fail'."""
    lost = 355
    got = verdict(MEASURED_BASELINE - lost)
    assert got == "REGRESSION", (
        f"losing {lost} passing tests is reported as {got}. Fixing the sign must "
        "not have removed the ratchet; a guard that cannot fail is a file, not a "
        "gate"
    )


def test_movement_within_tolerance_passes_in_both_directions() -> None:
    """Churn is allowed, which is what the tolerance was for."""
    for observed, why in (
        (MEASURED_BASELINE + 2, "two more"),
        (MEASURED_BASELINE + 1, "one more"),
        (MEASURED_BASELINE - 1, "one fewer"),
        (MEASURED_BASELINE - 2, "two fewer"),
    ):
        got = verdict(observed)
        assert got == "OK", f"{why} passing test(s) is {got}, not OK"


def test_just_past_tolerance_fails_in_the_losing_direction_only() -> None:
    """The boundary is where the tolerance says it is."""
    assert verdict(MEASURED_BASELINE - 3) == "REGRESSION", (
        "three fewer passing tests is inside the stated tolerance of 2"
    )
    assert verdict(MEASURED_BASELINE + 3) == "OK", (
        "three more passing tests is still not a regression"
    )


def test_the_failure_message_describes_the_direction_it_checks() -> None:
    """The message and the arithmetic must agree.

    This is the defect itself: `regresión de tests passed` was printed for a
    growth. Whatever the guard checks, the sentence a human reads at 3am has to
    name the same direction.
    """
    code = script_code()
    assert "regresión de tests passed" not in code, (
        "the old two-sided guard's message is still in the code. It called a "
        "growth a regression, and that contradiction is the bug this contract "
        "exists for. (Checked against code, not the whole file: the comment "
        "above the function quotes that sentence on purpose, and a check that "
        "fails on its own documentation is a check that gets deleted)"
    )
    assert "por debajo del baseline" in code, (
        "the guard must name the direction it actually checks, so the message "
        "and the comparison describe the same property"
    )
    assert "re-baseline" in code, (
        "the failure must say what to do when the drop was intentional, "
        "including the variables that re-baseline it. A guard that fails "
        "without saying how to proceed is a gate people route around"
    )


def test_the_rule_is_defined_once_and_called_by_the_comparison() -> None:
    """One definition, one caller.

    The rule was inline in stage 6; making it a function is what lets a contract
    reach it, and a function nobody calls is a function that no longer describes
    the gate.
    """
    code = script_code()
    assert code.count("passed_count_regressed()") == 1, (
        "the function is defined more than once, so the copy the contract reads "
        "and the copy stage 6 calls can drift"
    )
    assert 'if passed_count_regressed "$BASELINE_PASSED" "$OBSERVED_PASSED"' in code, (
        "stage 6 no longer calls the shared function, so the rule under test and "
        "the rule the preflight runs could disagree"
    )
    # The absolute-value idiom is the bug itself and must not come back.
    assert "PASSED_DIFF#-" not in code, (
        "'${PASSED_DIFF#-}' strips the sign and makes the check two-sided again"
    )


def test_the_seam_answers_in_bounded_time() -> None:
    """Cheap in green, so it can be run on a PR.

    Asserted here as its own test rather than only inside `verdict` so that a
    future change to the seam's cost is a named failure rather than a slow suite
    somebody starts skipping.
    """
    started = time.monotonic()
    got = verdict(MEASURED_OBSERVED)
    elapsed = time.monotonic() - started
    assert got != "PASSED_SEAM-ABSENT", (
        f"{PASSED_SEAM} is not implemented in the script's code. The comparison is "
        "otherwise only reachable after a full clean-clone preflight, which "
        f"means it is untested; {CLONE_REF_SEAM} is the pattern to copy"
    )
    assert elapsed < SEAM_BUDGET_SECONDS, f"the seam took {elapsed:.1f}s"


def git(cwd: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(cwd), *args], capture_output=True, text=True, timeout=120
    )


def make_repo(root: Path) -> Path:
    """A repo with one commit, one tag and one branch."""
    subprocess.run(
        ["git", "init", "-q", "-b", "main", str(root)],
        check=True,
        capture_output=True,
        timeout=120,
    )
    git(root, "-c", "user.email=contract@example.invalid", "-c", "user.name=c",
        "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "x")
    git(root, "tag", "v1")
    return root


def resolve_clone_ref(cwd: Path) -> str:
    """What the script would pass to `git clone --branch`, from `cwd`.

    Runs the script itself rather than reimplementing the rule: a contract that
    reimplemented the thing it checks would keep passing after the thing broke.

    The seam is verified by inspection first. Without that check, an unfixed
    script reads `--print-clone-ref` as a SHA and runs the whole preflight —
    including a real clone of this repository — so the contract's red state
    would be the most expensive thing in the suite.
    """
    source = SCRIPT.read_text(encoding="utf-8")
    code = "\n".join(
        line for line in source.splitlines() if not line.lstrip().startswith("#")
    )
    if CLONE_REF_SEAM not in code:
        raise AssertionError(
            f"the script does not implement {CLONE_REF_SEAM}, so its ref resolution cannot "
            "be exercised without running the preflight end to end. Add the seam "
            "next to resolve_clone_ref; without it this contract either clones "
            "the repository to find out it is broken, or stops testing the rule"
        )

    started = time.monotonic()
    try:
        done = subprocess.run(
            ["bash", str(SCRIPT), CLONE_REF_SEAM],
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=PROBE_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        raise AssertionError(
            f"{CLONE_REF_SEAM} did not answer within {PROBE_TIMEOUT_SECONDS}s. The seam is "
            "meant to print one ref and exit; if it runs the preflight first, "
            "this contract silently becomes a 15-minute test"
        ) from None
    elapsed = time.monotonic() - started
    if done.returncode != 0:
        raise AssertionError(
            f"the script does not answer {CLONE_REF_SEAM} (exit {done.returncode}). Either "
            f"the seam was removed or the script fails before reaching it.\n"
            f"stdout: {done.stdout!r}\nstderr: {done.stderr!r}"
        )
    if elapsed > SEAM_BUDGET_SECONDS:
        raise AssertionError(
            f"{CLONE_REF_SEAM} took {elapsed:.1f}s, over the {SEAM_BUDGET_SECONDS}s budget. "
            "The seam is supposed to answer the ref resolution without running "
            "the preflight; if it runs the preflight, this contract silently "
            "becomes a 15-minute test and stops being run"
        )
    return done.stdout.strip()


def test_a_detached_checkout_names_no_branch() -> None:
    """The regression: a tag checkout must not hand `HEAD` to the clone.

    This is the exact state a release cut lives in, and the exact state that
    killed the lane.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(Path(raw))
        git(root, "checkout", "-q", "--detach", "v1")
        assert git(root, "rev-parse", "--abbrev-ref", "HEAD").stdout.strip() == "HEAD", (
            "the fixture is not a detached checkout, so this test would pass "
            "without exercising anything"
        )
        ref = resolve_clone_ref(root)
        assert ref != "HEAD", (
            "a detached checkout still resolves to the literal 'HEAD', which "
            "git clone rejects with 'Remote HEAD branch not found'. The "
            "`|| echo main` fallback never fires because --abbrev-ref exits 0"
        )
        assert ref == "", (
            f"a detached checkout should name no branch at all, got {ref!r}"
        )


def test_a_branch_checkout_still_names_its_branch() -> None:
    """The fix is not "always return nothing".

    Dropping `--branch` unconditionally would make the two failing cases pass and
    cost a full default-branch clone on every run. This is the assertion that
    keeps the optimisation alive.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(Path(raw))
        ref = resolve_clone_ref(root)
        assert ref == "main", (
            f"a checkout on branch 'main' resolved to {ref!r}; the branch name "
            "is the whole point of the optimisation and it must survive the fix"
        )


def test_the_named_ref_is_clonable() -> None:
    """End to end: whatever the resolver returns, the clone accepts.

    The two unit tests above pin the resolver. This one pins the pair, which is
    the property that actually failed: a ref that is not clonable, from any
    state, is a broken release path.
    """
    for state, setup in (
        ("on a branch", ["checkout", "-q", "main"]),
        ("detached at a tag", ["checkout", "-q", "--detach", "v1"]),
    ):
        with tempfile.TemporaryDirectory() as raw:
            root = make_repo(Path(raw) / "src")
            git(root, *setup)
            ref = resolve_clone_ref(root)
            args = ["--no-tags"]
            if ref:
                args += ["--branch", ref]
            with tempfile.TemporaryDirectory() as dst:
                done = subprocess.run(
                    ["git", "clone", "-q", *args, str(root), f"{dst}/clone"],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                assert done.returncode == 0, (
                    f"cloning a repository {state} failed with ref={ref!r}:\n"
                    f"{done.stderr.strip()}"
                )


def test_the_rule_is_written_once() -> None:
    """One definition, called by the stage that clones.

    Two copies of a resolution rule is one of them wrong inside a month, and the
    symptom would be a release path that breaks again the day someone edits the
    copy that is not the one the contract reads.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    assert "resolve_clone_ref()" in text, (
        "the resolver function is gone; the stage and the seam must share one "
        "definition rather than each inlining the rule"
    )
    assert 'clone_ref="$(resolve_clone_ref)"' in text, (
        "the clone stage no longer calls the shared resolver, so the rule the "
        "contract exercises and the rule the clone uses could disagree"
    )
    # One definition, one call site — counted in code, not in prose. The comment
    # above the resolver deliberately quotes the old line verbatim so the reason
    # for the change survives; counting that would make the test fail on its own
    # documentation, which is how a linter ends up being deleted instead of fixed.
    # A comment that quotes a command is indistinguishable from code that runs it,
    # in both directions — the same reason `pipeline_authority.strip_line_comments`
    # exists.
    code = "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )
    occurrences = code.count("--abbrev-ref")
    assert occurrences == 1, (
        f"--abbrev-ref appears {occurrences} times in executable code; the "
        "detached-HEAD rule is written in more than one place, so the copy the "
        "contract reads and the copy the clone uses can drift apart"
    )


def test_the_branch_flag_is_only_passed_when_a_branch_exists() -> None:
    """The clone must not be handed an empty `--branch`.

    A bash array expanded unguarded under `set -u` can yield an empty first
    argument, which `git clone` reads as the branch name. The guarded expansion
    is what makes the omitted case safe, and it is worth pinning.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    assert "clone_branch_args=()" in text, (
        "the clone no longer builds its --branch arguments conditionally"
    )
    guarded = '${clone_branch_args[@]+"${clone_branch_args[@]}"}'
    assert guarded in text, (
        f"the array is expanded unguarded. Under `set -u`, {guarded} is what "
        "keeps an omitted --branch from being passed as an empty branch name"
    )
    assert '--branch "$clone_ref"' in text, (
        "the --branch flag is no longer derived from the resolved ref"
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
        except Exception as exc:  # noqa: BLE001 - un contrato que lanza es un fallo, no un aborto
            failures += 1
            print(f"  FAIL {name}")
            print(f"       raised {type(exc).__name__}: {exc}")
        else:
            print(f"  PASS {name}")
    print(f"  -> {len(tests) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
