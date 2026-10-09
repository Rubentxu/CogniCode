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


def make_tree(directory: str) -> tuple[Path, Path]:
    """Un arbol de mentira: un crate con un `.rs` y un binario junto."""
    root = Path(directory) / "repo"
    crate = root / "crates" / "thing"
    crate.mkdir(parents=True)
    source = crate / "lib.rs"
    source.write_text("fn main() {}\n", encoding="utf-8")
    binary = root / "target" / "release" / "thing"
    binary.parent.mkdir(parents=True)
    binary.write_text("#!/bin/sh\n", encoding="utf-8")
    binary.chmod(0o755)
    return root, binary


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
        root, binary = make_tree(raw)
        # El binario es mas nuevo que la fuente: el caso normal.
        now = time.time()
        os.utime(binary, (now, now))
        done = run_check(binary, root)
    assert done.returncode == 0, done.stdout + done.stderr


def test_a_binary_older_than_a_source_is_rejected() -> None:
    """El caso medido: un binario de otro checkout, mas nuevo en reloj pero mas
    viejo de contenido."""
    with tempfile.TemporaryDirectory() as raw:
        root, binary = make_tree(raw)
        old = time.time() - 3600
        os.utime(binary, (old, old))
        done = run_check(binary, root)
    assert done.returncode != 0, done.stdout
    combined = done.stdout + done.stderr
    assert "CARGO_TARGET_DIR" in combined, (
        f"el mensaje tiene que decir como se arregla, no solo que pasa.\n{combined}"
    )


def test_a_missing_binary_is_not_a_pass() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root, binary = make_tree(raw)
        binary.unlink()
        done = run_check(binary, root)
    assert done.returncode != 0, "un binario ausente no es un binario fresco"
    assert "no existe" in (done.stdout + done.stderr)


def test_the_release_tool_stage_checks_the_tool_it_just_built() -> None:
    """Si la stage no lo llama, la comprobacion no protege nada."""
    text = CANDIDATE.read_text(encoding="utf-8")
    start = text.index('stage("release-tool")')
    nxt = text.find('stage("', start + 10)
    body = text[start : nxt if nxt != -1 else len(text)]
    assert "check-built-binary.sh" in body, (
        "la stage que construye la herramienta de release tiene que comprobar "
        "que es de este arbol: un target dir compartido entre checkouts puede "
        "dejarle una herramienta de otro, y el fallo sale veinte minutos "
        "despues, en otra stage, blaming un subcomando que si existe"
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
