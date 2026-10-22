#!/usr/bin/env python3
"""Contracts for the absence of unresolved merge markers in the tree.

## El hueco

Un fichero con marcadores de conflicto sin resolver es un fichero que nadie
puede leer: el texto viene en dos versiones y el lector tiene que adivinar
cuál vale. Cuando eso ocurre en el documento que dice cuál es el estado
actual, el estado que se lee no es ninguno.

MEDIDO 2026-10-22: `docs/roadmap/CURRENT.md` llevaba **seis** bloques de
marcadores de conflicto sin resolver, commitados en `main`. Un rebase dejó
las dos caras del texto y alguien commiteó el resultado tal cual. Nadie lo
vió porque nada miraba: ningún contrato buscaba marcadores, y el documento es
markdown, así que todo lo que lo leía leía las dos versiones.

## Lo que este contrato hace

Recorre los ficheros versionados de texto y busca los tres marcadores que
git escribe al dejar un conflicto sin resolver. El separador (`=======`) sólo
cuenta cuando está entre un inicio y un fin de bloque, porque una línea de
siete signos iguales es también un título de nivel uno en markdown, y un
contrato que se dispara con un título lawful no se lee dos veces.

El contrato es también su propio control: si no encuentra ficheros que
mirar, lo dice, en vez de dar un verde por no haber nada que comprobar.

## Por qué no basta con mirar el diff

Un conflicto se resuelve en el momento del rebase, y el fichero resultante se
commitea sin que el diff de ese commit muestre nada raro: los marcadores son
líneas de texto válidas. El único sitio donde se ven es el fichero, y el
fichero es lo que se commitea.

Run:
    python3 scripts/ci/test_no_conflict_markers.py
"""

from __future__ import annotations

import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Los marcadores se arman con trozos para que este fichero no los contenga:
# una guarda que se dispara con su propio fuente es una guarda que nadie
# se cree.
CONFLICT_START = "<" * 7
CONFLICT_END = ">" * 7
CONFLICT_SEPARATOR = "=" * 7

# Los ficheros que se miran son los de texto: un binario no lleva marcadores
# y leerlo como UTF-8 solo produce ruido.
TEXT_SUFFIXES = frozenset(
    {
        ".md",
        ".rs",
        ".py",
        ".sh",
        ".kts",
        ".toml",
        ".json",
        ".yaml",
        ".yml",
        ".ts",
        ".tsx",
        ".txt",
    }
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def tracked_text_files() -> list[Path]:
    """The versioned files whose contents can carry a conflict marker."""
    listed = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "ls-files", "-z"],
        capture_output=True,
        text=True,
        check=False,
    )
    if listed.returncode != 0:
        return []
    paths: list[Path] = []
    for raw in listed.stdout.split("\0"):
        if not raw:
            continue
        candidate = REPO_ROOT / raw
        if candidate.suffix.lower() in TEXT_SUFFIXES and candidate.is_file():
            paths.append(candidate)
    return paths


def markers_in(text: str) -> list[tuple[int, str]]:
    """The lines of a text that carry an unresolved conflict marker.

    The separator only counts inside a block: a line of seven equal signs is
    also a level-one heading in markdown, and a contract that fires on a
    lawful heading is a contract nobody reads twice.
    """
    found: list[tuple[int, str]] = []
    inside = False
    for number, line in enumerate(text.splitlines(), start=1):
        stripped = line.rstrip()
        if stripped.startswith(CONFLICT_START):
            inside = True
            found.append((number, "inicio de conflicto"))
            continue
        if stripped.startswith(CONFLICT_END):
            inside = False
            found.append((number, "fin de conflicto"))
            continue
        if inside and stripped == CONFLICT_SEPARATOR:
            found.append((number, "separador de conflicto"))
    return found


def test_no_file_carries_an_unresolved_merge_marker() -> None:
    """A file with two versions of the same text is a file nobody can read."""
    files = tracked_text_files()
    check(
        bool(files),
        "no versioned text file could be listed, so the contract would pass "
        "without having looked at anything",
    )
    offenders: list[str] = []
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for number, kind in markers_in(text):
            offenders.append(f"{path.relative_to(REPO_ROOT)}:{number} ({kind})")
    check(
        not offenders,
        "these files carry unresolved merge markers, so the reader gets two "
        f"versions of the same text: {offenders}",
    )


def test_the_control_reads_a_file_with_markers() -> None:
    """The search has to find markers when they are there.

    Without this, a search that stopped matching anything would report a
    clean tree and be believed.
    """
    sample = (
        f"{CONFLICT_START} HEAD\n"
        "one version\n"
        f"{CONFLICT_SEPARATOR}\n"
        "another version\n"
        f"{CONFLICT_END} una rama\n"
    )
    found = markers_in(sample)
    check(
        len(found) == 3,
        f"a conflict block has to be read as three markers, found {len(found)}",
    )

    # And a level-one heading in markdown is not a conflict.
    heading = f"Título\n{CONFLICT_SEPARATOR}\n"
    check(
        not markers_in(heading),
        "a heading underlined with equal signs is not a conflict marker",
    )


def main() -> int:
    for name, value in sorted(globals().items()):
        if name.startswith("test_") and callable(value):
            value()
    if failures:
        for message in failures:
            print("FAIL " + message)
        print("merge markers: %d finding(s)" % len(failures))
        return 1
    print("merge markers: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
