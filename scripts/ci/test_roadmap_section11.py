#!/usr/bin/env python3
"""ROADMAP §11 (las unidades abiertas): su AUTORIDAD y su CIERRE.

Dos contratos sobre el mismo bloque de `docs/roadmap/ROADMAP.md`, en un
solo fichero. Cada uno conserva su propio extractor del bloque a proposito:
`open_units_block` (la autoridad: parrafo en blockquote, techo de 80 lineas)
y `section11_open_units` (el cierre: ids en negrita del trailer). Son dos
preguntas con dos delimitaciones, no una duplicacion que unificar sin medir.

# Parte 1 — La autoridad que §11 cita

La autoridad normativa que §11 cita tiene que existir y cubrir la unidad.

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
    contrato; lo hace la Parte 2 de este fichero, que es
    complementario y no solapado: aquel vigila el CIERRE, este la AUTORIDAD.

# Parte 2 — El cierre

La lista de trabajo abierto no puede sobrevivir a su propio cierre.

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

## Los ids de dos digitos

La lista de abiertas se extraia con `[A-Z]\\d(?:\\.\\d)?`, que casa con
`H5` y `H4.4` y **no** con `H23`: un digito y punto opcional. Una unidad
con dos digitos no era visible para la regla, de modo que el trailer
podia declararla abierta y el Journal cerrarla sin que nadie se enterara
-- que es exactamente el fallo que este contrato existe para cazar, en
una version mas pequena. La regexp ahora acepta el grupo de digitos
entero, y la regla vigila tambien las unidades nuevas.

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
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ROADMAP = ROOT / "docs/roadmap/ROADMAP.md"
JOURNAL = ROOT / "docs/roadmap/JOURNAL.md"

# --- Parte 1: la autoridad que §11 cita ------------------------------

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


# --- Parte 2: el cierre ---------------------------------------------

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
    return re.findall(r"\*\*([A-Z]\d+(?:\.\d)?)\*\*", m.group(0))


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
        for m in re.finditer(r"\b([A-Z]\d+(?:\.\d)?)\b", line):
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
        test_every_cited_authority_is_versioned,
        test_every_cited_adr_claims_the_unit_it_is_attributed,
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
        print("ROADMAP section 11: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    units = sorted(set(section11_open_units()))
    print(f"ROADMAP section 11: OK ({len(units)} abiertas: {', '.join(units)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
