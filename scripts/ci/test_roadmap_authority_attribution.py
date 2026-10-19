#!/usr/bin/env python3
"""La autoridad normativa que §11 cita tiene que existir y cubrir la unidad.

## El hueco

`docs/roadmap/ROADMAP.md` §11 dice cuales son las unidades abiertas y de
donde sale su especificacion. Es la puerta de entrada del trabajo
pendiente, asi que sus citas no son adorno: son lo primero que abre quien
llega.

Las tres estaban mal, y por el mismo motivo:

  - **H3 y H4 (K1 y K2) compartian un unico ADR**, `ADR-K003.md`, cuya
    linea 3 dice literalmente "**Aplica a:** K2". K1 se quedaba sin
    autoridad correcta y K2 recibia una cita que no podia desmentir.
  - **H4.4 y H5 se remitian a un HITOS del programa delivery-first** que
    vive bajo `docs/*` en `.gitignore`: `git ls-files` devuelve **0**
    ficheros. Existia en el disco de quien lo escribio y en ninguno mas.

Un clon limpio no resolvia ninguna de las tres citas. No es una opinion
sobre si el ADR elegido es el mejor: es que el puntero no llevaba a nada.

## Lo que se anade

Una regla derivada, no una segunda lista. La autoridad de cada ADR ya esta
declarada en su propia linea 3 ("Aplica a:"); aqui solo se comprueba que
§11 no la contradiga:

  - una ruta citada en §11 que no este VERSIONADA  -> 1 rojo
  - un ADR citado cuya "Aplica a:" no declara la unidad que §11 le
    atribuye                                           -> 1 rojo
  - (control, sin mutacion)                            -> todo verde

El control importa mas que los otros dos: una regla que no puede llegar a
fallar es indistinguible de una que nunca falla.

## Lo que NO se comprueba

  - **Que la autoridad citada sea la mejor eleccion.** Solo que exista, este
    versionada y declare la unidad. Elegir bien un ADR es juicio humano.
  - **El estado de la unidad.** Que H3 siga abierta no lo decide este
    contrato; lo hace `test_roadmap_open_units_coherence.py`, que es
    complementario y no solapado: aquel vigila el CIERRE, este la AUTORIDAD.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ROADMAP = ROOT / "docs/roadmap/ROADMAP.md"

OPEN_UNITS_ANCHOR = "Las unidades realmente abiertas son"

# Techo del parrafo de §11: es un parrafo, no una seccion.
MAX_BLOCK_LINES = 80

# Rutas de repo citadas entre acentos graves dentro de esa zona.
CITED_PATH = re.compile(r"`(docs/[A-Za-z0-9._/-]+\.(?:md|json|kts))`")

# Una cita de ADR con la unidad a la que se le atribuye, escritas juntas
# para que la atribucion sea legible y parseable a la vez:
#   `.../ADR-K002.md` (K1)
# El separador entre la cita y su unidad admite `>` del blockquote: la
# atribucion suele caer en la linea siguiente. MEDIDO: con `\s*` el par
# ADR-K003/(K2) no casaba, la lista de pares salia vacia y el contrato
# daba verde SIN comprobar esa atribucion — precisamente la que existe
# para cazar. Un contrato que pasa mirando menos de lo que dice mirar es
# peor que no tener contrato.
#
# Se captura la RUTA completa, no el nombre de fichero. MEDIDO: hay dos
# `ADR-K002.md` bajo `docs/` — uno versionado en consumer-contracts y otro
# en `docs/cognicode-kotlin-roadmap/`, que no lo esta. Resolver por nombre
# seria elegir entre dos sin saber cual; resolver por la ruta que §11 cita
# no deja esa decision abierta.
ADR_WITH_UNIT = re.compile(
    r"`(docs/[A-Za-z0-9._/-]*ADR-[A-Za-z0-9-]+\.md)`[\s>]*"
    r"\((K[0-9][0-9.A-Za-z–-]*)\)"
)

# Lo que el propio ADR declara. Es la autoridad: aqui no se reinterpreta.
ADR_SCOPE = re.compile(r"\*\*Aplica a:\*\*\s*([^\n.]+)")


def open_units_block(text: str) -> str:
    """El parrafo de §11 que enumera las unidades realmente abiertas.

    Acotado a proposito. MEDIDO: sin techo, el recorrido se seguia por el
    resto del documento entero, porque §11 abre un blockquote largo y todo
    lo que viene despues sigue siendo cita — tomaba 1575 lineas en vez del
    parrafo. Un contrato que escanea el documento entero no esta mirando lo
    que dice mirar. El techo no recorta por comodidad: al superarlo, el
    formato cambio y el contrato debe dejar de poder afirmar nada, en vez
    de siguiendo citas sueltas.

    Si el ancla desaparece, lo mismo: un contrato que se apaga cuando
    cambia el formato inspecciona cero y sale verde.
    """
    start = text.find(OPEN_UNITS_ANCHOR)
    assert start != -1, (
        f"FAIL: {ROADMAP.name} ya no contiene el ancla "
        f"{OPEN_UNITS_ANCHOR!r}. Este contrato no puede declararse verde "
        "sobre texto que no leyo."
    )
    tail = text[start:].splitlines()
    block = [tail[0]]
    for line in tail[1:]:
        if not line.startswith(">"):
            break
        block.append(line)
        if len(block) > MAX_BLOCK_LINES:
            raise AssertionError(
                f"FAIL: el bloque de unidades abiertas de {ROADMAP.name} pasa "
                f"de {MAX_BLOCK_LINES} lineas de cita. El formato cambio y "
                "este contrato no puede afirmar que las citas que encuentra "
                "son las que corresponden."
            )
    return "\n".join(block)


def tracked(paths: list[str]) -> set[str]:
    """Ficheros versionados. `git ls-files` es la unica verdad de eso."""
    out = subprocess.run(
        ["git", "ls-files", "--", *paths],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return {line for line in out.stdout.splitlines() if line}


def test_every_cited_authority_is_versioned() -> None:
    """Una cita que no esta versionada no es una cita: es una nota local."""
    block = open_units_block(ROADMAP.read_text(encoding="utf-8"))
    cited = CITED_PATH.findall(block)
    assert cited, (
        f"FAIL: no se encontro ninguna ruta citada en el bloque de unidades "
        f"abiertas de {ROADMAP.name}. Sin citas, este contrato no comprueba "
        "nada y solo puede pasar."
    )
    present = tracked(cited)
    missing = sorted(set(cited) - present)
    assert not missing, (
        f"FAIL: {ROADMAP.name} cita como autoridad rutas que git no versiona: "
        f"{missing}. Un clon limpio no las resuelve, asi que el puntero no "
        "lleva a nada."
    )


def test_every_cited_adr_claims_the_unit_it_is_attributed() -> None:
    """El ADR declara su alcance en su linea 3; §11 no puede contradecirlo."""
    block = open_units_block(ROADMAP.read_text(encoding="utf-8"))
    pairs = ADR_WITH_UNIT.findall(block)
    assert pairs, (
        f"FAIL: no se encontro ninguna cita de ADR con su unidad en "
        f"{ROADMAP.name}. El formato esperado es `.../ADR-XXX.md` (K<n>), "
        "que es lo que hace falta para comprobar la atribucion."
    )

    versioned = tracked([path for path, _ in pairs])
    problems: list[str] = []
    for adr_path, unit in pairs:
        if adr_path not in versioned:
            problems.append(f"{unit} se atribuye a `{adr_path}`, sin versionar")
            continue
        adr = ROOT / adr_path
        scope = ADR_SCOPE.search(adr.read_text(encoding="utf-8"))
        assert scope is not None, (
            f"FAIL: {adr_path} no declara '**Aplica a:**'. Un ADR que no "
            "declara su alcance no puede desmentir una atribucion, asi que "
            "este contrato no tendria nada que comprobar."
        )
        if unit not in scope.group(1):
            problems.append(
                f"{adr_path} se atribuye a {unit}, pero declara "
                f"'Aplica a: {scope.group(1).strip()}'"
            )
    assert not problems, (
        "FAIL: atribucion de autoridad que el propio ADR desmiente: "
        + "; ".join(problems)
    )


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_every_cited_authority_is_versioned,
        test_every_cited_adr_claims_the_unit_it_is_attributed,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("ROADMAP authority attribution: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("ROADMAP authority attribution: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
