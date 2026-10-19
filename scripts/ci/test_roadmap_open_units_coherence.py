#!/usr/bin/env python3
"""La lista de trabajo abierto no puede sobrevivir a su propio cierre.

## El hueco

`docs/roadmap/ROADMAP.md` §11 termina con un trailer que enumera "las
unidades realmente abiertas" del programa vivo: **H3**, **H4.1**, **H4.2**,
**H4.4**, **H5** y **H0.1**.

Es una segunda fuente de verdad. El Journal es el registro append-only que
declara cuando una unidad cierra; el trailer del ROADMAP *repite* ese estado
en prosa, y nada pineaba que coincidieran.

Divergieron, y en la misma dirección que §10 hace un mes:

  - el programa **H0** completo (H0.1..H0.5) cerro en el Journal Entry con
    `bc3733a`, con su RED ya en verde y 267/267 en la suite fast;
  - §11 seguia nombrando **H0.1** entre las unidades abiertas, y de paso lo
    describia como "con RED ya presentado" — un RED que ya es verde.

Un agente que siga esa instruccion literal reconstruye unaCache que ya esta
corregida y probada.

## Lo que se anade

Una regla derivada, no una segunda lista: **una unidad que el Journal da por
cerrada no puede figurar como abierta en §11**. La autoridad es el Journal
(append-only, con SHA y recuento de suite); el trailer deriva de ella.

  - una unidad del trailer de §11 con cierre en el Journal  -> 1 rojo
  - el trailer no lista ninguna unidad                        -> 1 rojo
  - el Journal no tiene ningun cierre (regla muerta)          -> 1 rojo
  - (control, sin mutacion)                                   -> todo verde

El tercer caso es el guard anti-vacuidad: sin el, un Journal vacio o un
cambio de formato dejaria la regla mirando sin comprobar nada — el mismo
modo de fallo que `test_every_released_block_has_a_goal_doc` busca en el
contrato hermano de §10.

## Lo que NO se comprueba, y por que

  - **Si una unidad sin cierre en el Journal esta de verdad abierta.** El
    Journal registra cierres; que no haya cierre es *ausencia de evidencia*,
    no evidencia de que siga abierta. La autoridad sigue siendo humana: este
    contrato solo quita del trailer lo que el Journal ya cerro, nunca
    declara abierta una unidad por su cuenta.
  - **El estado de H3, H4.x, H5.** siguen abiertos y aqui no hay evidencia
    en ningun sentido. Que el contrato no los mencione es correcto: no esta
   anotarlo lo que hace.
  - **Que el trabajo exista.** Mismo limite que el contrato hermano: esto
    comprueba la coherencia del libro, no la verdad del trabajo. Lo que caza
    eso es el gate de publicacion mas la suite de la unidad.

## Limites del contrato

Al ser una regla de *cierre* y no de *avance*, es monotona: no puede
detectar que se haya olvidado anadir una unidad nueva. Esa asimetria es
deliberada — el fallo caro es reconstruir trabajo vendido (lo que ocurrio),
no tardar un ciclo mas en notar una unidad nueva (que se nota en el primer
`git log`).
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
ROADMAP = REPO_ROOT / "docs" / "roadmap" / "ROADMAP.md"
JOURNAL = REPO_ROOT / "docs" / "roadmap" / "JOURNAL.md"

# El Journal declara cierres con estas formas. Se listan porque son las que
# el registro usa de verdad, no porque el formato sea estable.
CLOSURE = re.compile(r"\b(?:closed|acceptance:\s*PASS)\b", re.IGNORECASE)


def _journal_closure_lines(unit: str) -> list[str]:
    """Lineas del Journal que declaran cerrada `unit`.

    `\\b` alrededor del id evita que `H4.1` case con `H4.10` y que `H5`
    case con un token mayor. La coincidencia es por *linea*: el cierre de un
    ciclo se cuenta en su linea, no repartido en varias.
    """
    pat = re.compile(rf"\b{re.escape(unit)}\b")
    out = []
    for line in JOURNAL.read_text(encoding="utf-8").splitlines():
        if pat.search(line) and CLOSURE.search(line):
            out.append(line.strip())
    return out


def section11_open_units() -> list[str]:
    """Ids en negrita del trailer 'unidades realmente abiertas' de §11.

    Solo el bloque final de §11: las lineas de progreso de CU-1/CU-2 estan
    repletas de `CU-x.y` y no son la lista de abiertas.
    """
    text = ROADMAP.read_text(encoding="utf-8")
    m = re.search(
        r"unidades realmente abiertas son.*?(?=\n\n|\n<!--|\Z)", text, re.DOTALL
    )
    if not m:
        return []
    return re.findall(r"\*\*([A-Z]\d(?:\.\d)?)\*\*", m.group(0))


def closed_units_in_journal() -> set[str]:
    """Unidades que cualquier linea del Journal declara cerradas.

    Recorre por linea: un cierre es un hecho de la linea completa, no del
    identificador suelto. Buscar el id en un Journal entero perderia el
    contexto que distingue "H3 arranco" de "H3 cerro".
    """
    found: set[str] = set()
    for line in JOURNAL.read_text(encoding="utf-8").splitlines():
        if not CLOSURE.search(line):
            continue
        for m in re.finditer(r"\b([A-Z]\d(?:\.\d)?)\b", line):
            found.add(m.group(1))
    return found


# ---------- assertions (each is one mutation seen to fail) ----------

def test_the_journal_actually_declares_closures() -> None:
    """Control anti-vacuidad: sin cierres en el Journal, la regla mira sin ver.

    MEDIDO: el Journal registra cierres desde el programa PRF. Si esta falla,
    o el Journal cambio de formato, o la regla siguiente ha muerto en
    silencio — y una regla muerta es peor que no tenerla, porque parece
    que vigila.
    """
    found = closed_units_in_journal()
    assert found, (
        "FAIL: no se encontro ninguna unidad cerrada en "
        f"{JOURNAL.name}. La regla de §11 no tendria contra que aplicarse "
        "y pasaria sin comprobar nada."
    )


def test_section11_does_not_list_a_unit_the_journal_closed() -> None:
    """M1: el trailer de abiertas contra el Journal.

    Es la regla que impide reconstruir trabajo ya cerrado: una unidad
    listada como abierta con su cierre escrito abajo manda a rehacerla.
    """
    stale = []
    for unit in sorted(set(section11_open_units())):
        lines = _journal_closure_lines(unit)
        if lines:
            # Dos cierres independientes si los hay: el que cierra el ciclo
            # concreto y el que cierra el bloque entero. Citar solo el
            # primero deja al lector dudando de si fue casualidad.
            cited = "\n      ".join(l[:110] for l in lines[:2])
            stale.append(f"{unit} — Journal:\n      {cited}")
    assert not stale, (
        "FAIL: unidades que ROADMAP §11 lista como abiertas y el Journal ya "
        "cerro:\n  - " + "\n  - ".join(stale) + "\n"
        "Quitalas del trailer. Reconstruir trabajo cerrado es el fallo caro "
        "que este contrato existe para evitar."
    )


def test_section11_names_at_least_one_open_unit() -> None:
    """M2: un trailer de abiertas vacio no es agenda, es un hueco.

    Si §11 deja de nombrar unidades, el siguiente agente no tiene por donde
    empezar — que es como se pierde una unidad, no como se encuentra.
    """
    units = sorted(set(section11_open_units()))
    assert units, (
        "FAIL: el trailer 'unidades realmente abiertas' de ROADMAP §11 no "
        "nombra ninguna unidad. O el formato cambio, o §11 quedo sin agenda."
    )


def test_control_all_green() -> None:
    """Control: en estado nominal, las tres anteriores pasan."""
    test_the_journal_actually_declares_closures()
    test_section11_does_not_list_a_unit_the_journal_closed()
    test_section11_names_at_least_one_open_unit()


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_the_journal_actually_declares_closures,
        test_section11_does_not_list_a_unit_the_journal_closed,
        test_section11_names_at_least_one_open_unit,
        test_control_all_green,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("ROADMAP open-units coherence: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    units = sorted(set(section11_open_units()))
    print(f"ROADMAP open-units coherence: OK ({len(units)} abiertas: {', '.join(units)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())