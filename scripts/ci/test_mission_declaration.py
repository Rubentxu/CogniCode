#!/usr/bin/env python3
"""La declaracion de la mision tiene que hablar el idioma que el cargador lee.

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


def test_control_all_green() -> None:
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
    failures: list[str] = []
    for fn in (
        test_declaration_exists_and_parses,
        test_every_candidate_uses_the_fields_the_loader_reads,
        test_every_candidate_carries_evidence_and_a_criterion,
        test_the_queue_is_not_written_backwards,
        test_every_reference_points_at_a_real_file,
        test_control_all_green,
    ):
        try:
            fn()
        except AssertionError as err:
            failures.append(f"{fn.__name__}: {err}")
        except Exception as err:  # pragma: no cover - solo para ver el fallo
            failures.append(f"{fn.__name__}: {type(err).__name__}: {err}")

    if failures:
        print("MISSION declaration contract: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    data = json.loads(_read())
    print(
        f"MISSION declaration contract: OK "
        f"({len(data['backlog'])} candidato(s), "
        f"prioridad {data['backlog'][0].get('priority')}..{data['backlog'][-1].get('priority')})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
