#!/usr/bin/env python3
"""Repository hygiene: what the index must not carry.

Two contracts over the same subject, the versioned tree, merged in one file.
Both read `git ls-files`, both fail closed when the index cannot be read, and
neither is satisfied by an empty listing.

## Part 1: no unresolved merge markers

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

## Part 2: no PipelineK run state tracked


Measured 2026-10-15. A run launched with `--control-root "$PWD"` instead of the
default `<workspace>/.pipelinek` writes its control state into the working
tree: `leases/`, `last-run/`, `output-plane/`, `retry-control/`,
`wait-until-control/`. A `cargo fmt --all` commit in this repository's history
swept 32 of those files into version control, and they have been tracked ever
since.

Nothing notices. `git status` shows the *new* ones as untracked noise and the
tracked ones as modified on every run, and a reviewer reads that churn as
someone else's WIP. Meanwhile two of the tracked files are `*.lock` files —
a lock whose purpose is to vanish with the process holding it, preserved in a
release tarball — and the `output-plane/streams/` set is per-run stdout, which
is exactly the kind of content that should never be published.

The oracle is `git ls-files`, because that is what names the content of a
release tarball. A directory can be absent from the working tree and still be
tracked; only the index knows.

Run:
    python3 scripts/ci/test_repo_hygiene.py
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path, PurePosixPath

REPO_ROOT = Path(__file__).resolve().parents[2]

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


# --- Part 1: merge markers ---------------------------------------------------

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


# --- Part 2: PipelineK run state ---------------------------------------------

# Written by the runtime into the control root. `.pipelinek/` is already
# ignored and is the default location; these five are what appears when the
# control root is pointed at the repository instead.
CONTROL_ROOT_DIRS = (
    "leases",
    "last-run",
    "output-plane",
    "retry-control",
    "wait-until-control",
)


def tracked_files() -> list[str]:
    proc = subprocess.run(
        ["git", "ls-files", "-z", "--", *CONTROL_ROOT_DIRS],
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
        check=False,
    )
    if proc.returncode != 0:
        failures.append(
            f"git ls-files failed: {proc.stderr.strip()[:200]}. This contract "
            f"cannot tell what is tracked, so it must not report a pass."
        )
        return []
    return [name for name in proc.stdout.split("\0") if name]


def test_no_control_root_state_is_tracked() -> None:
    tracked = tracked_files()
    if tracked:
        failures.append(
            f"{len(tracked)} PipelineK control-root file(s) are tracked and "
            f"would ship in a release tarball:\n    "
            + "\n    ".join(tracked[:12])
            + ("\n    ..." if len(tracked) > 12 else "")
            + "\nThey are per-run scratch. Untrack them with "
            "`git rm -r --cached` and keep them out with .gitignore; the "
            "control root belongs at `.pipelinek`, not at the repository root."
        )


def test_the_control_root_state_is_ignored() -> None:
    """Untracked is not enough: the next run recreates them."""
    ignored = [
        d
        for d in CONTROL_ROOT_DIRS
        if subprocess.run(
            ["git", "check-ignore", "-q", f"{d}/probe"],
            cwd=REPO_ROOT,
            check=False,
        ).returncode
        == 0
    ]
    missing = [d for d in CONTROL_ROOT_DIRS if d not in ignored]
    check(
        not missing,
        f"{missing} are not in .gitignore. Untracking them fixes this commit; "
        f"without an ignore rule the next pipeline run puts them straight back "
        f"into `git status` as untracked noise.",
    )


# MEDIDO 2026-10-15. A second shape of the same leak, found because it nearly
# went into a commit: a run writes one directory per *step* at the repository
# root, named `<run-uuid>-s<index>-<step>/` and holding `script.sh`,
# `result.txt`, `wrapper.sh` and `script.sh.copy`. Two of them were sitting
# untracked in the working tree and `git add -A` swept them into a commit that
# was about pipelines — four files whose entire content is "how this run went".
#
# The five directories above are named by the runtime, so they can be listed and
# checked one by one. These cannot: the name contains the run id, so there is
# no fixed list. The oracle is therefore the index filtered by what the runtime
# writes inside such a directory, not a directory name.
STEP_SCRATCH = ("script.sh", "wrapper.sh", "result.txt", "script.sh.copy")


# El nombre del directorio que crea el runtime: `<run-uuid>-s<indice>-<step>`,
# con un `\n` entre el id del run y el del step. Se ancla en el ULTIMO
# componente del path para que un directorio real con esa forma en cualquier
# sitio cuente, no solo en la raiz.
STEP_DIR = re.compile(r"-s\d+-\d+$")


def test_no_per_step_scratch_is_tracked() -> None:
    """Any tracked file whose parent directory has the runtime's step shape.

    MEDIDO: la primera version filtraba con `git ls-files -- result.txt`, y un
    pathspec sin `:(glob)` solo casa con la raiz — asi que el fichero bajo
    `<dir>/` no lo encontraba y el contrato paso verde con el scratch
    trackeado. Un oráculo que no encuentra lo que busca no es un oraculo.

    Se lista el indice entero y se filtra por la FORMA del directorio padre,
    que es lo unico que se puede exigir sin conocer el id del run.
    """
    proc = subprocess.run(
        ["git", "ls-files", "-z"],
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
        check=False,
    )
    if proc.returncode != 0:
        failures.append(
            f"git ls-files failed: {proc.stderr.strip()[:200]}. This contract "
            f"cannot tell what is tracked, so it must not report a pass."
        )
        return
    everything = [name for name in proc.stdout.split("\0") if name]
    # Si el indice viniera vacio, la comprobacion de abajo pasaria sola. Que
    # el repo tenga ficheros es la precondicion minima para que "ninguno" sea
    # un resultado y no un fallo silencioso.
    check(
        len(everything) > 100,
        f"git ls-files devolvio {len(everything)} ficheros; el indice no se "
        f"puede leer, y un filtro sobre una lista vacia pasa siempre.",
    )
    tracked = [
        name
        for name in everything
        if PurePosixPath(name).parent.name
        and STEP_DIR.search(PurePosixPath(name).parent.name)
    ]
    check(
        not tracked,
        f"{len(tracked)} PipelineK per-step scratch file(s) are tracked and "
        f"would ship in a release tarball:\n    "
        + "\n    ".join(tracked[:8])
        + "\nThey are written by the durable-shell runtime into a per-step "
        "directory at the repository root. `git add -A` sweeps them in because "
        "nothing ignores them; untrack and ignore the directory shape, which "
        "carries the run id and so cannot be listed by name.",
    )


def test_a_step_directory_name_is_ignored() -> None:
    """The rule has to match a name nobody can predict.

    Probe uses the exact shape seen on disk, newlines and all: the runtime joins
    the run id and the step id with `\\n`, so the directory name really does
    contain one, and a rule written without `\\n` in mind misses.
    """
    probe = "aaaaaaaa-1111-2222-3333-444444444444\\nbbbbbbbb-5555-6666-7777-888888888888-s1-0/result.txt"
    ignored = (
        subprocess.run(
            ["git", "check-ignore", "-q", probe],
            cwd=REPO_ROOT,
            check=False,
        ).returncode
        == 0
    )
    check(
        ignored,
        "a PipelineK per-step directory name is not ignored. Every run leaves "
        "one per step at the repository root, so `git status` carries them as "
        "noise and `git add -A` will eventually commit a directory whose "
        "entire content is the transcript of one step.",
    )


def main() -> int:
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        "PASS — no unresolved merge marker in the versioned text, and no "
        "PipelineK run state tracked or left unignored"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
