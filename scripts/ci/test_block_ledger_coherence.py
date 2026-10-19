#!/usr/bin/env python3
"""El bloque no puede decir una cosa mientras la agenda dice otra.

## El hueco

`docs/roadmap/ROADMAP.md` §10 es, desde el cierre de B1, el unico sitio que
lista cada bloque con su tag y su veredicto. Los goal docs de
`odd/tasks/b[1-6]-*.md` son prosa que *repiten* ese estado en su cabecera.

Nada pineaba que las dos cosas coincidieran, y divergieron:

  - `b5-safe-refactoring.md` decia `PENDING` con el recibo entero en
    `_pendiente_`, cuando `v0.101.15` estaba publicado, habia veredicto
    escrito, y ROADMAP §10 lo listaba cerrado desde 2026-10-12. Un agente que
    siguió la instruccion literal —"avanza con el siguiente WU no
    bloqueado"— habria reconstruido ~2.500 lineas de codigo ya publicado.
  - La tabla "Estado actual del programa" de
    `odd/tasks/2026-Q4-work-loop.md` seguia diciendo `B1 PENDING` y
    `B2..B6 BLOCKED (B1 prerequisite)` cuatro dias despues del cierre.

El coste no fue teorico: el primer caso pide rehacer trabajo ya vendido.

## Lo que se anade

Una regla derivada, no una segunda lista: **la cabecera de cada goal doc se
comprueba contra §10**, que es la autoridad. Asi el goal doc no vuelve a ser
una fuente de verdad —deriva de otra— y la deriva se detecta sola.

Dos invariantes, ambas vistas caer sobre el estado real:

  - un bloque con tag publicado en §10 no puede declarar un estado distinto
    de `CLOSED` en su goal doc                                  -> 1 rojo
  - un bloque con tag publicado en §10 no puede figurar como `PENDING` o
    `BLOCKED` en la tabla del work-loop                         -> 1 rojo
  - (control, sin mutacion)                                      -> todo verde

## Lo que NO se comprueba, y por que

  - **Casillas `- [ ]` en un bloque `CLOSED`.** Es cosmético: quien lee el
    bloque ve `CLOSED` y el recibo con SHA debajo. Una casilla sin marcar no
    le dice a nadie que rehaga el trabajo; el que si lo hace es una cabecera
    que dice `PENDING`. Distinguir las dos cosas es el punto de la regla.
    (B4 y el doc detallado de B5 tienen las casillas sin marcar; es deuda
    de higiene declarada, no un defecto que este contrato deba bloquear.)
  - **`odd/tasks/b5-safe-refactoring-authorized-changes.md`.** Es una
    especificacion archivada y protegida: no se reescribe. Su cabecera y su
    recibo ya coinciden, asi que la dejariamos pasar igual; se excluye por
    proteccion, no porque se haya perdonado.
  - **Si el SHA del recibo resuelve en git.** Seria un buen contrato, pero
    `git cat-file -e` sobre SHAs de ramas ya_consumidas depende de que el
    objeto siga presente en el clon, y eso lo decidio el GC, no el
    documento. Un recibo puede ser honesto y su SHA pode no estar aqui.
    (Lo que si se comprobara —alcanzabilidad desde un ref, no mera
    existencia— habria caido sobre `b5-safe-refactoring-authorized-changes.md`,
    que acredita las cuatro WU de B5 al SHA `0e5708d`,commit hermano
    descartado que no esta en la historia del proyecto. Ver alli.)

## Limites del contrato

Este contrato comprueba **coherencia del libro mayor**, no la verdad del
trabajo que el libro mayor afirma. Un `CLOSED` acompanado de una edicion
consistente en ROADMAP §10 pasaria, y no deberia pasar: lo que caza eso es
el gate de publicacion (`merge-gate`) mas las suites UAT/AAT del bloque
(`b5_w1..b5_w4`), que es donde vive la prueba de que el trabajo existe.

La division es deliberada: el libro es barato de comprobar y caro que se
MENTIRA; el trabajo es caro de comprobar y lo que hay que proteger. Que cada
cosa la compruebe la evidencia que le corresponde.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
ROADMAP = REPO_ROOT / "docs" / "roadmap" / "ROADMAP.md"
WORK_LOOP = REPO_ROOT / "odd" / "tasks" / "2026-Q4-work-loop.md"

# Especificaciones detailizadas y archivadas. Se leen, no se reescriben.
PROTECTED = {"b5-safe-refactoring-authorized-changes.md"}

CLOSED_MARKER = "CLOSED"


def goal_docs() -> list[Path]:
    return sorted(
        p
        for p in (REPO_ROOT / "odd" / "tasks").glob("b[1-6]-*.md")
        if p.name not in PROTECTED
    )


def released_blocks() -> dict[str, str]:
    """`{bloque: tag}` segun ROADMAP §10 — la unica autoridad.

    Solo filas con tag de release publicado. Una fila que aun no tiene tag
    es trabajo en curso, que es exactamente lo que §10 no declara cerrado.
    """
    text = ROADMAP.read_text(encoding="utf-8")
    out: dict[str, str] = {}
    for block, tag in re.findall(
        r"\|\s*\*\*B([1-6])\*\*\s*\|\s*\**(v0\.\d+\.\d+)\**\s*\|", text
    ):
        out[f"B{block}"] = tag
    return out


def _declared_state(text: str) -> str | None:
    """La primera palabra tras `**Estado**:` en la cabecera."""
    m = re.search(r"\*\*Estado\*\*\s*:\s*([A-Za-z_]+)", text)
    return m.group(1).upper() if m else None


def block_of(doc: Path) -> str | None:
    """`b5-...` -> `B5`, por el nombre del fichero."""
    m = re.match(r"b([1-6])-", doc.name)
    return f"B{m.group(1)}" if m else None


def goal_doc_states() -> dict[str, tuple[str | None, Path]]:
    out: dict[str, tuple[str | None, Path]] = {}
    for doc in goal_docs():
        block = block_of(doc)
        if block:
            out[block] = (_declared_state(doc.read_text(encoding="utf-8")), doc)
    return out


def work_loop_states() -> dict[str, str]:
    """El estado por bloque en la tabla del work-loop."""
    text = WORK_LOOP.read_text(encoding="utf-8")
    out: dict[str, str] = {}
    for block, state in re.findall(
        r"\|\s*B([1-6])\s*\|\s*[^|]*\|\s*([A-Za-z_]+)", text
    ):
        out[f"B{block}"] = state.upper()
    return out


# ---------- assertions (each is one mutation seen to fail) ----------

def test_every_released_block_has_a_goal_doc() -> None:
    """Control: §10 y los goal docs tienen que poder referirse el uno al otro.

    Sin esto, un goal doc renombrado haria que las reglas siguientesFinder
    vacio un bloque sin comprobar nada — un contrato que pasa de no mirar.
    """
    docs = {block for block, _ in goal_doc_states().items()}
    missing = sorted(set(released_blocks()) - docs)
    assert not missing, (
        f"FAIL: ROADMAP §10 publica {missing} pero no hay goal doc "
        f"correspondiente en odd/tasks/. Las reglas siguientes no podrian "
        f"comprobar nada sobre esos bloques."
    )


def test_a_released_block_does_not_declare_itself_pending() -> None:
    """M1: cabecera del goal doc contra §10.

    Es la regla que impide reconstruir trabajo ya publicado: una cabecera
    `PENDING` sobre un tag vendido manda a rehacer el bloque entero.
    """
    states = goal_doc_states()
    wrong = []
    for block, tag in sorted(released_blocks().items()):
        declared = states.get(block, (None, None))[0]
        if declared != CLOSED_MARKER:
            wrong.append(
                f"{block} (tag {tag}) declara {declared!r} en "
                f"{states.get(block, (None, '?'))[1]}"
            )
    assert not wrong, (
        "FAIL: bloques publicados en ROADMAP §10 que su goal doc no declara "
        "CLOSED:\n  - " + "\n  - ".join(wrong) + "\n"
        "O el goal doc va atras, o §10 va erroneo. Reconcilia antes de "
        "trabajar sobre el bloque."
    )


def test_work_loop_table_agrees_with_the_roadmap() -> None:
    """M2: la tabla de cadencia contra §10.

    Es la segunda fuente de verdad, y la que un agente lee primero para
    saber por donde empezar. Si dice `BLOCKED (B1 prerequisite)` cuatro
    dias despues del cierre de B1, manda a parar donde hay trabajo.
    """
    table = work_loop_states()
    wrong = []
    for block, tag in sorted(released_blocks().items()):
        state = table.get(block)
        if state != CLOSED_MARKER:
            wrong.append(f"{block} (tag {tag}) figura como {state!r}")
    assert not wrong, (
        "FAIL: la tabla de `odd/tasks/2026-Q4-work-loop.md` contradice a "
        "ROADMAP §10 en:\n  - " + "\n  - ".join(wrong) + "\n"
        "La tabla manda a detenerse en bloques que ya estan cerrados."
    )


def test_control_all_green() -> None:
    """Control: en estado nominal, las tres anteriores pasan.

    Si esto falla con las fuentes ya reconciliadas, el bug es del contrato.
    """
    test_every_released_block_has_a_goal_doc()
    test_a_released_block_does_not_declare_itself_pending()
    test_work_loop_table_agrees_with_the_roadmap()


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_every_released_block_has_a_goal_doc,
        test_a_released_block_does_not_declare_itself_pending,
        test_work_loop_table_agrees_with_the_roadmap,
        test_control_all_green,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("Block ledger coherence: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    released = released_blocks()
    print(f"Block ledger coherence: OK ({len(released)} bloques publicados)")
    return 0


if __name__ == "__main__":
    sys.exit(main())