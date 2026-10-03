#!/usr/bin/env python3
"""Las skills publicadas no pueden enseñar un comando que no existe o que falla.

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
"""

from __future__ import annotations

import re
import shlex
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
COMMANDS_RS = REPO_ROOT / "crates" / "cognicode-core" / "src" / "interface" / "cli" / "commands.rs"
RELEASE_CONTRACT_RS = (
    REPO_ROOT / "crates" / "cognicode-cli" / "src" / "cmd" / "release_contract.rs"
)
SKILLS_DIR = REPO_ROOT / "skills"

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
    nadie ejecuta, y hacerlas fallar seria un falso positivo que ense馃a a la
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


# ---------------------------------------------------------------------------
# Descubrimiento
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


def audit(specs: dict[str, dict[str, dict]], files: list[Path]) -> list[str]:
    flags = cli_root_flags(COMMANDS_RS.read_text(encoding="utf-8"))
    problems: list[str] = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        for line, tokens in extract_invocations(text):
            for problem in check_invocation(tokens, specs, flags):
                problems.append(f"{path.relative_to(REPO_ROOT)}:{line}: {problem}")
    return problems


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


def test_the_real_skills_are_clean() -> None:
    """El contrato de verdad, sobre el contenido que se empaqueta."""
    problems = audit(_specs(), skill_files())
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


def main() -> int:
    tests = [value for name, value in sorted(globals().items()) if name.startswith("test_")]
    failures: list[str] = []
    for test in tests:
        try:
            test()
        except Exception as error:  # noqa: BLE001 - un contrato reporta, no propaga
            failures.append(f"{test.__name__}: {error}")
    if failures:
        print(f"FAIL - {len(failures)} fallo(s):")
        for failure in failures:
            print(failure)
        return 1
    print(
        "PASS - cada invocacion de CLI en las skills existe y aporta los "
        "argumentos que su firma real exige."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
