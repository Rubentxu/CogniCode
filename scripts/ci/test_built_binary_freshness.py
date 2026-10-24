#!/usr/bin/env python3
"""Que una lane certificara con binarios de SU arbol, no de otro checkout.

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
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECK = REPO_ROOT / "scripts" / "ci" / "check-built-binary.sh"
CANDIDATE = REPO_ROOT / "release-candidate.pipeline.kts"


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


def test_the_release_tool_stage_checks_the_tool_it_just_built() -> None:
    """Si la stage no lo llama, la comprobacion no protege nada."""
    text = CANDIDATE.read_text(encoding="utf-8")
    start = text.index('stage("release-tool")')
    nxt = text.find('stage("', start + 10)
    body = text[start : nxt if nxt != -1 else len(text)]
    assert "check-built-binary.sh" in body, (
        "la stage que construye la herramienta de release tiene que comprobar "
        "que es de este arbol"
    )
    assert "cognicode-release" in body, (
        "la comprobacion tiene que mirar el binario que la stage acaba de "
        "construir, no otro"
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
