"""Catálogo de obligaciones leído de las pipelines de PipelineK (TH-S01).

La fuente de verdad de *qué* hay que ejecutar es PipelineK, no una lista paralela
mantenida a mano en el harness. Este adaptador lee los `*.pipeline.kts`, extrae las
invocaciones de `cargo test` y de scripts, y las convierte en obligaciones con id
estable. Si mañana alguien añade una suite a la pipeline, aparece aquí sin tocar
el harness; si la quita, deja de planificarse, que es lo que se quiere.

Sólo los metadatos que **no** son derivables se declaran a mano (lane por fichero).
Todo lo demás se deriva del texto.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass, replace
from pathlib import Path

# Qué pipelines alimentan qué lane. Es la única tabla escrita a mano, y es
# deliberada: expresa intención de gate, no inventario de tests.
LANE_PIPELINES: dict[str, tuple[str, ...]] = {
    "change": ("product-fast.pipeline.kts", "merge-gate.pipeline.kts"),
    "merge": ("merge-gate.pipeline.kts",),
    "integration": ("integration.pipeline.kts", "merge-gate.pipeline.kts"),
    "certification": ("certification.pipeline.kts",),
    "release-candidate": ("release-candidate.pipeline.kts",),
    "release": ("release.pipeline.kts",),
}

# Obligaciones que existen en las pipelines de una lane pero que esa lane no
# debe arrastrar. `cargo deny` consulta una base de avisos externa: es una
# obligación de seguridad del merge, no una prueba de feedback local, y
# incluirla en `change` convertiría cada plan en indeterminado sin motivo.
LANE_EXCLUDES: dict[str, tuple[str, ...]] = {
    "change": ("cargo:deny",),
}

_TERMINATORS = ('"', "\n", "|", ";", ")", "&", ">")
_CARGO_TEST_RE = re.compile(r"\bcargo\s+test\b")
_CARGO_CHECK_RE = re.compile(r"\bcargo\s+(fmt|clippy|deny|build)\b")
_SCRIPT_RE = re.compile(r"\bbash\s+(scripts/[\w./-]+\.sh)\b")

# Subcomandos de cargo que son obligaciones del gate sin ser suites de test.
CHECK_SUBCOMMANDS = ("fmt", "clippy", "deny", "build")


# Una expansión sin resolver. El detector es deliberadamente más ancho que la
# sustitución: `$1`, `$(cmd)` o `$$` no se pueden resolver aquí, así que cuentan
# como variables pendientes y la suite no se ejecuta. Lo que no puede pasar es
# que un token llegue a cargo con un `$` dentro y se interprete como un target
# literal, que es exactamente el fallo medido el 2026-10-16:
# `cargo build --release --target $target` llegaba a cargo con `$target` sin
# expandir y devolvía «could not find specification for target "$target"`.
_EXPANSION_RE = re.compile(r"\$[A-Za-z0-9_{(@*#?!$-]")
_NAMED_VAR_RE = re.compile(r"\$(?:\{)?([A-Za-z_][A-Za-z0-9_]*)(?:\})?")
_KOTLIN_ESCAPED_DOLLAR = re.compile(r"\$\{'\$'\}")
_DECLARED_TARGETS_RE = re.compile(
    r"val\s+targets\s*=\s*listOf\(([^)]*)\)", re.S
)


def unescape_kotlin(text: str) -> str:
    """`${'$'}VAR` en el fuente de la pipeline es un `$VAR` para el shell.

    El pipeline escribe `${'$'}TARGET_DIR` cuando quiere que la expansión la haga
    el shell y no Kotlin. Si no se revierte, el catálogo ve una variable con un
    nombre que no existe (`{`$`}TARGET_DIR`) y el diagnóstico sería una mentira.
    """
    return _KOTLIN_ESCAPED_DOLLAR.sub("$", text)


def variables_in(argv: tuple[str, ...]) -> tuple[str, ...]:
    """Nombres de variable que aparecen en el argv, únicos y ordenados."""
    names: set[str] = set()
    for token in argv:
        for match in _NAMED_VAR_RE.finditer(token):
            names.add(match.group(1))
        if _EXPANSION_RE.search(_NAMED_VAR_RE.sub("", token)):
            # Queda una expansión que no tiene forma de nombre (`$1`, `$(...)`).
            names.add(token[token.index("$") :])
    return tuple(sorted(names))


def expand_variables(
    argv: tuple[str, ...], variables: dict[str, str]
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Sustituye `$nombre`/`${nombre}` y devuelve lo que quedó sin resolver.

    Lo que no se puede resolver se devuelve en vez de dejarse pasar: un argv con
    una expansión viva no es un comando, es una promesa.
    """
    resolved: list[str] = []
    for token in argv:
        def _replace(match: re.Match[str]) -> str:
            name = match.group(1)
            return variables.get(name, match.group(0))

        resolved.append(_NAMED_VAR_RE.sub(_replace, token))
    expanded = tuple(resolved)
    return expanded, variables_in(expanded)


def read_declared_targets(repo_root: Path) -> tuple[str, ...]:
    """Targets que las pipelines declaran construir (`val targets = listOf(...)`).

    Se leen para no inventar un target: si la máquina no es uno de los declarados,
    el harness no sustituye `$target` y la suite queda sin ejecutar en vez de
    ejecutarse contra un target que esa lane no construye.
    """
    found: set[str] = set()
    for names in LANE_PIPELINES.values():
        for name in names:
            path = Path(repo_root) / name
            if not path.exists():
                continue
            text = unescape_kotlin(path.read_text(encoding="utf-8"))
            for match in _DECLARED_TARGETS_RE.finditer(text):
                for item in match.group(1).split(","):
                    value = item.strip().strip('"').strip("'")
                    if value:
                        found.add(value)
    return tuple(sorted(found))


@dataclass(frozen=True)
class CatalogEntry:
    """Una obligación tal y como aparece hoy en las pipelines."""

    suite_id: str
    package: str
    target: str
    argv: tuple[str, ...]
    lanes: tuple[str, ...]
    origin: str  # fichero de pipeline donde aparece
    scope: str = "package"  # package | workspace | script
    kind: str = "test"  # test | check | script
    advisory: bool = False  # el gate la deja fallar a propósito
    serial: bool = False  # muta el entorno: corre sola, con barrera
    locks: tuple[str, ...] = ()  # recursos compartidos que necesita
    # El mismo suite_id no siempre se ejecuta igual en cada lane: merge-gate hace
    # `cargo build -p cognicode-cli --bins` y release-candidate hace
    # `cargo build --release --target $target -p cognicode-cli --bin cogh`. El
    # catálogo antiguo se quedaba con uno de los dos y lo presentaba como «la»
    # obligación, así que la lane change ejecutaba el comando de release-candidate.
    argv_by_lane: tuple[tuple[str, tuple[str, ...]], ...] = ()

    def sort_key(self) -> tuple[str, str]:
        return (self.package, self.target)

    def argv_for(self, lane: str) -> tuple[str, ...]:
        """El comando que **esta** lane declara, no el de otra."""
        for name, argv in self.argv_by_lane:
            if name == lane:
                return argv
        return self.argv


def _join_continuations(text: str) -> str:
    return re.sub(r"\\\n\s*", " ", text)


def _tokenize_after(text: str, start: int) -> tuple[list[str], int]:
    """Tokens del comando que empieza en `start`, y dónde terminó."""
    tokens: list[str] = []
    current: list[str] = []
    index = start
    while index < len(text):
        char = text[index]
        if char in _TERMINATORS:
            break
        if char.isspace():
            if current:
                tokens.append("".join(current))
                current = []
            index += 1
            continue
        current.append(char)
        index += 1
    if current:
        tokens.append("".join(current))
    return tokens, index


def _parse_cargo_test(tokens: list[str]) -> tuple[str, str, tuple[str, ...]] | None:
    """(suite_id, etiqueta_de_target, features) de una invocación de cargo test.

    El `argv` no se reconstruye: se conserva la forma exacta observada en la
    pipeline, porque el orden de `--lib --bins` es normativo y reconstruirlo sería
    inventar una segunda fuente de verdad.
    """
    package = ""
    targets: list[str] = []
    filters: list[str] = []
    features: list[str] = []
    index = 2  # saltar "cargo" y "test"
    while index < len(tokens):
        token = tokens[index]
        if token in ("-p", "--package"):
            index += 1
            if index < len(tokens):
                package = tokens[index]
        elif token == "--test":
            index += 1
            if index < len(tokens):
                targets.append(tokens[index])
        elif token == "--bench":
            index += 1
            if index < len(tokens):
                targets.append("bench:" + tokens[index])
        elif token in ("--lib", "--bins", "--doc", "--all-targets", "--examples"):
            targets.append(token.lstrip("-"))
        elif token == "--features":
            index += 1
            if index < len(tokens):
                features.append(tokens[index])
        elif token.startswith("--features="):
            features.append(token.split("=", 1)[1])
        elif token.startswith("-"):
            pass
        else:
            filters.append(token)
        index += 1
    if not package or not targets:
        return None
    ordered = sorted(set(targets))
    filter_label = ",".join(filters) if filters else "-"
    if len(ordered) > 6:
        # Una invocación con 41 `--test` no cabe como etiqueta legible; el id dice
        # cuántos targets y qué paquete, y el argv exacto vive en la entrada.
        short = hashlib.sha256("\0".join(ordered).encode("utf-8")).hexdigest()[:8]
        suite_id = f"{package}:multi{len(ordered)}#{filter_label}-{short}"
    else:
        suite_id = f"{package}:{'+'.join(ordered)}#{filter_label}"
    return suite_id, "+".join(ordered), tuple(features)


def _parse_cargo_check(tokens: list[str]) -> tuple[str, str, str] | None:
    """(suite_id, package, scope) de `cargo fmt|clippy|deny|build ...`.

    Son obligaciones del gate que no son suites de test, pero cuyos fallos
    bloquean igual. No cuentan casos: su evidencia es el exit code y el log.
    """
    if len(tokens) < 2:
        return None
    subcommand = tokens[1]
    if subcommand not in CHECK_SUBCOMMANDS:
        return None
    package = ""
    index = 2
    while index < len(tokens):
        if tokens[index] in ("-p", "--package") and index + 1 < len(tokens):
            package = tokens[index + 1]
            index += 2
            continue
        index += 1
    scope = "package" if package else "workspace"
    label = f"cargo:{subcommand}"
    suite_id = f"{package or 'workspace'}:{label}#-"
    return suite_id, package, scope


def _is_advisory(text: str, end: int) -> bool:
    """¿La etapa termina con `|| echo 'ADVISORY: ...'`?

    PipelineK no tiene `continue-on-error`: una etapa que puede fallar sin
    romper la lane lo dice con ese sufijo. El harness copia esa semántica en vez
    de inventar una propia, y por eso estas obligaciones no bloquean.
    """
    window = text[end : end + 240]
    return "ADVISORY" in window and "||" in window.split("ADVISORY")[0]


def _in_comment(text: str, index: int) -> bool:
    """¿El punto `index` cae dentro de un comentario de Kotlin?

    MEDIDO 2026-10-16. `release.pipeline.kts` explica en un comentario que el
    candidato construye `cargo build --release --bin cognicode-release`, y el
    lector lo tomó por una obligación: la lane `release` quedó con un argv que
    incluía la prosa del comentario (`... appearing in all three of its`). Una
    obligación inventada es peor que una perdida: se planifica y, si se ejecuta,
    corre un comando que nadie declaró.
    """
    line_start = text.rfind("\n", 0, index) + 1
    prefix = text[line_start:index].lstrip()
    return prefix.startswith("//") or prefix.startswith("*") or prefix.startswith("/*")


def _observed_argv(text: str, start: int) -> tuple[tuple[str, ...], int]:
    tokens, end = _tokenize_after(text, start)
    return tuple(tokens), end


def read_catalog(repo_root: Path) -> tuple[CatalogEntry, ...]:
    """Catálogo completo: todas las obligaciones de todas las pipelines conocidas.

    Un mismo suite_id puede declararse en varias pipelines con comandos distintos,
    y cada lane debe ejecutar **el suyo**. El argv canónico (y el `origin`) es el
    de la primera lane que lo declara, en el orden de `LANE_PIPELINES`: así el
    valor por defecto es el del lane más cercano al desarrollador y el de
    release-candidate no se cuela en `change`.
    """
    observed: dict[str, dict[str, object]] = {}
    by_lane: dict[str, dict[str, tuple[str, ...]]] = {}
    root = Path(repo_root)

    def register(suite_id: str, lane: str, argv: tuple[str, ...], name: str, **fields: object) -> None:
        entry = observed.get(suite_id)
        lanes = tuple(sorted({lane, *((entry or {}).get("lanes") or ())}))
        if entry is None:
            observed[suite_id] = {"lanes": lanes, "argv": argv, "origin": name, **fields}
        else:
            entry["lanes"] = lanes
            # `advisory` es pegajoso: si una pipeline la deja fallar a propósito,
            # el plan no puede convertirla en obligación dura por mirar otra.
            entry["advisory"] = bool(entry.get("advisory")) or bool(fields.get("advisory"))
            if fields.get("serial"):
                entry["serial"] = True
        by_lane.setdefault(suite_id, {})[lane] = argv

    for lane, files in LANE_PIPELINES.items():
        for name in files:
            path = root / name
            if not path.exists():
                continue
            text = unescape_kotlin(_join_continuations(path.read_text(encoding="utf-8")))
            for match in _CARGO_TEST_RE.finditer(text):
                if _in_comment(text, match.start()):
                    continue
                tokens, end = _observed_argv(text, match.start())
                if len(tokens) < 3:
                    continue
                parsed = _parse_cargo_test(list(tokens))
                if parsed is None:
                    continue
                suite_id, target, _ = parsed
                register(
                    suite_id,
                    lane,
                    tuple(tokens),
                    name,
                    target=target,
                    package=suite_id.split(":", 1)[0],
                    scope="package",
                    kind="test",
                    advisory=_is_advisory(text, end),
                )
            for match in _CARGO_CHECK_RE.finditer(text):
                if _in_comment(text, match.start()):
                    continue
                tokens, end = _observed_argv(text, match.start())
                parsed_check = _parse_cargo_check(list(tokens))
                if parsed_check is None:
                    continue
                suite_id, package, scope = parsed_check
                register(
                    suite_id,
                    lane,
                    tuple(tokens),
                    name,
                    target=f"cargo:{tokens[1]}",
                    package=package,
                    scope=scope,
                    kind="check",
                    advisory=_is_advisory(text, end),
                )
            for match in _SCRIPT_RE.finditer(text):
                if _in_comment(text, match.start()):
                    continue
                script = match.group(1)
                suite_id = f"workspace:script:{Path(script).name.removesuffix('.sh')}"
                register(
                    suite_id,
                    lane,
                    ("bash", script),
                    name,
                    target=f"script:{script}",
                    package="",
                    scope="script",
                    kind="script",
                    advisory=_is_advisory(text, match.end()),
                )

    entries: list[CatalogEntry] = []
    for suite_id in sorted(observed):
        fields = dict(observed[suite_id])
        lanes = tuple(sorted(fields.pop("lanes") or ()))
        argv = tuple(fields.pop("argv"))
        origin = str(fields.pop("origin"))
        per_lane = tuple(sorted(by_lane.get(suite_id, {}).items()))
        entries.append(
            CatalogEntry(
                suite_id=suite_id,
                argv=argv,
                lanes=lanes,
                origin=origin,
                argv_by_lane=per_lane,
                **fields,
            )
        )
    return tuple(entries)


SUITE_METADATA = Path("test-harness/config/suite-metadata.json")
SUITE_METADATA_SCHEMA = "th.suite-metadata/v1"


def _rule_matches(rule: str, suite_id: str) -> bool:
    if rule.endswith("*"):
        return suite_id.startswith(rule[:-1])
    return rule == suite_id


def apply_suite_metadata(
    entries: tuple[CatalogEntry, ...], repo_root: Path
) -> tuple[CatalogEntry, ...]:
    """Aplica los metadatos declarados que no son derivables (TH-S01).

    `serial` y `locks` no se pueden deducir del comando: un script que toca el
    entorno y otro que solo lee se ven igual en la línea de la pipeline. Se
    declaran en `test-harness/config/suite-metadata.json` y se aplican aquí.

    Un fichero ilegible **no** se ignora en silencio: devuelve las entradas sin
    metadatos y un aviso queda registrado en el log del harness. Silenciarlo
    paralelizaría suites que el repositorio declaró seriales.
    """
    path = repo_root / SUITE_METADATA
    if not path.exists():
        return entries
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        raise ValueError(f"{SUITE_METADATA} ilegible: {exc}") from exc
    if payload.get("schema_version") != SUITE_METADATA_SCHEMA:
        raise ValueError(
            f"{SUITE_METADATA} declara schema_version {payload.get('schema_version')!r} "
            f"y se esperaba {SUITE_METADATA_SCHEMA!r}"
        )
    rules = payload.get("rules") or []
    updated: list[CatalogEntry] = []
    for entry in entries:
        serial = entry.serial
        locks = set(entry.locks)
        for rule in rules:
            match = rule.get("match", "")
            if not match or not _rule_matches(match, entry.suite_id):
                continue
            if rule.get("serial"):
                serial = True
            locks.update(rule.get("locks") or ())
        updated.append(replace(entry, serial=serial, locks=tuple(sorted(locks))))
    return tuple(updated)


def catalog_for_lane(catalog: tuple[CatalogEntry, ...], lane: str) -> tuple[CatalogEntry, ...]:
    """Obligaciones de una lane, sin las que esa lane no debe arrastrar."""
    excluded_targets = LANE_EXCLUDES.get(lane, ())
    return tuple(
        entry
        for entry in catalog
        if lane in entry.lanes and entry.target not in excluded_targets
    )
