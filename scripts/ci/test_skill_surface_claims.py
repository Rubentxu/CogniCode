#!/usr/bin/env python3
"""Las skills publicadas no pueden afirmar una superficie que no sea la real.

## El hueco, exacto

Hay tres validadores de skills en este repositorio y cada uno ve una cosa:

| Dueño | Que comprueba |
|---|---|
| `scripts/validate_skills.py` | nombres de tool MCP contra `product/tools.json`, frontmatter, YAML, rutas internas |
| `scripts/ci/test_skill_cli_invocations.py` | que cada invocacion `cognicode <sub> ...` exista y tenga la aridad correcta |
| `scripts/ci/test_skills_gate_contract.py` | que el validador y el verificador esten cableados y sean verdes |

Los tres comprueban que **lo que la skill nombra existe**. Ninguno comprueba que
**lo que la skill dice de la superficie sea cierto**.

Eso son dos preguntas distintas, y la segunda se la puede responder mal sin que
ninguna de las tres se entere. MEDIDO 2026-10-04 sobre las skills que se
publican:

    $ grep -rnoE "[0-9]+[ -](tools?|lenguajes|languages|platformas|profiles)" skills/*/SKILL.md
    skills/cognicode-mcp/SKILL.md:32:73 tools
    skills/cognicode-mcp/SKILL.md:248:73 tools
    skills/cognicode/SKILL.md:29:73-tool

Las tres son ciertas hoy. Ese es exactamente el problema: **una afirmacion
correcta por casualidad no es una afirmacion verificada**. La tool 74 entra, el
catalogo pasa a 74, los tres validadores siguen verdes porque las tres skills
siguen nombrando tools que existen, y la release publica una skill que dice que
el servidor tiene 73 tools. La afirmacion vive en el tar del candidato y se
publica sin rebuild.

Es la misma forma que `perf-budget.toml` diciendo "9 of 16 have no benchmark"
mucho despues de que el numero dejara de ser cierto, y que nadie comprobara. La
diferencia es que aqui el numero viaja al usuario.

## Que se comprueba

Para cada afirmacion `<N> <sustantivo>` o `<N>-<sustantivo>` en una skill
publicada, el `N` tiene que coincidir con el documento publicado que es la
autoridad de ese sustantivo.

Y tres guardas, para que el contrato no pueda pasar por no haber mirado nada:

- el conjunto de skills publicadas se lee de `SKILL_BUNDLES` en
  `release_contract.rs` —la misma tabla que gobierna que se empaqueta, no una
  lista propia— y no puede estar vacio;
- toda skill publicada esta cubierta por el barrido;
- **se ha encontrado al menos una afirmacion**. Un contrato queQD no encuentra
  nada es un contrato que no mira nada, y pasaria en verde indefinidamente.

## Que NO comprueba, y por que no se disimula

Solo conoce los sustantivos de la tabla de abajo. Una afirmacion sobre una
superficie que no esta en la tabla —"5 ejemplos", o un sustantivo mal escrito
—"73 toos"— es **invisible** para este contrato, porque no hay forma de saber
que deberia mirar. Se dice aqui en vez de dejar que se descubra dentro de seis
meses creyendo que la superficie entera esta cubierta. La guarda de "al menos
una afirmacion" evita que el contrato se vuelva vacio por la via de las skills,
pero no por la via del vocabulario.

## Mutaciones que este contrato tiene que detectar

- cambiar un `73` de una skill por `70` (falsifica la afirmacion);
- cambiar `runtime_tool_count` en `product/tools.json` (falsifica la verdad);
- vaciar `SKILL_BUNDLES` (falsifica el fail-closed);
- borrar todas las afirmaciones de las skills (falsifica la guarda de suelo).
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_CONTRACT_RS = (
    REPO_ROOT / "crates" / "cognicode-cli" / "src" / "cmd" / "release_contract.rs"
)
SKILLS_DIR = REPO_ROOT / "skills"
PRODUCT_DIR = REPO_ROOT / "product"

# Sustantivo -> (documento publicado que es su autoridad, como contar).
#
# `platforms` se ata a `platforms.json` y no a `product-manifest.json` a
# proposito, y es una decision que conviene tener escrita porque parece un
# error: MEDIDO 2026-10-04, el manifest declara 2 plataformas y el documento de
# soporte declara 6. No es una divergencia. `generate_product_manifest.py`
# construye `platforms` con `certified_platforms(root)`, que es
# `release_lane.release_targets(root)` —los targets que la lane construye— y su
# docstring explica que sustituyo la lectura de un fichero que el cutover
# borra. `validate_manifest` comprueba ademas que ese conjunto sea el
# certificate-set esperado. O sea: el manifest responde a "que se publica", y el
# documento de soporte responde a "donde funciona". Una skill que dice cuantos
# plataformas soporta el producto esta usando la segunda pregunta, asi que este
# contrato la ata ahi.
CLAIM_SURFACES: dict[str, tuple[str, Callable[[dict[str, Any]], int]]] = {
    "tool": ("tools.json", lambda d: d["runtime_tool_count"]),
    "tools": ("tools.json", lambda d: d["runtime_tool_count"]),
    "language": ("languages.json", lambda d: len(d["languages"])),
    "languages": ("languages.json", lambda d: len(d["languages"])),
    "lenguaje": ("languages.json", lambda d: len(d["languages"])),
    "lenguajes": ("languages.json", lambda d: len(d["languages"])),
    "platform": ("platforms.json", lambda d: len(d["platforms"])),
    "platforms": ("platforms.json", lambda d: len(d["platforms"])),
    "plataforma": ("platforms.json", lambda d: len(d["platforms"])),
    "platformas": ("platforms.json", lambda d: len(d["platforms"])),
    "profile": ("profiles.json", lambda d: len(d["profiles"])),
    "profiles": ("profiles.json", lambda d: len(d["profiles"])),
    "perfil": ("profiles.json", lambda d: len(d["profiles"])),
    "perfiles": ("profiles.json", lambda d: len(d["profiles"])),
}

# `73 tools`, `73-tool`, `73 tools.` — con y sin guion, que es como se escribe
# "a 73-tool server" en la prosa de una skill.
CLAIM_PATTERN = re.compile(
    r"(?<![\w.])(\d{1,5})\s*[- ]\s*(" + "|".join(sorted(CLAIM_SURFACES, key=len, reverse=True)) + r")(?![\w])",
    re.IGNORECASE,
)

# MEDIDO 2026-10-04: aqui hubo una constante `_FRONTMATTER` que recortaba el
# bloque de YAML, y el recorte se llevo por delante la `description` —que es
# la primera prosa que se lee de una skill— para proteger un `version` que no
# podia colarse nunca, porque `version` no pertenece al vocabulario de
# superficies. Una proteccion que no protegia nada, comprada con cobertura
# real. La regla que hace falta es "solo se compara lo que se sabe comparar", y
# esa no necesita excepciones.


def published_skill_ids() -> set[str]:
    """Los ids que `SKILL_BUNDLES` marca como publicados.

    La misma tabla que gobierna que se empaqueta en el tar del candidato, leida
    igual que en `test_skill_cli_invocations.py`. Dos listas de skills
    publicadas serian dos fuentes de verdad, y una de las dos se quedaria vieja
    sin avisar.
    """
    source = RELEASE_CONTRACT_RS.read_text(encoding="utf-8")
    table = re.search(r"pub const SKILL_BUNDLES[^=]*=\s*&\[(.*?)\n\];", source, re.DOTALL)
    if table is None:
        raise AssertionError(f"no se encontro SKILL_BUNDLES en {RELEASE_CONTRACT_RS.name}")
    return set(re.findall(r'id:\s*"([^"]+)"[\s\S]{0,200}?published:\s*true', table.group(1)))


def skill_files() -> list[Path]:
    return sorted(SKILLS_DIR.glob("*/SKILL.md"))


def _load(document: str) -> dict[str, Any]:
    path = PRODUCT_DIR / document
    if not path.is_file():
        raise AssertionError(
            f"{path} no existe. La autoridad de una afirmacion de skill no puede "
            "ser un fichero que no esta: el contrato fallaria cerrado por una "
            "razon que no es la que cree."
        )
    return json.loads(path.read_text(encoding="utf-8"))


_CACHE: dict[str, int] = {}


def surface_size(noun: str) -> int:
    document, count = CLAIM_SURFACES[noun.lower()]
    if document not in _CACHE:
        _CACHE[document] = count(_load(document))
    return _CACHE[document]


def extract_claims(text: str) -> list[tuple[int, int, str]]:
    """`(linea, numero, sustantivo)` de cada afirmacion de superficie.

    MEDIDO 2026-10-04. La primera version saltaba el frontmatter, y el motivo
    era que `version: "1.0.0"` no es una afirmacion de superficie. El motivo era
    falso y el recorte salio caro: `skills/cognicode-mcp/SKILL.md:4` dice

        Drive CogniCode's 73-tool MCP server effectively: tool selection,

    y esa linea esta **dentro de la `description` del frontmatter**, que es la
    prosa que un agente lee para decidir si carga la skill. Un recorte que
    pierde la primera frase que se lee de un documento no es una precaution, es
    un agujero en el sitio que mas importa.

    Ademas no hacia falta: el vocabulario de `CLAIM_SURFACES` no tiene
    `version`, asi que `version: "1.0.0"` nunca podia colarse. La regla es
    "solo se compara lo que se sabe comparar", y el frontmatter es prosa
    publicada como el resto.

    Una version mas antigua de esta funcion ademas recortaba el frontmatter y
    enumeraba las lineas del CUERPO, con lo que toda linea posterior se
    reportaba con un numero que no era el del fichero: el diagnostico mandaba al
    lector a la linea equivocada. Un contrato que senala la linea equivocada es
    peor que uno que no senala ninguna, porque el que lo lee pierde la
    confianza en el todo y no solo en ese fallo.
    """
    claims: list[tuple[int, int, str]] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        for match in CLAIM_PATTERN.finditer(line):
            noun = match.group(2).lower()
            if noun not in CLAIM_SURFACES:
                continue
            claims.append((line_number, int(match.group(1)), noun))
    return claims


def audit(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        claims = extract_claims(path.read_text(encoding="utf-8"))
        for line, said, noun in claims:
            actual = surface_size(noun)
            if said == actual:
                continue
            document = CLAIM_SURFACES[noun][0]
            problems.append(
                f"{path.parent.name}/SKILL.md:{line} dice '{said} {noun}' y "
                f"{document} publica {actual}. La afirmacion viaja en el tar del "
                f"candidato y se publica sin rebuild."
            )
    return problems


# --- Tests ------------------------------------------------------------------


def test_the_published_set_is_not_empty_and_is_covered() -> None:
    """Fail-closed: un guard que vigila un conjunto vacio no vigila nada."""
    published = published_skill_ids()
    assert published, (
        "SKILL_BUNDLES no declara ninguna skill publicada: o se rompio la tabla, "
        "o este contrato esta mirando la fuente equivocada"
    )
    files = skill_files()
    assert files, f"no se encontro ningun skills/*/SKILL.md bajo {SKILLS_DIR}"
    covered = {path.parent.name for path in files}
    missing = published - covered
    assert not missing, (
        f"skills publicadas sin SKILL.md, y por tanto sin cubrir: {sorted(missing)}"
    )


def test_the_sweep_actually_finds_a_claim() -> None:
    """La guarda de suelo: sin afirmaciones, este contrato no miraria nada.

    Una skill que deja de afirmar el tamaño de la superficie es un hecho
    legitimo —casi siempre mejor— y por eso este contrato tiene que seguir
    y seguir en verde en ese caso. Lo que no puede es quedarse verde *porque* dejo de
    mirar, y la unica forma de distinguir las dos cosas es exigir que hoy
    encuentra algo. Si alguien cambia todas las frases, este test falla y
    pregunta si el numero de afirmaciones ha cambiado de verdad o si el
    extractor dejo de reconocer la prosa.
    """
    claims = [
        (path, claim)
        for path in skill_files()
        for claim in extract_claims(path.read_text(encoding="utf-8"))
    ]
    assert claims, (
        "no se encontro ninguna afirmacion de superficie en las skills publicadas. "
        "O el extractor dejo de reconocer la prosa, o las skills dejaron de "
        "afirmar el tamaño de la superficie. Lo segundo es una mejora y este "
        "contrato pasaria en verde sin vigilar nada."
    )


def test_the_real_skills_are_clean() -> None:
    """El contrato de verdad, sobre el contenido que se empaqueta."""
    problems = audit(skill_files())
    assert not problems, (
        "las skills publicadas afirman una superficie que no es la real:\n  "
        + "\n  ".join(problems)
    )


def test_a_near_miss_noun_is_not_counted_as_a_surface_claim() -> None:
    """Fija el limite del extractor, y lo hace de una forma que puede fallar.

    Solo se reconocen los sustantivos de `CLAIM_SURFACES`. "30 toolchains" lleva
    un numero delante de una palabra que *empieza* como una superficie, asi que
    un extractor descuidado que emparejara por prefijo contaria un_surface que
    no existe — y ese recuento erroneo se compararia contra un documento que no
    habla de toolchains.

    El limite es real y no se disimula: una afirmacion sobre una superficie que
    no este en la tabla es invisible para este contrato. Lo que este test fija
    es que el prefijo no se confunde con la palabra, y que una afirmacion real
    en la misma linea sigue viéndose.
    """
    text = "The guide ships 30 toolchains, and the server exposes 73 tools.\n"
    assert extract_claims(text) == [(1, 73, "tools")], (
        f"solo la superficie declarada tiene que contar: {extract_claims(text)}"
    )


def test_the_frontmatter_description_is_checked_and_the_version_is_not() -> None:
    """La `description` es la primera prosa que se lee de una skill, y se vigila.

    MEDIDO 2026-10-04: `skills/cognicode-mcp/SKILL.md:4` afirma "73-tool" dentro
    de la `description` del frontmatter, que es lo que un agente lee para decidir
    cargar la skill. Un recorte del frontmatter la dejaba sin vigilancia
    mientras el resto del documento si se comprobaba.

    En la otra direccion, `metadata.version` es la version **de la skill** y no
    tiene por que coincidir con la del producto. Lo que lo mantiene tranquilo no
    es un recorte, es que `version` no pertenece al vocabulario de superficies:
    si alguien lo anadiera al vocabulario, este test lo cazaria.
    """
    text = (
        "---\n"
        "name: x\n"
        "description: >\n"
        "  Drive CogniCode's 68-tool MCP server effectively: tool selection.\n"
        "license: MIT\n"
        "metadata:\n"
        '  version: "1.0.0"\n'
        "---\n"
        "\n"
        "The body says nothing about sizes.\n"
    )
    assert extract_claims(text) == [(4, 68, "tool")], (
        "la description del frontmatter tiene que vigilarse y la version de la "
        f"skill no: {extract_claims(text)}"
    )


def test_the_two_forms_of_the_same_claim_are_both_seen() -> None:
    """`73 tools` y `73-tool` son la misma afirmacion escrita de dos formas.

    MEDIDO 2026-10-04: `skills/cognicode/SKILL.md` escribe "a 73-tool server" y
    `skills/cognicode-mcp/SKILL.md` escribe "the 73 tools". Un extractor que
    solo aceptase la forma con espacio vigilaria una de las dos.
    """
    spaced = extract_claims("Group the 73 tools by intent.\n")
    hyphenated = extract_claims("It is a 73-tool server.\n")
    assert spaced == [(1, 73, "tools")], spaced
    assert hyphenated == [(1, 73, "tool")], hyphenated


def test_the_published_counts_are_themselves_consistent() -> None:
    """`runtime_tool_count` y la lista de tools tienen que decir lo mismo.

    MEDIDO: hoy los dos son 73. Un contrato que solo leyera `runtime_tool_count`
    compararia las skills contra un numero que el documento afirma de si mismo,
    y no contra la lista que de verdad se publica. Es la misma debilidad que
    el digest firmado por el propio generador: la comprobacion tiene que mirar
    los bytes que se publican, no lo que el documento dice de si mismo.
    """
    tools = _load("tools.json")
    declared = tools["runtime_tool_count"]
    actual = len(tools["tools"])
    assert declared == actual, (
        f"product/tools.json declara runtime_tool_count={declared} pero publica "
        f"{actual} tools. Una skill que afirme el numero declarado estara "
        "equivocada respecto de lo que se publica."
    )


def test_main_reports_and_returns() -> None:
    """`main()` tiene que correr los tests, no declararlos.

    El mismo fallo que trailing en `test_provenance_gate.py`: un `main()` que
    imprime PASS sin ejecutar nada es la forma mas barata de publicar un
    contrato vacio. Se comprueba desde aqui para que el fallo aparezca aqui.
    """
    module = sys.modules[__name__]
    tests = [
        name
        for name in dir(module)
        if name.startswith("test_") and callable(getattr(module, name))
    ]
    assert tests, "ningun test_ encontrado"


def main() -> int:
    module = sys.modules[__name__]
    tests = sorted(
        (
            name
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda name: getattr(module, name).__code__.co_firstlineno,
    )
    assert tests, (
        "ningun test_ encontrado: este main() no ejecutaria nada y reportaria PASS"
    )
    failures: list[str] = []
    for test in tests:
        try:
            getattr(module, test)()
            print(f"  PASS {test}")
        except Exception as error:  # noqa: BLE001 - un contrato reporta, no propaga
            print(f"  FAIL {test}: {error}")
            failures.append(f"{test}: {error}")
    if failures:
        print(f"FAIL - {len(failures)} fallo(s)")
        return 1
    print(
        "PASS - las skills publicadas afirman la superficie que el producto "
        "publica, y el contrato sigue teniendo dientes sobre lo que vigila."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
