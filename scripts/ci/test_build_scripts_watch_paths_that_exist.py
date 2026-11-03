#!/usr/bin/env python3
"""Contrato: un build.rs no vigila rutas de git que un worktree no tiene.

## Lo que esta en juego

`cargo:rerun-if-changed=<ruta>` sobre una ruta que NO existe hace que Cargo
vuelva a ejecutar el script en cada invocacion, y con el script, que
recompile el crate. En un worktree `.git` es un fichero, no un directorio, asi
que `../../.git/HEAD` no existe: dos `cargo build` identicos seguidos
recompilaban `cognicode-core` los dos (58 s y 52 s medidos). El gate corre
varias veces `cognicode-core` con features distintas, y cada una pagaba esa
recompilacion de mas.

La ruta correcta la dice git (`git rev-parse --git-path HEAD` y
`--git-common-dir`), y solo se vigila si existe.

Run:
    python3 scripts/ci/test_build_scripts_watch_paths_that_exist.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
BUILD_SCRIPTS = ("crates/cognicode-core/build.rs", "crates/cognicode-cli/build.rs")

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def watched_literals(text: str) -> list[str]:
    live = "\n".join(l for l in text.splitlines() if not l.strip().startswith("//"))
    return re.findall(r'cargo:rerun-if-changed=([^"{}]+)"', live)


def test_no_script_hardcodes_a_dot_git_directory() -> None:
    for rel in BUILD_SCRIPTS:
        text = (REPO / rel).read_text(encoding="utf-8")
        bad = [p for p in watched_literals(text) if ".git" in p]
        check(not bad, f"{rel} vigila {bad}: en un worktree esa ruta no existe y Cargo recompila siempre")


def test_the_detector_sees_a_planted_hardcoded_path() -> None:
    planted = 'println!("cargo:rerun-if-changed=../../.git/HEAD");'
    check(watched_literals(planted) == ["../../.git/HEAD"], "el detector no ve una ruta fija plantada")


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("PASS — ningun build.rs vigila una ruta .git fija que un worktree no tiene")
    return 0


if __name__ == "__main__":
    sys.exit(main())
