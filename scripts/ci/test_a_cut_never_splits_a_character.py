#!/usr/bin/env python3
"""Un corte por indice de byte no parte un caracter.

MEDIDO: en los caminos de lectura y edicion del servicio de ficheros, y en
dos de la presentacion de la CLI, el texto se recortaba por indice de byte.
Un indice que cae dentro de la codificacion multibyte de un caracter aborta
el proceso que corta; en el servidor de protocolo ese proceso es el que
atiende a las peticiones de todos los agentes conectados.

Este contrato no ejecuta el servicio: mira el codigo y comprueba que ningun
corte por indice sobre texto se queda sin frontera, que los caminos que
leen usan la forma que el tipo hace segura, y que el detector ve un corte
cuando hay uno. El comportamiento observable lo fijan los tests de
integracion y los tests unitarios del helper.

Ejecutar:  python3 scripts/ci/test_a_cut_never_splits_a_character.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

CORE = "crates/cognicode-core/src"
CLI = "crates/cognicode-cli/src"

HELPER = f"{CORE}/application/text_boundary.rs"
APPLICATION_MOD = f"{CORE}/application/mod.rs"
SERVICE = f"{CORE}/application/services/file_operations.rs"
PRESENTER = f"{CLI}/presenter.rs"
LIFECYCLE = f"{CLI}/cmd/lifecycle_resolver.rs"

# Los receptores que, en estos ficheros, son texto. Un corte por indice sobre
# ellos parte un caracter cuando el indice no cae en una frontera.
TEXT_RECEIVERS: dict[str, tuple[str, ...]] = {
    SERVICE: ("content", "remaining", "s"),
    PRESENTER: ("diagram",),
    LIFECYCLE: ("s",),
}

# Las formas que dejan el corte en una frontera: el rango se pide con `get`,
# que devuelve None cuando no cae en una, o el indice se lleva a la frontera
# antes de cortar.
SAFE_FORMS = (
    "get(",
    "floor_char_boundary(",
    "is_char_boundary(",
    "truncate_at_boundary(",
    "slice_at_boundaries(",
)

FUNCTIONS = (
    "floor_char_boundary",
    "is_char_boundary",
    "truncate_at_boundary",
    "slice_at_boundaries",
)


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


def cuts_without_boundary(text: str, receivers: tuple[str, ...]) -> list[tuple[int, str]]:
    """Los cortes por indice sobre texto que no pasan por una frontera."""
    pattern = re.compile(
        r"\b(" + "|".join(re.escape(name) for name in receivers) + r")\s*\[[^\[\]]*\]"
    )
    found: list[tuple[int, str]] = []
    for number, line in enumerate(text.splitlines(), start=1):
        if not pattern.search(line):
            continue
        if any(form in line for form in SAFE_FORMS):
            continue
        found.append((number, line.strip()))
    return found


def test_the_boundary_helper_is_public() -> None:
    """El helper existe, se declara y expone las cuatro operaciones."""
    text = source(HELPER)
    for name in FUNCTIONS:
        assert f"pub fn {name}" in text, f"FAIL: {HELPER} no expone `pub fn {name}`"

    declared = source(APPLICATION_MOD)
    assert "pub mod text_boundary;" in declared, (
        f"FAIL: {APPLICATION_MOD} no declara `pub mod text_boundary;`, "
        "así que el helper no es alcanzable desde fuera del crate"
    )


def test_no_text_is_cut_by_byte_without_a_boundary() -> None:
    """Ningun corte por indice sobre texto se queda sin frontera."""
    offenders: list[str] = []
    for relative, receivers in TEXT_RECEIVERS.items():
        for number, line in cuts_without_boundary(source(relative), receivers):
            offenders.append(f"{relative}:{number}: {line}")
    assert not offenders, (
        "FAIL: un corte por indice de byte ha vuelto a aparecer:\n  "
        + "\n  ".join(offenders)
        + "\nUn corte por byte parte un caracter multibyte y aborta el proceso."
    )


def test_the_read_path_asks_for_the_range_in_a_safe_form() -> None:
    """El servicio pide el rango con la forma que el tipo hace segura."""
    text = source(SERVICE)
    assert "content.get(offset..)" in text, (
        f"FAIL: {SERVICE} no pide el tramo con `get`, que devuelve None cuando "
        "el rango no cae en fronteras; un token que empieza dentro de un "
        "caracter abortaria el proceso"
    )
    assert "floor_char_boundary(remaining," in text, (
        f"FAIL: {SERVICE} no ajusta el tamano del trozo a una frontera de "
        "caracter; un trozo que termina dentro de un caracter abortaria el "
        "proceso"
    )


def test_the_production_paths_use_the_helper() -> None:
    """Quien recorta texto pasa por las fronteras de caracter."""
    for relative, symbol in (
        (SERVICE, "floor_char_boundary"),
        (PRESENTER, "truncate_at_boundary"),
        (LIFECYCLE, "truncate_at_boundary"),
    ):
        assert symbol in source(relative), (
            f"FAIL: {relative} no usa `{symbol}`; el recorte de texto tiene "
            "que pasar por el helper de fronteras"
        )


def test_the_detector_sees_a_cut_when_there_is_one() -> None:
    """Control anti-vacuidad: el detector mira de verdad."""
    probe = "for ch in content[..cap].chars() {\n"
    found = cuts_without_boundary(probe, ("content",))
    assert len(found) == 1, (
        "FAIL: el detector no ve un corte por indice en un texto de ejemplo; "
        "la regla de arriba pasaria sin comprobar nada"
    )
    safe = "let chunk = content.get(offset..).ok_or_else(|| 1)?;\n"
    assert not cuts_without_boundary(safe, ("content",)), (
        "FAIL: el detector marca como inseguro un corte que usa `get`"
    )


def main() -> int:
    checks = [
        test_the_boundary_helper_is_public,
        test_no_text_is_cut_by_byte_without_a_boundary,
        test_the_read_path_asks_for_the_range_in_a_safe_form,
        test_the_production_paths_use_the_helper,
        test_the_detector_sees_a_cut_when_there_is_one,
    ]
    failures: list[str] = []
    for check in checks:
        try:
            check()
        except AssertionError as error:
            failures.append(str(error))
            print(f"  FAIL {check.__name__}")
        else:
            print(f"  PASS {check.__name__}")

    if failures:
        print("un corte por indice de byte parte un caracter: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(f"un corte por indice de byte parte un caracter: OK ({len(checks)} comprobaciones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
