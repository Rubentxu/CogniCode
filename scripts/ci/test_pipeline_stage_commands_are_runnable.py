#!/usr/bin/env python3
"""Una etapa renderizada no puede ejecutar una opcion como si fuera un comando.

MEDIDO 2026-10-21, en el gate de candidato de `v0.101.25`. La etapa
`core-b6-w1-release-stdio-smoke` murio con:

    script.sh: linea 38: --: orden no encontrada

El cuerpo de esa etapa, en `release-candidate.pipeline.kts`, era:

    cargo test -p cognicode-core --test b6_w1_mcp_robustness --quiet \\\\
        -- release_binary_stdio_round_trip_smoke

La barra invertida estaba duplicada. En una cadena cruda de Kotlin no hay
escapes: `\\\\` son DOS caracteres, y el shell lee una barra invertida escapada
—no una continuacion de linea—. La consecuencia es que el filtro del test
dejaba de ser parte del comando y se convertia en la siguiente orden de la
secuencia: `-- release_binary_stdio_round_trip_smoke`, que no existe. El
`cargo test` de al lado corria con un filtro vacio, y la stage se caia con
127.

La stage se habia creado el 2026-10-18, el mismo dia que la lane de
candidato. Desde entonces nunca se habia ejecutado con exito, porque las
corridas anteriores se caian antes: una en el clon limpio por falta de disco,
y esta. La lane que verifica lo que se publica llevaba tres semanas con una
orden imposible en el cuerpo.

## Por que un contrato y no un comentario

El comentario de al lado de la stage era correcto y no impedia nada. El
problema no es la intention: es que **una barra invertida de mas convierte la
linea siguiente en una orden**. Y eso es una propiedad del shell renderizado,
no una opinion sobre la stage.

Este contrato renderiza el cuerpo de cada etapa de cada pipeline —con el
mismo renderizador que el resto de contratos del repo, para no reimplementar
la resolucion de Kotlin— y exige que ninguna linea logica empiece por una
opcion. Una linea logica es la que el shell ejecuta como una orden: una
linea fisica que continua a la anterior NO lo es.

El caso que importa es la barra invertida IMPAR: una linea que termina en una
barra invertida continua la siguiente. Una que termina en dos barras —`\\\\`—
no: la segunda escapa a la primera, y la siguiente linea es una orden nueva.
Ese es exactamente el defecto medido.

Run:
    python3 scripts/ci/test_pipeline_stage_commands_are_runnable.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))

import pipeline_authority  # noqa: E402 - el renderer vive junto a los contratos
import test_candidate_packaging as packaging  # noqa: E402 - el extractor de cuerpos


def stage_bodies(pipeline: Path) -> list[tuple[str, str]]:
    """El cuerpo de shell de cada stage, con su nombre.

    No se usa `pipeline_authority.steps()`, que saca los cuerpos con una
    expresion regular que solo encuentra un `sh(...)` de una sola pieza. La
    stage que fallo escribia su `sh()` concatenando varias —un raw string
    entre medias—, y esa forma se le escapa: el contrato pasaba sobre el
    pipeline roto sin mirar la stage.

    `packaging.extract_stage_body` si resuelve esa forma —busca el `sh(` y
    luego el raw string que abre— y es el extractor que ya usan el resto de
    contratos del repo, asi que no hay una segunda manera de leer un cuerpo.
    """
    text = pipeline.read_text(encoding="utf-8")
    bodies: list[tuple[str, str]] = []
    for match in re.finditer(r'stage\("([^"]+)"\)\s*\{', text):
        name = match.group(1)
        try:
            body = packaging.extract_stage_body(text, name)
        except ValueError:
            # Un stage group, o un stage sin `sh(...)`: no hay cuerpo que mirar.
            continue
        bodies.append((name, body))
    return bodies


def logical_lines(body: str) -> list[tuple[int, str]]:
    """Las lineas que el shell ejecuta como orden, con su numero fisico.

    Una linea fisica continua la anterior cuando termina en un numero IMPAR
    de barras invertidas: una barra continua, dos escapan la una a la otra.
    """
    result: list[tuple[int, str]] = []
    pending = ""
    pending_at = 0
    for number, raw in enumerate(body.splitlines(), start=1):
        line = raw.rstrip()
        if not pending:
            pending_at = number
        stripped = line.rstrip()
        backslashes = len(stripped) - len(stripped.rstrip("\\"))
        if backslashes % 2 == 1:
            pending += stripped[:-1] + " "
            continue
        result.append((pending_at, (pending + stripped).strip()))
        pending = ""
    if pending:
        result.append((pending_at, pending.strip()))
    return result


def test_no_stage_executes_an_option_as_a_command() -> None:
    """La property: ninguna orden renderizada empieza por una opcion.

    `--` no es un programa. Una orden que empieza por `--` es un fragmento de
    una orden que se partio, y el shell la intenta ejecutar.
    """
    offenders: list[str] = []
    for pipeline in pipeline_authority.pipeline_paths():
        for name, body in stage_bodies(pipeline):
            for number, line in logical_lines(body):
                if not line or line.startswith("#"):
                    continue
                first = line.split()[0]
                if first.startswith("--"):
                    offenders.append(
                        f"{pipeline.name}: stage `{name}`, linea logica {number} "
                        f"empieza por {first!r}: {line[:90]!r}"
                    )
    assert not offenders, (
        "una etapa renderizada ejecuta una opcion como si fuera una orden:\n"
        + "\n".join(offenders)
        + "\nEl shell lee una barra invertida doble como una barra escapada, no "
        "como una continuacion de linea, y la linea siguiente se convierte en "
        "una orden nueva. En un raw string de Kotlin no hay escapes: una sola "
        "barra continua la linea."
    )


def test_the_continuation_rule_is_the_shell_rule() -> None:
    """El contrato tiene que usar la misma regla de continuacion que el shell.

    Si esta cuenta las barras de otra manera, el contrato pasa con un
    pipeline roto —que es exactamente lo que pasó con el comentario al lado
    de la stage— o falla con uno bueno.
    """
    odd = logical_lines("cargo test --quiet \\\n    -- un_filtro")
    assert len(odd) == 1, (
        f"una linea que termina en una barra invertida continua la siguiente: "
        f"el contrato vio {len(odd)} ordenes donde el shell ve una"
    )
    even = logical_lines("cargo test --quiet \\\\\n    -- un_filtro")
    assert len(even) == 2, (
        f"una linea que termina en dos barras invertidas NO continua la "
        f"siguiente: el shell las lee como una barra escapada. El contrato vio "
        f"{len(even)} ordenes donde el shell ve dos"
    )


def main() -> int:
    failures: list[str] = []
    for func in (
        test_no_stage_executes_an_option_as_a_command,
        test_the_continuation_rule_is_the_shell_rule,
    ):
        try:
            func()
        except AssertionError as exc:
            failures.append(str(exc))
        except Exception as exc:  # noqa: BLE001 - a check failing is data
            failures.append(f"{func.__name__} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(failure)
        return 1
    print(
        "PASS — every rendered stage body runs only real commands: no line "
        "that the shell would execute starts with an option."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
