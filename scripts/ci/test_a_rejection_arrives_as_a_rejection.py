#!/usr/bin/env python3
"""Un rechazo llega al cliente como un rechazo, no como un fallo interno.

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

Ejecutar:  python3 scripts/ci/test_a_rejection_arrives_as_a_rejection.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

HANDLERS = "crates/cognicode-core/src/interface/mcp/file_ops_handlers.rs"
TELEMETRY = "crates/cognicode-core/src/infrastructure/telemetry/mod.rs"
BOUNDARY_TEST = "crates/cognicode-core/tests/a_rejection_arrives_as_a_rejection.rs"


def source(relative: str) -> str:
    path = REPO_ROOT / relative
    if not path.is_file():
        raise AssertionError(f"FAIL: no existe {relative}")
    return path.read_text(encoding="utf-8")


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


def main() -> int:
    checks = [
        test_the_handlers_do_not_flatten_the_error,
        test_the_counter_does_not_demand_its_own_error_type,
        test_every_counted_handler_has_a_case,
        test_the_detectors_see_a_violation_when_there_is_one,
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
        print("un rechazo llega como un rechazo: FAIL")
        for failure in failures:
            print(failure)
        return 1
    print(
        "un rechazo llega como un rechazo: OK "
        f"({len(checks)} comprobaciones)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
