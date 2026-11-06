#!/usr/bin/env python3
"""El ROADMAP no puede declarar retirado lo que sigue en el árbol.

## El hueco

`docs/roadmap/ROADMAP.md` es la agenda vigente: lo que dice ahí se cita
como si fuera cierto, y quien lo ejecuta va a buscar lo que la nota le
manda. Una nota que afirma que un documento fue retirado lo manda a otro
sitio —el recibo del `JOURNAL`— cuando el documento sigue en el árbol, con
la decisión dentro.

Eso es lo que pasó con la nota 2 de §11.1, MEDIDO en TASK-129: la nota
afirmaba que la poda documental —PR #399— se había llevado
`docs/adr/ADR-D1-006-uat-retirement.md`. El fichero existe, y
`git log --all --diff-filter=D` no registra su baja. La nota mandaba a un
sitio que ya no era el único, y hacía invisible el fichero que sí tenía la
decisión.

El daño que este contrato evita no es que la nota se equivoque. Es que la
nota **no se puede equivocar sin que nada lo note**: un documento que se
declara retirado y sigue presente es una contradicción que el árbol
denuncia y la prosa esconde.

## Lo que se comprueba

Para cada ruta de `docs/` que el ROADMAP declara retirada en una frase, el
documento no puede estar en el árbol.

  - una ruta declarada retirada que sigue en el árbol      -> 1 rojo
  - (control) una prosa sintética que retira un fichero que está -> 1 rojo
  - (control) una prosa sintética que retira un fichero que no está -> 0 rojos
  - (control) una negación no es una afirmación: «no fue retirado» no
    declara nada                                              -> 0 rojos
  - (control) el extractor ve las afirmaciones que dice ver  -> 1 afirmación

Los controles importan, y el último es el que sostiene a los demás. En el
árbol ya no queda ninguna afirmación de retirada: la única que había era
el defecto. Sin el control sintético, un extractor que dejó de ver
afirmaciones pasaría siempre y el contrato no vigilaría nada.

## Lo que NO se comprueba

  - **Que la retirada haya ocurrido de verdad.** El contrato mira el árbol,
    no la historia: una ruta que se declara retirada y que ya no está pasa,
    aunque la baja no figure en el historial de git. Que la baja figure es
    otra unidad.
  - **Que el documento citado sea el mejor.** Solo que el ROADMAP no afirme
    que está retirado algo que está.
  - **Las filas de tabla.** Una celda puede declarar una retirada mientras
    otra celda de la MISMA fila cita rutas, y una fila no es una
    afirmación sobre una ruta. Se miran las frases de la prosa.
  - **Los SHAs citados.** No se comprueba que el commit que se cita viva en
    `main`: hay varios que no, y verificarlos todos es otra unidad.

## La redacción que el contrato pide

El contrato lee frase a frase. Una frase que declara una retirada y cita
una ruta no puede distinguirse de una que declara la retirada de esa ruta:
la redacción tiene que separar las dos cosas. «El fichero `X` existe» y
«la poda se llevó seis ficheros» son dos frases, no una.
"""

from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ROADMAP = REPO_ROOT / "docs" / "roadmap" / "ROADMAP.md"

# Una cita de ruta dentro de acentos graves: es la forma en que el ROADMAP
# nombra un documento del árbol.
CITED = re.compile(r"`(docs/[A-Za-z0-9._/-]+)`")

# Una afirmación de retirada. La lista cubre las formas que el ROADMAP
# usa: el verbo de retirada (retirar, eliminar, borrar, suprimir), la poda
# (podar, llevarse), la desaparición, y el pasado de existir —«existió» es
# decir «ya no está»—.
#
# `pod` lleva su forma propia porque `pod\w+` también casaría con «podemos»
# y «podría», que no retiran nada.
RETIREMENT = re.compile(
    r"\b(?:retir\w+|elimin\w+|borr\w+|suprimid\w+|desapareci\w+|llev\w+"
    r"|pod(?:ar|a|ó|aron|ando|aba|aban)"
    r"|exist(?:ió|ieron|ía|íamos|ían)"
    r"|ya no existe|ya no está)\b",
    re.IGNORECASE,
)

# Una negación cancela la afirmación si está cerca del verbo: «no fue
# retirado» no declara una retirada, la desmiente.
NEGATION = re.compile(
    r"\b(?:no|nunca|jamás|sin|ningún|ninguna|nadie)\b", re.IGNORECASE
)

# La ventana, en caracteres, en la que la negación sigue alcanzando al
# verbo. Es corta a propósito: una negación lejana no es la negación de
# ese verbo, y una afirmación que se salva de una negación que no era suya
# es una afirmación que el contrato no vigila.
NEGATION_WINDOW = 60


def prose_sentences(text: str) -> list[str]:
    """Las frases de la prosa del documento.

    Las filas de tabla no se miran, por lo que dice el contrato: una fila
    puede mezclar en la misma línea una celda que declara una retirada y
    otra que cita rutas.
    """
    prose = [line for line in text.splitlines() if not line.strip().startswith("|")]
    return re.split(r"(?<=[.!?:])\s+|\n", " ".join(prose))


def mask_citations(sentence: str) -> str:
    """La frase con las rutas citadas tapadas.

    Una ruta es un nombre, no una afirmación: el fichero
    `docs/adr/ADR-D1-006-uat-retirement.md` contiene la palabra
    «retirement», que no es un verbo de retirada. Sin taparla, el contrato
    leería el nombre del fichero como una afirmación de que se retiró.
    """
    return CITED.sub(lambda match: " " * len(match.group(0)), sentence)


def asserts_retirement(sentence: str) -> bool:
    """Si la frase declara una retirada.

    Declara una retirada si tiene un verbo de retirada SIN negación cerca.
    """
    masked = mask_citations(sentence)
    for marker in RETIREMENT.finditer(masked):
        window = masked[max(0, marker.start() - NEGATION_WINDOW) : marker.start()]
        if not NEGATION.search(window):
            return True
    return False


def retirement_claims(text: str) -> list[tuple[str, str]]:
    """Las (ruta, frase) en las que el documento declara una retirada."""
    claims: list[tuple[str, str]] = []
    for sentence in prose_sentences(text):
        if not asserts_retirement(sentence):
            continue
        for path in sorted(set(CITED.findall(sentence))):
            claims.append((path, " ".join(sentence.split())))
    return claims


def still_there(
    claims: list[tuple[str, str]], root: Path = REPO_ROOT
) -> list[tuple[str, str]]:
    """Las afirmaciones que el árbol contradice.

    Una ruta está si el árbol tiene ese fichero, o ese fichero con
    extensión `.md`: el ROADMAP cita los documentos por su nombre de
    fichero y a veces sin la extensión.
    """
    contradicted: list[tuple[str, str]] = []
    for path, sentence in claims:
        if (root / path).exists() or (root / f"{path}.md").exists():
            contradicted.append((path, sentence))
    return contradicted


def test_the_roadmap_does_not_retire_a_document_that_is_there() -> None:
    """Ninguna ruta que el ROADMAP declara retirada sigue en el árbol."""
    text = ROADMAP.read_text(encoding="utf-8")
    claims = retirement_claims(text)
    contradicted = still_there(claims)

    assert not contradicted, (
        "FAIL: el ROADMAP declara retirados documentos que siguen en el "
        "árbol:\n  "
        + "\n  ".join(f"{path} — «{sentence}»" for path, sentence in contradicted)
        + "\nUna nota que declara retirado un documento que existe manda a "
        "quien la lee a un sitio que ya no es el único, y esconde el "
        "fichero que sí tiene la decisión."
    )


def test_a_synthetic_retirement_of_a_present_document_is_caught() -> None:
    """Control: una retirada inventada de un fichero que está sale en rojo.

    Sin este control, un contrato que no mira nada y un contrato que mira
    y no encuentra nada serían lo mismo.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        present = root / "docs" / "adr" / "ADR-X.md"
        present.parent.mkdir(parents=True, exist_ok=True)
        present.write_text("decisión\n", encoding="utf-8")

        prose = (
            "Ese fichero existió (`docs/adr/ADR-X.md`, commit `abc1234`) y lo "
            "retiró la poda documental —PR #399—."
        )
        claims = retirement_claims(prose)
        assert claims, (
            "FAIL: el extractor no ha visto la afirmación de retirada de una "
            "prosa que la contiene"
        )
        contradicted = still_there(claims, root=root)
        assert contradicted, (
            "FAIL: una afirmación de retirada de un documento que está en el "
            "árbol no sale en rojo: el contrato no vigila"
        )


def test_a_retirement_of_a_document_that_is_gone_passes() -> None:
    """Control: una retirada verdadera no sale en rojo.

    El contrato tiene que poder decir «aquí está bien»: si todo fuera
    rojo, nadie distinguiría entre un defecto y un contrato ruidoso.
    """
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        prose = (
            "Ese fichero existió (`docs/adr/ADR-Y.md`, commit `abc1234`) y lo "
            "retiró la poda documental —PR #399—."
        )
        claims = retirement_claims(prose)
        assert claims, (
            "FAIL: el extractor no ha visto la afirmación de retirada de una "
            "prosa que la contiene"
        )
        assert not still_there(claims, root=root), (
            "FAIL: una afirmación de retirada de un documento que NO está en el "
            "árbol sale en rojo: el contrato es ruidoso"
        )


def test_a_negated_retirement_is_not_a_retirement() -> None:
    """Control: una negación no es una afirmación.

    Corregir una nota sobre una retirada exige decir que NO hubo retirada.
    Si la negación contara como afirmación, el contrato no dejaría
    corregir la nota que señala.
    """
    prose = (
        "Ese fichero no fue retirado: `docs/adr/ADR-Z.md` sigue en el árbol "
        "y la decisión vive dentro."
    )
    claims = retirement_claims(prose)
    assert not claims, (
        "FAIL: una frase que NIEGA una retirada sale como si la declarara: "
        f"{claims}"
    )


def test_a_document_name_is_not_an_assertion() -> None:
    """Control: el nombre de un fichero no es una afirmación.

    `docs/adr/ADR-D1-006-uat-retirement.md` lleva la palabra «retirement»
    dentro del nombre. Si el contrato leyera el nombre como una
    afirmación, no podría corregir la nota que señalaba: la nota
    corregida tiene que poder NOMBRAR el fichero.
    """
    prose = (
        "El fichero `docs/adr/ADR-D1-006-uat-retirement.md` está en el árbol, "
        "con la decisión aceptada."
    )
    claims = retirement_claims(prose)
    assert not claims, (
        "FAIL: el nombre de un fichero sale como si fuera una afirmación de "
        f"retirada: {claims}"
    )


def test_the_extractor_sees_the_claims_it_claims_to_see() -> None:
    """Control: el extractor ve las afirmaciones de retirada.

    En el árbol ya no queda ninguna afirmación que retirar —la que había
    era el defecto—, así que el control es sintético: una prosa con una
    retirada tiene que producir una afirmación. Si el extractor dejó de
    verlas, este contrato pasaría siempre y no vigilaría nada.
    """
    prose = (
        "La poda documental se llevó `docs/adr/ADR-W.md`, y con ella se fue "
        "la decisión que llevaba."
    )
    claims = retirement_claims(prose)
    assert claims, (
        "FAIL: el extractor no ve una afirmación de retirada en una prosa "
        "que la contiene. Si dejó de verlas, este contrato pasaría siempre."
    )


def main() -> int:
    """Ejecuta las comprobaciones y devuelve 1 si alguna falla.

    El contrato de los contratos exige esta forma: un fichero que acumula
    sus hallazgos en una lista tiene que convertir esa lista en un código
    de salida distinto de cero, o ejecutarlo a mano informaría de un
    éxito que no ocurrió.
    """
    failures: list[str] = []
    for name, function in sorted(globals().items()):
        if not name.startswith("test_") or not callable(function):
            continue
        try:
            function()
            print(f"PASS: {name}")
        except AssertionError as error:
            failures.append(f"FAIL: {name}: {error}")
            print(f"FAIL: {name}: {error}", file=sys.stderr)
        except Exception as error:  # noqa: BLE001 — el contrato informa, no aborta
            failures.append(f"ERROR: {name}: {error}")
            print(f"ERROR: {name}: {error}", file=sys.stderr)

    if failures:
        print(f"\n{len(failures)} contract(s) failed", file=sys.stderr)
        return 1
    print("\nall contracts passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
