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


def make_tree(directory: str) -> tuple[Path, Path, Path]:
    """Un arbol de mentira: las fuentes que construyen el binario, y el binario."""
    root = Path(directory) / "repo"
    crate = root / "crates" / "thing"
    crate.mkdir(parents=True)
    source = crate / "lib.rs"
    source.write_text("fn main() {}\n", encoding="utf-8")
    # Un fichero del mismo crate que NO entra en este binario. Existe para
    #MEDIDO: con el barrido por `crates/` que hacia la primera version, este
    # fichero hacia fallar la stage con un binario perfectamente fresco.
    unrelated = crate / "otro.rs"
    unrelated.write_text("pub fn otro() {}\n", encoding="utf-8")
    binary = root / "target" / "release" / "thing"
    binary.parent.mkdir(parents=True)
    binary.write_text("#!/bin/sh\n", encoding="utf-8")
    binary.chmod(0o755)
    return root, source, unrelated


def run_check(binary: Path, root: Path, *sources: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", str(CHECK), str(binary), str(root), *[str(s) for s in sources]],
        capture_output=True,
        text=True,
        timeout=120,
    )


def test_the_check_exists() -> None:
    assert CHECK.is_file(), f"no existe {CHECK}"
    assert os.access(CHECK, os.X_OK), f"{CHECK} no es ejecutable"


def test_a_binary_built_from_these_sources_passes() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        now = time.time()
        os.utime(binary, (now, now))
        os.utime(source, (now - 10, now - 10))
        done = run_check(binary, root, source)
    assert done.returncode == 0, done.stdout + done.stderr


def test_a_source_that_is_not_named_cannot_trip_it() -> None:
    """MEDIDO 2026-10-03: la guarda de la primera version comparaba contra
    todos los `.rs` de `crates/` y rechazaba la stage porque habia cambiado
    `ide.rs`, que no entra en `cognicode-release`. Un guard que rechaza lo que
    no debe entrena a ignorar su veredicto."""
    with tempfile.TemporaryDirectory() as raw:
        root, source, unrelated = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        now = time.time()
        # El binario es fresco respecto a lo que lo construye, y `unrelated`
        # es mas nuevo que el: la stage tiene que pasar igual.
        os.utime(binary, (now, now))
        os.utime(source, (now - 10, now - 10))
        os.utime(unrelated, (now + 60, now + 60))
        done = run_check(binary, root, source)
    assert done.returncode == 0, (
        f"un fichero que no construye el binario no puede rechazarlo.\n"
        f"{done.stdout}{done.stderr}"
    )


def test_a_binary_older_than_a_source_is_rejected() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        old = time.time() - 3600
        os.utime(binary, (old, old))
        done = run_check(binary, root, source)
    assert done.returncode != 0, done.stdout
    combined = done.stdout + done.stderr
    assert "CARGO_TARGET_DIR" in combined, (
        f"el mensaje tiene que decir como se arregla, no solo que pasa.\n{combined}"
    )


def test_naming_no_sources_fails_closed() -> None:
    """Una guarda a la que no se le dice que vigilar no vigila nada."""
    with tempfile.TemporaryDirectory() as raw:
        root, _, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        now = time.time()
        os.utime(binary, (now, now))
        done = run_check(binary, root)
    assert done.returncode != 0, (
        "sin fuentes nombradas la guarda tiene que negarse a dar un veredicto"
    )


def test_a_named_source_that_does_not_exist_fails() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, _, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        done = run_check(binary, root, Path("crates/thing/no_existe.rs"))
    assert done.returncode != 0, "una fuente que no existe no puede probar frescura"
    assert "no existe" in (done.stdout + done.stderr)


def test_a_missing_binary_is_not_a_pass() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, source, _ = make_tree(raw)
        binary = root / "target" / "release" / "thing"
        binary.unlink()
        done = run_check(binary, root, source)
    assert done.returncode != 0, "un binario ausente no es un binario fresco"
    assert "no existe" in (done.stdout + done.stderr)


def test_the_release_tool_stage_names_the_sources_it_checks() -> None:
    """Si la stage no lo llama, la comprobacion no protege nada; y si no nombra
    las fuentes correctas, deja de proteger justo lo que queria."""
    text = CANDIDATE.read_text(encoding="utf-8")
    start = text.index('stage("release-tool")')
    nxt = text.find('stage("', start + 10)
    body = text[start : nxt if nxt != -1 else len(text)]
    assert "check-built-binary.sh" in body, (
        "la stage que construye la herramienta de release tiene que comprobar "
        "que es de este arbol"
    )
    for source in (
        "crates/cognicode-cli/src/bin/release.rs",
        "crates/cognicode-cli/src/cmd/release_contract.rs",
        "crates/cognicode-cli/src/cmd/release_factory.rs",
    ):
        assert source in body, (
            f"la guarda tiene que mirar {source}: es una de las fuentes que "
            f"construyen la herramienta, y es de donde salio el subcomando "
            f"`skills` que la lane no encontraba"
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
