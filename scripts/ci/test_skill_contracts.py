#!/usr/bin/env python3
"""Las skills publicadas no pueden ensenar un comando roto ni afirmar una superficie falsa.

Este contrato reune dos barridos sobre los mismos `skills/*/SKILL.md` que
`SKILL_BUNDLES` marca como publicados. Comparten el descubrimiento de skills
(una sola lista de skills publicadas, no dos) y cada barrido conserva su propio
vocabulario, su motivo y sus mutaciones.

Los otros validadores de skills ven otra cosa: `scripts/validate_skills.py`
(nombres de tool MCP contra `product/tools.json`, frontmatter, YAML, rutas
internas) y `scripts/ci/test_skills_gate_contract.py` (que el validador y el
verificador esten cableados y sean verdes). Este fichero cubre los dos huecos
que esos no ven.

Run: `python3 scripts/ci/test_skill_contracts.py`

# PARTE A: invocaciones de CLI

MEDIDO 2026-10-03. `skills/cognicode-pr-review/SKILL.md` enseñaba, dos veces,
el paso central de la skill:

    cognicode navigate references <symbol>

La firma real es `NavigateCommand::References { position: String }` y
`position` es `file:line:column`. El simbolo se enlaza a `position`,
`parse_position()` falla, y el brazo `Navigate` de `commands.rs` imprime

    Navigate command failed: Invalid position NoSuchPos: expected file:line:column

en stderr **y no hace `return Err`**, asi que el proceso sale con 0. Un agente
que sigue la skill recibe un exito y ninguna referencia. El mismo archivo tenia
`cognicode index symbol-code MySymbol`, cuya firma real es
`SymbolCode { file: String, line: u32, column: u32 }`: tres positionals
requeridos, uno dado, y clap sale con 2.

Que esto llegue al usuario no es hipotetico. `release-candidate.pipeline.kts`
empaqueta cada skill publicada en `staging/<id>-<version>.tar.gz`, y la release
publica **exactamente** el candidato, sin rebuild. Un candidato cortado antes
del arreglo publica la instruccion rota de forma permanente.

## Por que un contrato y no un comentario

`validate_skills.py` ya existe y es verde, y no puede ver esto: contrasta
nombres de tool MCP contra el catalogo. No mira invocaciones de CLI, y no mira
aridad de argumentos. El hueco no es que falte una comprobacion suelta, es que
nadie contrasta la documentacion contra la definicion que la documentacion
describe. Este contrato hace esa comparacion, y la hace leyendo las firmas
clap reales de `commands.rs` en cada ejecucion: no hay una lista de comandos
mantenida a mano que pueda quedarse vieja.

## Que se comprueba

Para cada invocacion `cognicode <sub> [<subsub>] ...` que aparece en un bloque
```bash de cualquier `skills/*/SKILL.md`:

- el subcomando existe en el enum real (con el nombre kebab-case que clap
  deriva del nombre de la variante);
- se suministraron todos los positionals **requeridos** por la firma. Un
  `#[arg(default_value = ...)]` o un `Option<T>` no cuenta;
- si el positional es de tipo posicion (`position`, documentado como
  `file:line:column`), el valor suministrado tiene que parecer una posicion.
  Esto es lo que separa `references <symbol>` de `references <file:line:column>`:
  los dos tienen la misma aridad, asi que un contrato que solo mirase el numero
  de argumentos dejaria pasar el defecto que motivo este contrato.

Y dosProperties de guarda, para que el contrato no pueda pasar por no haber
mirado nada:

- el conjunto de skills publicadas se lee de `SKILL_BUNDLES` en
  `release_contract.rs` —la misma tabla que gobierna que se empaqueta— y no
  puede estar vacio;
- toda skill publicada esta cubierta por el barrido.

## Mutaciones que este contrato tiene que detectar

- borrar el `default_value` de un campo opcional (falsifica la aridad);
- renombrar una variante de enum (falsifica la existencia del subcomando);
- poner `<symbol>` donde la firma pide `position` (falsifica el chequeo de
  posicion);
- vaciar `SKILL_BUNDLES` (falsifica el fail-closed).

# PARTE B: afirmaciones de superficie

## El hueco, exacto

Hay tres validadores de skills en este repositorio y cada uno ve una cosa:

| Dueño | Que comprueba |
|---|---|
| `scripts/validate_skills.py` | nombres de tool MCP contra `product/tools.json`, frontmatter, YAML, rutas internas |
| `scripts/ci/test_skill_contracts.py` (parte A) | que cada invocacion `cognicode <sub> ...` exista y tenga la aridad correcta |
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
import shlex
import sys
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parents[2]
COMMANDS_RS = REPO_ROOT / "crates" / "cognicode-core" / "src" / "interface" / "cli" / "commands.rs"
RELEASE_CONTRACT_RS = (
    REPO_ROOT / "crates" / "cognicode-cli" / "src" / "cmd" / "release_contract.rs"
)
SKILLS_DIR = REPO_ROOT / "skills"
PRODUCT_DIR = REPO_ROOT / "product"


# ---------------------------------------------------------------------------
# Descubrimiento compartido
# ---------------------------------------------------------------------------
def published_skill_ids() -> set[str]:
    """Los ids que `SKILL_BUNDLES` marca como publicados.

    Es la misma tabla que gobierna que se empaqueta en el tar del candidato, asi
    que leerla aqui no introduce una segunda fuente de verdad.
    """
    source = RELEASE_CONTRACT_RS.read_text(encoding="utf-8")
    table = re.search(r"pub const SKILL_BUNDLES[^=]*=\s*&\[(.*?)\n\];", source, re.DOTALL)
    if table is None:
        raise AssertionError(f"no se encontro SKILL_BUNDLES en {RELEASE_CONTRACT_RS.name}")
    return set(re.findall(r'id:\s*"([^"]+)"[\s\S]{0,200}?published:\s*true', table.group(1)))


def skill_files() -> list[Path]:
    return sorted(SKILLS_DIR.glob("*/SKILL.md"))


# Enums de clap que describen la superficie de la CLI. `CliCommand` cuelga de la
# raiz; los demas son los subcomandos de los que cuelgan por `#[command(subcommand)]`.
SUBCOMMAND_ENUMS = (
    "CliCommand",
    "IndexCommand",
    "GraphCommand",
    "NavigateCommand",
    "EvidenceCommand",
)

# Tipos que ocupan un positional. `bool` es un flag en la derivada de clap
# (necesita `--long`), asi que nunca cuenta como positional.
SCALAR_TYPES = ("String", "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize")

BASH_FENCE = re.compile(r"^\s*```\s*(?:bash|sh|shell)\s*$", re.IGNORECASE)
ANY_FENCE = re.compile(r"^\s*```")

# Flags que clap genera en la raiz y que no son subcomandos. `--version` solo
# cuenta si el atributo `version` esta presente, y se lee del fuente: quitarlo
# de `#[command(name = "cognicode", version)]` tiene que volver a poner en rojo
# la skill que lo use.
CLI_STRUCT = re.compile(r"pub struct Cli\b")

# Un positional de tipo posicion se documenta asi. El valor que se le pase
# tiene que contener al menos un `:`: `<symbol>` no lo tiene, `<file:line:column>`
# si. Es una regla deliberadamente burda, y esa es su virtud: no intenta adivinar
# la intencion de quien escribe la skill, solo comprueba que la forma escrita se
# parezca a la forma documentada.
POSITION_HINT = re.compile(r"\bposition\b|file:line:column", re.IGNORECASE)


# ---------------------------------------------------------------------------
# Firmas reales
# ---------------------------------------------------------------------------
def _enum_body(source: str, enum_name: str) -> str:
    """Devuelve el cuerpo `{...}` del enum pedido, contando llaves de verdad."""
    match = re.search(r"pub enum %s\b" % re.escape(enum_name), source)
    if match is None:
        raise AssertionError(
            f"no se encontro `pub enum {enum_name}` en {COMMANDS_RS.name}: el "
            "contrato no puede leer las firmas y pasaria por verde sin mirar nada"
        )
    start = source.index("{", match.start())
    depth = 0
    for index in range(start, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    raise AssertionError(f"el enum {enum_name} no cierra sus llaves")


def _kebab(variant: str) -> str:
    """Como clap deriva el nombre del CLI de una variante: `SymbolCode` -> `symbol-code`."""
    return re.sub(r"(?<!^)(?=[A-Z])", "-", variant).lower()


def cli_root_flags(source: str) -> set[str]:
    """Flags que la raiz de la CLI acepta sin subcomando.

    `--help` lo pone clap siempre. `--version` solo si el atributo `version`
    esta en el `#[command(...)]` del struct, y se lee de ahi a proposito: si
    alguien lo quita, esta lista deja de ofrecer `--version` y la skill que lo
    use vuelve a fallar, que es lo que tiene que pasar.
    """
    flags = {"-h", "--help"}
    struct = CLI_STRUCT.search(source)
    if struct is None:
        return flags
    head = source[max(0, struct.start() - 400) : struct.start()]
    if re.search(r"(?<![\w])version(?![\w])", head):
        flags |= {"-V", "--version"}
    return flags


def _fields(variant_body: str) -> list[dict]:
    """Campos de una variante, con su bloque de atributos inmediatamente anterior.

    El bloque es lo que decide si el campo es un positional requerido: un
    `#[arg(default_value = ...)]` en ese bloque lo hace opcional, y da igual que
    el atributo ocupe una linea o cinco.
    """
    fields: list[dict] = []
    # Trocea por lineas de declaracion de campo, a 8 espacios de sangria.
    pattern = re.compile(r"^        (\w+):\s*([\w:<>, ]+?),?\s*$", re.MULTILINE)
    matches = list(pattern.finditer(variant_body))
    for position, match in enumerate(matches):
        attributes_start = matches[position - 1].end() if position else 0
        attributes = variant_body[attributes_start : match.start()]
        fields.append(
            {
                "name": match.group(1),
                "type": match.group(2).strip(),
                "attributes": attributes,
            }
        )
    return fields


def parse_specs(source: str) -> dict[str, dict[str, dict]]:
    """`{enum: {nombre-cli: spec}}` desde el fuente real de los comandos.

    spec = {
        "required": int,        # positionals requeridos
        "optional": int,        # positionals con default
        "subcommand": str|None, # enum de subcomandos colgante, si lo hay
        "value_flags": set,     # --flags que consumen un valor
        "position_fields": set, # positionals de tipo posicion
    }
    """
    specs: dict[str, dict[str, dict]] = {}
    for enum in SUBCOMMAND_ENUMS:
        body = _enum_body(source, enum)
        variants: dict[str, dict] = {}
        # La cabeza de una variante abre con `{` o con `(`. Hace falta las dos:
        # `Evidence(EvidenceCommand)` es una variante de tupla, y si solo se
        # reconoce la forma de llaves sus campos se cuelan en la variante
        # anterior y esa variante parece colgar de subcomandos.
        heads = list(re.finditer(r"^    (\w+)\s*[({]", body, re.MULTILINE))
        # Y una variante de tupla lleva su tipo en la cabeza, no en una linea de
        # campo, asi que se lee aparte: `Evidence(EvidenceCommand)`.
        tuple_heads = {
            match.group(1): match.group(2)
            for match in re.finditer(r"^    (\w+)\s*\(\s*([\w:<>]+)\s*\)", body, re.MULTILINE)
        }
        for position, head in enumerate(heads):
            variant = head.group(1)
            end = heads[position + 1].start() if position + 1 < len(heads) else len(body)
            segment = body[head.end() : end]

            spec = {
                "required": 0,
                "optional": 0,
                "subcommand": None,
                "value_flags": set(),
                "position_fields": set(),
            }
            if variant in tuple_heads:
                spec["subcommand"] = tuple_heads[variant]
            for field in _fields(segment):
                name, ftype, attributes = field["name"], field["type"], field["attributes"]
                if ftype.startswith("Option<"):
                    continue
                # `bool` en la derivada de clap es un flag: necesita `--long`, no
                # ocupa un positional, y no es un holder de subcomandos.
                if ftype == "bool":
                    continue
                if ftype not in SCALAR_TYPES:
                    # Un tipo que no es escalar, ni Option, ni bool es el holder
                    # de un `#[command(subcommand)]`, no un argumento.
                    spec["subcommand"] = ftype
                    continue

                takes_value = "long" in attributes or "short" in attributes
                has_default = "default_value" in attributes
                if takes_value:
                    # `--long` con tipo escalar: solo es un flag de verdad si es
                    # bool, y bool ya se descarto arriba. Aqui es un valor.
                    spec["value_flags"].add("--" + name.replace("_", "-"))
                elif has_default:
                    spec["optional"] += 1
                else:
                    spec["required"] += 1
                    if POSITION_HINT.search(attributes) or name == "position":
                        spec["position_fields"].add(name)
            variants[_kebab(variant)] = spec
        specs[enum] = variants
    return specs


# ---------------------------------------------------------------------------
# Invocaciones
# ---------------------------------------------------------------------------
def extract_invocations(text: str) -> list[tuple[int, list[str]]]:
    """`cognicode ...` que aparece en un bloque de shell, con su numero de linea.

    Solo dentro de cercas ```bash: una mencion en prosa no es un comando que
    nadie ejecuta, y hacerlas fallar seria un falso positivo que enseña a la
    gente a no correr el contrato.
    """
    found: list[tuple[int, list[str]]] = []
    inside = False
    for number, line in enumerate(text.splitlines(), start=1):
        if not inside and BASH_FENCE.match(line):
            inside = True
            continue
        if inside and ANY_FENCE.match(line):
            inside = False
            continue
        if not inside:
            continue
        stripped = line.strip()
        if not stripped.startswith("cognicode "):
            continue
        try:
            # `comments=True` porque `#` abre un comentario en shell cuando
            # empieza una palabra. Sin esto, el texto de un comentario al final
            # de la linea —que es justo como se anotan estas skills— se cuenta
            # como argumentos del comando.
            tokens = shlex.split(stripped, comments=True)
        except ValueError:
            continue
        # `cognicode-mcp` es otro binario; `cogh` es otro binario.
        if tokens and tokens[0] == "cognicode":
            found.append((number, tokens[1:]))
    return found


def check_invocation(
    tokens: list[str],
    specs: dict[str, dict[str, dict]],
    root_flags: set[str] | None = None,
) -> list[str]:
    """Problemas de una invocacion. Vacio significa que la invocacion es valida."""
    problems: list[str] = []
    if not tokens:
        return ["invocacion vacia"]

    top, root = tokens[0], specs["CliCommand"]

    # Un flag global (`cognicode --version`) no es un subcomando: se resuelve
    # contra los flags de la raiz y se acepta. Un flag que no exista se rechaza.
    if top.startswith("-") and top != "-":
        accepted = root_flags if root_flags is not None else {"-h", "--help", "-V", "--version"}
        if top not in accepted:
            return [
                f"`cognicode {top}` no es un subcomando ni un flag global; "
                f"los flags globales son: {', '.join(sorted(accepted))}"
            ]
        return []

    if top not in root:
        known = ", ".join(sorted(root))
        return [f"`cognicode {top}` no existe; los subcomandos son: {known}"]

    spec = root[top]
    rest = tokens[1:]

    if spec["subcommand"] is not None:
        if not rest:
            return [f"`cognicode {top}` necesita un subcomando"]
        sub, rest = rest[0], rest[1:]
        if spec["subcommand"] not in specs:
            raise AssertionError(
                f"`cognicode {top}` cuelga del enum {spec['subcommand']}, que no "
                f"esta en {', '.join(SUBCOMMAND_ENUMS)}; el contrato no lo leeria "
                "y rechazaria toda invocacion de ese subcomando por la razon "
                "equivocada"
            )
        table = specs[spec["subcommand"]]
        if sub not in table:
            known = ", ".join(sorted(table))
            return [f"`cognicode {top} {sub}` no existe; los subcomandos son: {known}"]
        spec = table[sub]
        label = f"cognicode {top} {sub}"
    else:
        label = f"cognicode {top}"

    # Cuenta positionals reales: un token que no empieza por `-` y al que no
    # precede un `--flag` que consuma valor.
    positionals: list[str] = []
    skip_next = False
    for token in rest:
        if skip_next:
            skip_next = False
            continue
        if token.startswith("-") and token != "-":
            if token in spec["value_flags"]:
                skip_next = True
            continue
        positionals.append(token)

    if len(positionals) < spec["required"]:
        problems.append(
            f"`{label}` pide {spec['required']} positional(es) obligatorios y la "
            f"skill suministra {len(positionals)}: {positionals or 'ninguno'}"
        )
        return problems

    # Y el exceso tambien es un error: clap rechaza un argumento que la firma no
    # declara. Sin esta comprobacion, convertir `position` en un flag
    # (`#[arg(long)] position: String`) dejaria verde una skill que le pasa un
    # valor que el comando ya no acepta.
    accepted = spec["required"] + spec["optional"]
    if len(positionals) > accepted:
        problems.append(
            f"`{label}` acepta como maximo {accepted} positional(es) y la skill "
            f"suministra {len(positionals)}: {positionals}"
        )
        return problems

    # Los positionals requeridos son los primeros, en orden de declaracion.
    if spec["position_fields"]:
        for value in positionals[: spec["required"]]:
            if ":" not in value:
                problems.append(
                    f"`{label}` recibe un positional de tipo posicion "
                    f"(file:line:column) y la skill pasa {value!r}, que no lo es; "
                    "el comando enlazara el valor y fallara al parsearlo"
                )
    return problems


def audit_invocations(specs: dict[str, dict[str, dict]], files: list[Path]) -> list[str]:
    flags = cli_root_flags(COMMANDS_RS.read_text(encoding="utf-8"))
    problems: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        for line, tokens in extract_invocations(text):
            for problem in check_invocation(tokens, specs, flags):
                problems.append(f"{path.relative_to(REPO_ROOT)}:{line}: {problem}")
    return problems



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


def audit_claims(files: list[Path]) -> list[str]:
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



# ---------------------------------------------------------------------------
# Tests de la parte A (invocaciones)
# ---------------------------------------------------------------------------
# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
SPECS: dict[str, dict[str, dict]] | None = None
FILES: list[Path] | None = None
ROOT_FLAGS: set[str] | None = None


def _specs() -> dict[str, dict[str, dict]]:
    global SPECS
    if SPECS is None:
        SPECS = parse_specs(COMMANDS_RS.read_text(encoding="utf-8"))
    return SPECS


def _root_flags() -> set[str]:
    global ROOT_FLAGS
    if ROOT_FLAGS is None:
        ROOT_FLAGS = cli_root_flags(COMMANDS_RS.read_text(encoding="utf-8"))
    return ROOT_FLAGS


def test_the_parser_reads_the_real_signatures() -> None:
    """Sin esto, todo lo demas pasa por verde sobre un parser roto."""
    specs = _specs()
    assert "analyze" in specs["CliCommand"], "no se leyo el enum CliCommand"
    assert "references" in specs["NavigateCommand"], "no se leyo NavigateCommand"
    assert specs["NavigateCommand"]["references"]["required"] == 1, (
        "References recibe `position` sin default, asi que exige 1 positional; "
        f"el parser leyo {specs['NavigateCommand']['references']['required']}"
    )
    assert specs["IndexCommand"]["symbol-code"]["required"] == 3, (
        "SymbolCode recibe file+line+column, 3 positionales; el parser leyo "
        f"{specs['IndexCommand']['symbol-code']['required']}"
    )
    assert specs["GraphCommand"]["entry-points"]["required"] == 0, (
        "EntryPoints no tiene positionales requeridos; el parser leyo "
        f"{specs['GraphCommand']['entry-points']['required']}"
    )


def test_a_tuple_variant_does_not_leak_into_the_previous_one() -> None:
    """`Evidence(EvidenceCommand)` es una variante de tupla, no de llaves.

    Si el parser solo reconoce la forma `Nombre {`, los campos de `Evidence` se
    atribuyen a `FindUsages`, y `find-usages` —que no cuelga de ningun
    subcomando— pasa a parecer que si. El sintoma es desconcertante: el
    contrato rechaza `cognicode find-usages <symbol>` diciendo que no existen
    subcomandos, con la lista vacia.
    """
    specs = _specs()
    assert specs["CliCommand"]["find-usages"]["subcommand"] is None, (
        "find-usages recibe `symbol` y nada mas; que cuelgue de un subcomando "
        "significa que una variante de tupla se le robo los campos"
    )
    assert specs["CliCommand"]["find-usages"]["required"] == 1, (
        f"find-usages pide 1 symbol; el parser leyo {specs['CliCommand']['find-usages']['required']}"
    )
    assert specs["CliCommand"]["evidence"]["subcommand"] == "EvidenceCommand", (
        "evidence cuelga de EvidenceCommand y el parser deberia verlo"
    )


def test_default_values_make_a_positional_optional() -> None:
    """La aridad depende de leer `default_value`, no de contar campos.

    Sin esto, `cognicode doctor` (format y cwd con default) se contaria como dos
    positionales requeridos y el contrato rechazaria una invocacion correcta.
    """
    specs = _specs()
    assert specs["CliCommand"]["doctor"]["required"] == 0, (
        "Doctor declara format y cwd con default_value, luego 0 requeridos"
    )
    assert specs["IndexCommand"]["build"]["required"] == 0, (
        "Index Build declara path y strategy con default, luego 0 requeridos"
    )


def test_a_valid_invocation_produces_no_problems() -> None:
    assert check_invocation(["doctor"], _specs()) == []
    assert check_invocation(["index", "build"], _specs()) == []
    assert check_invocation(["graph", "entry-points"], _specs()) == []
    assert check_invocation(["graph", "trace-path", "caller", "callee"], _specs()) == []


def test_a_position_is_accepted_where_a_position_is_required() -> None:
    """La forma correcta tiene que pasar, o el contrato no es corregible."""
    specs = _specs()
    assert check_invocation(["navigate", "references", "src/main.rs:42:10"], specs) == []
    assert check_invocation(["navigate", "references", "<file:line:column>"], specs) == []


def test_a_symbol_where_a_position_is_required_is_rejected() -> None:
    """La regresion: misma aridad, forma equivocada.

    `references <symbol>` y `references <file:line:column>`doo ambos un
    positional. Solo la forma distingue uno del otro, asi que este es el caso
    para el que existe el chequeo de posicion.
    """
    problems = check_invocation(["navigate", "references", "<symbol>"], _specs())
    assert problems, "un simbolo donde la firma pide file:line:column tiene que fallar"
    assert "posicion" in problems[0], f"el motivo deberia nombrar el tipo posicion: {problems}"


def test_a_missing_positional_is_rejected() -> None:
    """`index symbol-code MySymbol` da 1 de 3 positionales y sale con codigo 2."""
    problems = check_invocation(["index", "symbol-code", "MySymbol"], _specs())
    assert problems, "faltan 2 positionals y tiene que fallar"
    assert "3 positional" in problems[0], f"el motivo deberia decir cuantos faltan: {problems}"


def test_an_unknown_subcommand_is_rejected() -> None:
    problems = check_invocation(["index", "no-existe"], _specs())
    assert problems, "un subcomando inexistente tiene que fallar"
    assert "no existe" in problems[0], problems

    problems = check_invocation(["nope"], _specs())
    assert problems, "un subcomando raiz inexistente tiene que fallar"


def test_a_value_flag_does_not_look_like_a_positional() -> None:
    """`graph hierarchy --depth 2 MySymbol` tiene un positional, no tres.

    `Hierarchy` declara `depth` con `#[arg(short = 'd', long, default_value =
    "3")]`, o sea un flag que consume valor. Si `--depth 2` se contara como dos
    argumentos, un comando al que le falta `symbol` pasaria el chequeo de aridad.
    """
    specs = _specs()
    assert check_invocation(["graph", "hierarchy", "--depth", "2", "MySymbol"], specs) == []
    problems = check_invocation(["graph", "hierarchy", "--depth", "2"], specs)
    assert problems, "sin symbol el comando falla, aunque --depth 2 parezca un argumento"
    assert "1 positional" in problems[0], problems


def test_a_root_flag_is_not_a_subcommand() -> None:
    """`cognicode --version` es el primer paso de la skill publicada `cognicode`.

    Un flag global no es un subcomando, asi que buscarlo en el enum lo
    reportaria como inexistente y el contrato seria un falso positivo en el
    primer comando que la skill ensena.
    """
    source = COMMANDS_RS.read_text(encoding="utf-8")
    flags = cli_root_flags(source)
    assert "--version" in flags, "el atributo `version` esta en `#[command(...)]`"
    assert check_invocation(["--version"], _specs(), flags) == []
    assert check_invocation(["--help"], _specs(), flags) == []
    assert check_invocation(["--nope"], _specs(), flags), "un flag inexistente si debe fallar"


def test_prose_is_not_an_invocation() -> None:
    """Una mencion en prosa no es un comando que nadie ejecuta.

    Sin esta regla el contrato seria un falso positivo permanente y la gente
    acabaria saltandoselo, que es peor que no tenerlo.
    """
    text = (
        "The three surfaces share capabilities (e.g. `cognicode graph impact` and\n"
        "MCP `analyze_impact` answer the same question).\n"
    )
    assert extract_invocations(text) == [], "una mencion en backticks es prosa"

    fenced = "```bash\ncognicode doctor\n```\n"
    assert extract_invocations(fenced) == [(2, ["doctor"])], "dentro de una cerca si es comando"


def test_the_published_set_is_not_empty_and_is_covered() -> None:
    """Fail-closed: si `SKILL_BUNDLES` dejara de publicarse, esto tiene que fallar.

    Un guard que vigila un conjunto vacio no vigila nada, y seria la razon por la
    que un defecto volveria a entrar sin que nadie lo note.
    """
    published = published_skill_ids()
    assert published, (
        "SKILL_BUNDLES no declara ninguna skill publicada: o se rompio la "
        "tabla, o el contrato esta mirando la fuente equivocada"
    )
    files = skill_files()
    assert files, f"no se encontro ningun skills/*/SKILL.md bajo {SKILLS_DIR}"
    covered = {path.parent.name for path in files}
    missing = published - covered
    assert not missing, (
        f"skills publicadas sin SKILL.md, y por tanto sin cubrir: {sorted(missing)}"
    )

def test_the_real_skills_teach_no_broken_invocation() -> None:
    """El contrato de verdad, sobre el contenido que se empaqueta."""
    problems = audit_invocations(_specs(), skill_files())
    assert not problems, "invocaciones rotas en skills:\n  " + "\n  ".join(problems)


def test_the_audit_actually_examines_invocations() -> None:
    """Fail-closed sobre el propio guard.

    Medido: borrando el bloque ```bash de una skill publicada, el contrato se
    quedaba en verde. Un guard que se puede neutralizar borrando el codigo que
    vigila no es un guard, es una decoracion — y la forma de neutralizarlo no
    requiere ni malicious ni un descuido raro: basta con reescribir el ejemplo.

    El suelo no pretende saber cuantas invocaciones deberia haber, solo que no
    pueden ser cero.
    """
    total = 0
    for path in skill_files():
        total += len(extract_invocations(path.read_text(encoding="utf-8")))
    assert total >= 8, (
        f"solo se auditaron {total} invocaciones de CLI en las skills; si el "
        "contenido se vacio, el contrato pasaria sin comprobar nada"
    )



# ---------------------------------------------------------------------------
# Tests de la parte B (afirmaciones de superficie)
# ---------------------------------------------------------------------------
# --- Tests ------------------------------------------------------------------


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


def test_the_real_skills_claim_no_false_surface() -> None:
    """El contrato de verdad, sobre el contenido que se empaqueta."""
    problems = audit_claims(skill_files())
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

    El mismo fallo que trailing en `test_provenance_contracts.py`: un `main()` que
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
        "PASS - cada invocacion de CLI en las skills existe y aporta los "
        "argumentos que su firma real exige, y las skills afirman la superficie "
        "que el producto publica."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
