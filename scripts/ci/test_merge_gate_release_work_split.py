#!/usr/bin/env python3
"""The release lane must not lose the build just because the gate has one too.

CORREGIDO 2026-10-15. Este módulo afirmaba que el merge gate no debe hacer
builds de perfil release, y lo escribí al moverlos a la lane de candidate. La
afirmación era falsa y el propio docstring lo decía sin verlo:

    "A merge no longer proves that the release binaries build ... One class of
     failure is detected later."

Eso no era un coste aceptable, era un conflicto con un contrato anterior que
no se había leído al cambiar nada:

    cli_gate_coverage_contract::the_gate_provides_what_the_release_flow_
    suites_require
    "the merge gate runs suites that require real release-profile binaries
     (cogh, cognicode, cognicode-release) but never builds them"

Cuatro suites UAT de la CLI resuelven sus binarios por `target/release/`. El
gate las ejecuta. Quitar el build no quita trabajo: quita la precondición de
esas cuatro suites y las convierte en un fallo que depende de lo que haya por
casualidad en la máquina. MEDIDO: con el build fuera, el contrato de cobertura
lleva rojo desde `e987d389`, y su propio mensaje dice que *"locally this is
masked by whatever release binaries happen to exist on the developer's
machine"*.

Un fallo que aparece en una máquina y no en otra no es un gate lento. Es un
gate que no dice lo que dice.

Así que los cuatro builds vuelven al merge gate. MEDIDO en frío, con el target
dir vacío: 358 + 123 + 174 + 38 = 693 s. En un runner limpio es ese el coste
de que cuatro UAT significen algo. La lane de release LOS TIENE TAMBIÉN, y eso
sigue siendo lo que se comprueba aquí: quitar del gate no es excusa para
quitarlos de allí.

MEDIDO además: el colapso de 73 etapas a 8 que acompaña a este cambio costó
124 ms por etapa, 8,9 s en total. No era la fuente del problema.

Lo que queda en este módulo son las dos mitades que siguen siendo verdaderas:
la lane de release construye lo que tiene que construir, y no ha perdido su
propia etapa de build al mover cosas.

Run:
    python3 scripts/ci/test_merge_gate_release_work_split.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Sibling contract, same directory. Imported, not copied: a second
# implementation would be a second source of truth for the same rule.
from test_candidate_packaging import strip_comments

REPO_ROOT = Path(__file__).resolve().parents[2]
MERGE_GATE = REPO_ROOT / "merge-gate.pipeline.kts"
RELEASE_LANE = REPO_ROOT / "release-candidate.pipeline.kts"

# What a merge must not build, and what a release must.
RELEASE_ONLY_IN_GATE = (
    (r"cargo\s+build\s+--release", "a release-profile build"),
    (r"cargo\s+doc\s+--workspace", "the workspace documentation build"),
)
REQUIRED_IN_LANE = (
    ("host-build-and-verify", "the host binary build"),
    ("public-docs", "the zero-warning documentation build"),
    ("cognicode-control-plane", "the control-plane host binary"),
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def code_of(path: Path) -> str:
    """The pipeline with Kotlin comments removed.

    A comment does not execute. `release-candidate.pipeline.kts` names
    `stage("skill-bundles")` *inside a comment* — a post-mortem explaining why
    a `$` in that stage once broke the Kotlin compile. Scanning raw text for
    stage names reports it as a second `skill-bundles`, which is not what
    this contract is about.

    That is the third time this session a gate-reading contract has matched
    its own prose. It is why `strip_comments` is imported from
    `test_candidate_packaging.py` rather than re-derived: the rule already
    exists and has been paid for.
    """
    return strip_comments(path.read_text(encoding="utf-8"))


def test_the_release_lane_actually_does_it() -> None:
    """Moving work out of a gate is not the same as deleting it."""
    text = code_of(RELEASE_LANE)
    for needle, what in REQUIRED_IN_LANE:
        check(
            needle in text,
            f"{RELEASE_LANE.name} does not mention {what} (`{needle}`). It was "
            f"removed from the merge gate; if it is not here, nothing builds or "
            f"checks it anywhere. `cognicode-control-plane` is a host binary and "
            f"the per-target stage cross-compiles only cogh, cognicode and "
            f"cognicode-mcp, so dropping this leaves it built by no lane.",
        )


def test_the_release_lane_still_has_its_own_build_stage() -> None:
    """The moved block must not have collided with an existing stage name.

    Measured 2026-10-15: the first attempt inserted a `stage("build")` into a
    file that already had one — `release-candidate.pipeline.kts` holds a
    `build` CONTAINER for the per-target cross builds. Two stages with one
    name in one file means a failure gets attributed to the wrong one, and
    nothing complains.
    """
    for path in (MERGE_GATE, RELEASE_LANE):
        names = re.findall(r'stage\("([^"]+)"\)', code_of(path))
        seen: set[str] = set()
        dupes: list[str] = []
        for name in names:
            if name in seen:
                dupes.append(name)
            seen.add(name)
        check(
            not dupes,
            f"{path.name} declares the stage name(s) {sorted(set(dupes))} more "
            f"than once. Two stages with one name in one pipeline is how a "
            f"failure gets attributed to the wrong stage.",
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
        "PASS — the release lane still builds what a release needs, and no stage "
        "name is declared twice"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())