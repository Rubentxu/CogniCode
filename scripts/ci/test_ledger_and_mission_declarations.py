#!/usr/bin/env python3
"""La declaracion no puede decir una cosa mientras su lector entiende otra.

Dos contratos de lectura de prosa/JSON declarativos, sin git ni procesos, en
un solo fichero. La parte A vigila que los goal docs y la tabla del work-loop
coincidan con ROADMAP §10; la B, que `goal-compass.json` hable los campos que
el cargador del plugin lee. Cada parte conserva su docstring original y sus
comprobaciones; los dos controles `test_control_all_green` se llaman ahora
`test_control_ledger_all_green` y `test_control_mission_all_green`.

Ejecutar: python3 scripts/ci/test_ledger_and_mission_declarations.py

# PARTE A — coherencia del libro mayor de bloques

El bloque no puede decir una cosa mientras la agenda dice otra.

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


# PARTE B — declaracion de la mision

La declaracion de la mision tiene que hablar el idioma que el cargador lee.

## El hueco

`goal-compass.json` es lo que el plugin `goal-compass` encadena: su `backlog`
es la cola de trabajo de la mision y su `evidenceRefs` es lo que se comprueba
antes de convertir un candidato en objetivo.

El cargador lee unos campos y **solo** esos (`adapters/mission.js`):

    candidateId, objective, whyNow, evidenceRefs, doneWhen, scope, risk,
    priority, prerequisites, source, tokenBudget

Un campo con otro nombre no se pierde con un aviso: se pierde **en silencio**.
La auditoria del plugin lo dice asi: *"el peor caso es una declaracion que
parece llevar la evidencia --porque el campo se llama `evidence` y su valor es
una ruta que existe-- y que en realidad la pierde entera: todos sus candidatos
quedan descartados y la cadena no tiene nada que encadenar"*.

MEDIDO 2026-10-21 en este repositorio: la declaracion escribia la evidencia
en `evidence` (una cadena) y no en `evidenceRefs` (una lista). Los 26
candidatos quedaron descartados por `sin-evidencia` y la cadena no encadenaba
nada. El `doctor` del plugin lo dira con su propia cuenta: 26 candidatos, 0
usables. Dos defectos mas en la misma declaracion: la cola estaba escrita al
reves --el selector ordena de mayor a menor y el primer candidato llevaba el
numero mas bajo--, y una referencia apuntaba a un fichero que no existe.

Nada en el repositorio lo vigilaba. La declaracion podia quedar inservible y
el roadmap seguia pareciendo completo.

## Lo que se anade

Un contrato que **no reimplementa** al cargador: usa las mismas reglas que el
plugin declara, sobre el mismo fichero, y falla si la declaracion no encadena.

  - un candidato sin `evidenceRefs` utilizable            -> 1 rojo
  - un candidato sin criterio de cierre observable         -> 1 rojo
  - la cola escrita al reves (prioridad no decreciente)   -> 1 rojo
  - una referencia que no apunta a un fichero real         -> 1 rojo
  - un candidato con un campo que el cargador no lee       -> 1 rojo
  - (control, sin mutacion)                                -> todo verde

El quinto caso es el guard anti-vacuidad: sin el, un campo mal escrito
--`evidence` en vez de `evidenceRefs`-- pasaria inadvertido, porque el valor
parece correcto y la ruta existe. Es el fallo que se midio.

## Lo que NO se comprueba, y por que

  - **Que el orden de las prioridades sea el que el roadmap quiere.** El
    contrato vigila que sea coherente y decreciente, no que sea el correcto:
    el orden es una decision del operador.
  - **Que un candidato este terminado.** El plugin lleva su propio registro
    de lo que la cadena encadenó y cumplio; este contrato no lo duplica.
  - **Que la mision exista fuera del repositorio.** El estado durable vive
    en el hogar del usuario, no aqui. Aqui solo se vigila la declaracion.

"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent

# ======================= PARTE A: libro mayor de bloques =====================

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


def test_control_ledger_all_green() -> None:
    """Control: en estado nominal, las tres anteriores pasan.

    Si esto falla con las fuentes ya reconciliadas, el bug es del contrato.
    """
    test_every_released_block_has_a_goal_doc()
    test_a_released_block_does_not_declare_itself_pending()
    test_work_loop_table_agrees_with_the_roadmap()


# ======================= PARTE B: declaracion de la mision ===================

DECLARATION = REPO_ROOT / "goal-compass.json"

# Los campos que el cargador de candidatos lee. Copiado de
# `adapters/mission.js::parseCandidate`. Un campo fuera de esta lista se
# pierde en silencio, que es justo lo que hay que cazar.
CAMPOS_QUE_EL_CARGADOR_LEE = {
    "candidateId",
    "objective",
    "whyNow",
    "evidenceRefs",
    "doneWhen",
    "scope",
    "risk",
    "priority",
    "prerequisites",
    "source",
    "tokenBudget",
}

# Campos que el cargador no lee pero que se permiten: son de lectura humana y
# no rompen nada. Lo que no se permite es un campo que *parezca* uno de los
# buenos, porque ahi es donde el autor cree que ha dejado la evidencia.
CAMPOS_TOLERADOS = {"title", "unit"}

# Campos que se parecen a uno bueno pero no lo son. Sus nombres alternativos
# son el fallo medido: el valor parece correcto y la ruta existe.
PARECIDOS_PERIGOSOS = {
    "evidence": "evidenceRefs",
    "evidenceRef": "evidenceRefs",
    "evidences": "evidenceRefs",
    "done": "doneWhen",
    "criteria": "doneWhen",
    "criterion": "doneWhen",
    "id": "candidateId",
}


def _read() -> str:
    if not DECLARATION.is_file():
        return ""
    return DECLARATION.read_text(encoding="utf-8")


def _split_reference(ref: str) -> tuple[str, str | None]:
    """Separa la referencia en ruta y ancla."""
    index = ref.find("#")
    if index == -1:
        return ref.strip(), None
    return ref[:index].strip(), ref[index + 1:].strip()


def _anchor_exists(content: str, anchor: str) -> bool:
    """El ancla tiene que estar en el fichero: id explicito o titulo de seccion."""
    if anchor == "":
        return True
    escaped = re.escape(anchor)
    if re.search(rf'(?:id|name)\s*=\s*["\']{escaped}["\']', content):
        return True
    if re.match(r"^[\d.]+$", anchor):
        if re.search(rf"^#{{1,6}}\s+{escaped}(?:\s|$)", content, flags=re.MULTILINE):
            return True
    return False


def test_declaration_exists_and_parses() -> None:
    """Sin esto, ninguna otra regla mira nada."""
    assert DECLARATION.is_file(), (
        f"FAIL: no existe {DECLARATION}. La mision se declara ahi."
    )
    raw = _read()
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as err:
        raise AssertionError(f"FAIL: {DECLARATION.name} no es JSON valido: {err}")
    assert isinstance(data, dict), f"FAIL: {DECLARATION.name} no es un objeto"
    assert isinstance(data.get("backlog"), list), (
        f"FAIL: {DECLARATION.name} no tiene una lista 'backlog'."
    )
    assert data["backlog"], (
        "FAIL: el backlog esta vacio. Una mision sin trabajo declarado no encadena nada."
    )


def test_every_candidate_uses_the_fields_the_loader_reads() -> None:
    """Un campo que el cargador no lee se pierde en silencio.

    El caso peligroso es un campo con nombre parecido al bueno: su valor
    parece correcto y su ruta existe, asi que nadie se entera de que el
    cargador no lo mira. Es el fallo medido el 2026-10-21.
    """
    data = json.loads(_read())
    problems: list[str] = []
    for index, candidate in enumerate(data["backlog"]):
        if not isinstance(candidate, dict):
            problems.append(f"backlog[{index}] no es un objeto")
            continue
        candidate_id = candidate.get("candidateId", f"backlog[{index}]")
        for field in candidate:
            if field in CAMPOS_QUE_EL_CARGADOR_LEE or field in CAMPOS_TOLERADOS:
                continue
            parecido = PARECIDOS_PERIGOSOS.get(field)
            if parecido:
                problems.append(
                    f"{candidate_id}: el campo '{field}' no lo lee el cargador; "
                    f"el que si se lee es '{parecido}'"
                )
            else:
                problems.append(
                    f"{candidate_id}: el campo '{field}' no lo lee el cargador"
                )
    assert not problems, "FAIL: campos que el cargador no lee:\n  " + "\n  ".join(problems)


def test_every_candidate_carries_evidence_and_a_criterion() -> None:
    """Las dos condiciones que el cargador exige para que un candidato sea trabajo.

    Sin evidencia no se puede comprobar nada; sin criterio de cierre observable
    no se puede saber si esta hecho. Las dos se descartan en silencio.
    """
    data = json.loads(_read())
    problems: list[str] = []
    for candidate in data["backlog"]:
        candidate_id = candidate.get("candidateId", "(sin identificador)")
        evidence = candidate.get("evidenceRefs")
        if not isinstance(evidence, list) or not evidence:
            problems.append(f"{candidate_id}: sin 'evidenceRefs' utilizable")
        done_when = candidate.get("doneWhen")
        if not isinstance(done_when, list) or not done_when:
            problems.append(f"{candidate_id}: sin criterio de cierre observable")
    assert not problems, (
        "FAIL: candidatos que el cargador descartaria:\n  " + "\n  ".join(problems)
    )


def test_the_queue_is_not_written_backwards() -> None:
    """El selector ordena de mayor a menor prioridad.

    Una cola escrita al reves hace que la cadena empiece por el final: el
    primer candidato del fichero tiene que llevar el numero mas alto.
    """
    data = json.loads(_read())
    priorities = [c.get("priority") for c in data["backlog"]]
    assert all(isinstance(p, int) for p in priorities), (
        "FAIL: hay candidatos sin prioridad numerica."
    )
    assert priorities == sorted(priorities, reverse=True), (
        f"FAIL: la cola esta escrita al reves. El selector ordena de mayor a "
        f"menor, asi que el primer candidato ({priorities[0]}) tiene que llevar "
        f"el numero mas alto, no el mas bajo. Orden actual: {priorities}"
    )


def test_every_reference_points_at_a_real_file() -> None:
    """Una referencia que no apunta a nada no es evidencia.

    La ruta es relativa al proyecto donde vive la declaracion, y el ancla, si
    la hay, tiene que estar en el fichero.
    """
    data = json.loads(_read())
    problems: list[str] = []
    for candidate in data["backlog"]:
        candidate_id = candidate.get("candidateId", "(sin identificador)")
        for ref in candidate.get("evidenceRefs", []):
            path, anchor = _split_reference(str(ref))
            if not path:
                problems.append(f"{candidate_id}: la referencia '{ref}' esta vacia")
                continue
            resolved = (REPO_ROOT / path).resolve()
            if not resolved.exists():
                problems.append(
                    f"{candidate_id}: la referencia '{ref}' no apunta a ningun fichero"
                )
                continue
            if anchor and resolved.is_file():
                content = resolved.read_text(encoding="utf-8", errors="replace")
                if not _anchor_exists(content, anchor):
                    problems.append(
                        f"{candidate_id}: la referencia '{ref}' no encuentra el ancla "
                        f"'{anchor}' en {path}"
                    )
    assert not problems, (
        "FAIL: referencias que no apuntan a nada:\n  " + "\n  ".join(problems)
    )


def test_control_mission_all_green() -> None:
    """En estado nominal, las reglas anteriores pasan.

    Si esto falla sobre una declaracion sana, hay un bug en el contrato, no en
    la declaracion.
    """
    test_declaration_exists_and_parses()
    test_every_candidate_uses_the_fields_the_loader_reads()
    test_every_candidate_carries_evidence_and_a_criterion()
    test_the_queue_is_not_written_backwards()
    test_every_reference_points_at_a_real_file()


def main() -> int:
    module = sys.modules[__name__]
    checks = sorted(
        (
            getattr(module, name)
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda check: check.__code__.co_firstlineno,
    )
    assert checks, "ningun test_ encontrado: el contrato no comprobaria nada"
    failures: list[str] = []
    for check in checks:
        try:
            check()
            print(f"  PASS {check.__name__}")
        except AssertionError as err:
            failures.append(check.__name__)
            print(f"  FAIL {check.__name__}: {err}")
        except Exception as err:
            failures.append(check.__name__)
            print(f"  FAIL {check.__name__}: {type(err).__name__}: {err}")
    if failures:
        print(f"libro mayor y mision: FAIL ({len(failures)}/{len(checks)})")
        return 1
    print(f"libro mayor y mision: OK ({len(checks)} comprobaciones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
