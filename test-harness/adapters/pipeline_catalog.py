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

    def sort_key(self) -> tuple[str, str]:
        return (self.package, self.target)


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


def _observed_argv(text: str, start: int) -> tuple[tuple[str, ...], int]:
    tokens, end = _tokenize_after(text, start)
    return tuple(tokens), end


def read_catalog(repo_root: Path) -> tuple[CatalogEntry, ...]:
    """Catálogo completo: todas las obligaciones de todas las pipelines conocidas."""
    entries: dict[str, CatalogEntry] = {}
    root = Path(repo_root)

    for lane, files in LANE_PIPELINES.items():
        for name in files:
            path = root / name
            if not path.exists():
                continue
            text = _join_continuations(path.read_text(encoding="utf-8"))
            for match in _CARGO_TEST_RE.finditer(text):
                tokens, end = _observed_argv(text, match.start())
                if len(tokens) < 3:
                    continue
                parsed = _parse_cargo_test(list(tokens))
                if parsed is None:
                    continue
                suite_id, target, _ = parsed
                argv = tuple(tokens)
                existing = entries.get(suite_id)
                lanes = tuple(sorted({*(existing.lanes if existing else ()), lane}))
                entries[suite_id] = CatalogEntry(
                    suite_id=suite_id,
                    package=suite_id.split(":", 1)[0],
                    target=target,
                    argv=argv,
                    lanes=lanes,
                    origin=existing.origin if existing else name,
                    scope="package",
                    kind="test",
                    advisory=_is_advisory(text, end) or bool(existing and existing.advisory),
                )
            for match in _CARGO_CHECK_RE.finditer(text):
                tokens, end = _observed_argv(text, match.start())
                parsed_check = _parse_cargo_check(list(tokens))
                if parsed_check is None:
                    continue
                suite_id, package, scope = parsed_check
                existing = entries.get(suite_id)
                lanes = tuple(sorted({*(existing.lanes if existing else ()), lane}))
                entries[suite_id] = CatalogEntry(
                    suite_id=suite_id,
                    package=package,
                    target=f"cargo:{tokens[1]}",
                    argv=tuple(tokens),
                    lanes=lanes,
                    origin=existing.origin if existing else name,
                    scope=scope,
                    kind="check",
                    advisory=_is_advisory(text, end) or bool(existing and existing.advisory),
                )
            for match in _SCRIPT_RE.finditer(text):
                script = match.group(1)
                suite_id = f"workspace:script:{Path(script).name.removesuffix('.sh')}"
                existing = entries.get(suite_id)
                end = match.end()
                lanes = tuple(sorted({*(existing.lanes if existing else ()), lane}))
                entries[suite_id] = CatalogEntry(
                    suite_id=suite_id,
                    package="",
                    target=f"script:{script}",
                    argv=("bash", script),
                    lanes=lanes,
                    origin=existing.origin if existing else name,
                    scope="script",
                    kind="script",
                    advisory=_is_advisory(text, end) or bool(existing and existing.advisory),
                )

    return tuple(entries[key] for key in sorted(entries))


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
