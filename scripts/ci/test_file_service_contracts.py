#!/usr/bin/env python3
"""Contratos del servicio de ficheros: el corte de texto y el rechazo.

Dos contratos de lectura de codigo (ninguno ejecuta el servicio) sobre los
caminos de ficheros del producto. Se fusionaron en uno (TASK-74) conservando
todas las comprobaciones. No comparten un sujeto unico: cada seccion vigila sus
propios ficheros.

# I. Un corte por indice de byte no parte un caracter.

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

# II. Un rechazo llega al cliente como un rechazo, no como un fallo interno.

MEDIDO: los cinco handlers de ficheros aplanan el error a `AppError::InternalError`
cuando hay colector de metricas, y el servidor MCP si lo inicializa. Un rechazo
deliberado --un camino fuera del workspace, un patron que no es una expresion
regular-- llegaba al cliente como "application: Internal error: Invalid
parameter: ...", de modo que el cliente no podia distinguir un rechazo de una
caida. Sin colector la categoria se conservaba, y por eso el defecto era
invisible a la suite entera: la suite prueba el camino que ya funcionaba.

La causa no era el aplanado, sino su causa: `instrument_tool` exigia un error
del tipo de la telemetria (`ToolError`), de modo que el llamante tenia que
convertir su error y despues reconstruirlo, y esa reconstruccion perdia la
categoria. El contador necesita un *nombre* para el error, no el error.

Este contrato mira tres cosas y ninguna ejecuta el servicio:

1. Ningun handler reconstruye un error de la aplicacion como fallo interno.
2. El contador no exige su propio tipo de error: es generico sobre el error.
3. Cada handler que pasa por el contador tiene un caso en la prueba de
   frontera, para que anadir uno nuevo no pueda dejar la tabla sin cubrir.

Cada detector devuelve los hallazgos que encuentra, de modo que el propio
contrato puede aplicarse a un texto que viola la garantia: un detector que no
mira nada no puede declararse verde.

Ejecutar:  python3 scripts/ci/test_file_service_contracts.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# --- I. un corte por indice de byte no parte un caracter -------------------

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

# --- II. un rechazo llega como un rechazo ----------------------------------

HANDLERS = "crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs"
TELEMETRY = "crates/cognicode-core/src/infrastructure/telemetry/mod.rs"
BOUNDARY_TEST = "crates/cognicode-core/tests/it/a_rejection_arrives_as_a_rejection.rs"


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


# --- I. cortes de texto ---


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


# --- II. rechazos ---


# ---------------------------------------------------------------------------
# Detectores
# ---------------------------------------------------------------------------


def the_handlers_do_not_flatten_the_error(text: str) -> list[str]:
    """Ningun handler reconstruye un error de la aplicacion como fallo interno.

    El aplanado no es una consecuencia del contador: es lo que hace el handler
    cuando el contador le devuelve un error que no es el suyo. Si aparece
    `AppError::InternalError(`, el rechazo vuelve a llegar como una caida.
    """
    findings: list[str] = []
    for number, line in enumerate(text.splitlines(), start=1):
        if "AppError::InternalError(" in line:
            findings.append(
                f"{HANDLERS}:{number}: el error se reconstruye como fallo "
                f"interno: {line.strip()}"
            )
    return findings


def the_counter_does_not_demand_its_own_error_type(text: str) -> list[str]:
    """El contador es generico sobre el error que la operacion produce.

    Si `instrument_tool` vuelve a exigir `Result<T, ToolError>`, el llamante
    tiene que convertir su error y reconstruirlo despues, y esa reconstruccion
    es la que pierde la categoria. El tipo del error tiene que viajar intacto.
    """
    findings: list[str] = []
    start = text.find("pub async fn instrument_tool")
    if start == -1:
        findings.append(f"{TELEMETRY}: no existe `instrument_tool`")
        return findings
    end = text.find("{", start)
    if end == -1:
        findings.append(f"{TELEMETRY}: no se puede leer la firma de `instrument_tool`")
        return findings
    signature = text[start:end]
    if "Result<T, ToolError>" in signature:
        findings.append(
            f"{TELEMETRY}: `instrument_tool` vuelve a exigir el error de la "
            "telemetria; el error de la operacion tiene que volver intacto"
        )
    if "E" not in signature:
        findings.append(
            f"{TELEMETRY}: `instrument_tool` no es generico sobre el error de la "
            "operacion"
        )
    return findings


def every_counted_handler_has_a_case(handlers_text: str, test_text: str) -> list[str]:
    """Cada handler que pasa por el contador tiene un caso en la prueba.

    Anadir un handler nuevo no puede dejar la tabla sin cubrir: la garantia es
    de todos los handlers, no de los que la tabla menciona hoy.
    """
    findings: list[str] = []
    cases = set(re.findall(r'id:\s*"([a-z_]+)"', test_text))
    if not cases:
        findings.append(
            f"{BOUNDARY_TEST}: no se encuentra ninguna tabla de casos: la prueba "
            "no probaria nada"
        )
        return findings

    blocks = re.split(r"(?m)^pub (?:async )?fn (handle_[a-z_]+)\(", handlers_text)
    counted: list[str] = []
    for index in range(1, len(blocks), 2):
        name = blocks[index][len("handle_") :]
        body = blocks[index + 1] if index + 1 < len(blocks) else ""
        if "instrument_tool(" in body:
            counted.append(name)
            if name not in cases:
                findings.append(
                    f"{HANDLERS}: `{name}` pasa por el contador y no tiene caso en "
                    f"{BOUNDARY_TEST}: anade el caso o explica por que no aplica"
                )
    if not counted:
        findings.append(
            f"{HANDLERS}: no se encuentra ningun handler que pase por el "
            "contador; el contrato no miraria nada"
        )
    return findings


# ---------------------------------------------------------------------------
# Comprobaciones
# ---------------------------------------------------------------------------


def test_the_handlers_do_not_flatten_the_error() -> None:
    """Ningun handler convierte un rechazo en un fallo interno."""
    findings = the_handlers_do_not_flatten_the_error(source(HANDLERS))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_counter_does_not_demand_its_own_error_type() -> None:
    """El contador devuelve el error de la operacion, intacto."""
    findings = the_counter_does_not_demand_its_own_error_type(source(TELEMETRY))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_every_counted_handler_has_a_case() -> None:
    """Anadir un handler no puede dejar la tabla de casos sin cubrir."""
    findings = every_counted_handler_has_a_case(source(HANDLERS), source(BOUNDARY_TEST))
    assert not findings, "FAIL: " + "; ".join(findings)


def test_the_detectors_see_a_violation_when_there_is_one() -> None:
    """Control anti-vacuidad: cada detector nota un texto que la viola."""
    probes: list[tuple[object, tuple[object, ...]]] = [
        (
            the_handlers_do_not_flatten_the_error,
            (
                "Err(e) => Err(HandlerError::App(AppError::InternalError(e.message))),\n",
            ),
        ),
        (
            the_counter_does_not_demand_its_own_error_type,
            (
                "pub async fn instrument_tool<F, T>(\n"
                "    metrics: &ToolMetrics,\n"
                "    tool_name: &str,\n"
                "    f: F,\n"
                ") -> Result<T, ToolError>\n"
                "where\n"
                "    F: Future<Output = Result<T, ToolError>>,\n"
                "{}\n",
            ),
        ),
        (
            every_counted_handler_has_a_case,
            (
                "pub async fn handle_read_file(\n"
                "    ctx: &HandlerContext,\n"
                ") -> HandlerResult<ReadFileOutput> {\n"
                "    let result = instrument_tool(&metrics, \"read_file\", async {\n"
                "    }).await;\n"
                "}\n"
                "pub async fn handle_write_file(\n"
                "    ctx: &HandlerContext,\n"
                ") -> HandlerResult<WriteFileOutput> {\n"
                "}\n",
                # El caso que hay es el de otro handler: el que pasa por el
                # contador se queda sin cubrir, y eso es lo que hay que notar.
                'id: "write_file",\n',
            ),
        ),
    ]
    for detector, args in probes:
        found = detector(*args)  # type: ignore[operator]
        assert found, (
            f"FAIL: el detector {detector.__name__} no nota un texto que la "
            "viola; las comprobaciones de arriba pasarian sin comprobar nada"
        )


failures: list[str] = []


def main() -> int:
    failures.clear()
    names: list[str] = []
    for name, func in sorted(globals().items()):
        if not (name.startswith("test_") and callable(func)):
            continue
        names.append(name)
        try:
            func()
        except AssertionError as error:
            failures.append(str(error))
            print(f"  FAIL {name}")
        except Exception as error:  # noqa: BLE001 - a check failing is data
            failures.append(f"{name} raised {type(error).__name__}: {error}")
            print(f"  FAIL {name}")
        else:
            print(f"  PASS {name}")

    if failures:
        print("servicio de ficheros: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(f"servicio de ficheros: OK ({len(names)} comprobaciones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
