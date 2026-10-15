"""Suelo de obligaciones críticas: una etapa del gate no puede desaparecer en silencio.

El fallo que esto evita ya ocurrió en este repositorio: una etapa contractual dejó
de invocarse y nadie lo notó porque ningún contrato decía «esta obligación tiene
que existir». La selección por impacto no puede cubrir ese caso —si la obligación
no está en el catálogo, no hay nada que seleccionar—, así que existe este suelo:
una lista corta y versionada de obligaciones que la lane de merge **debe**
contener.

No es un inventario paralelo: es un mínimo auditable. Las obligaciones siguen
leyéndose de las pipelines; este fichero solo dice cuáles no pueden faltar, y el
plan se vuelve indeterminado cuando alguna falta en vez de dar el merge por bueno.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

FLOOR_PATH = Path("test-harness/obligations.json")
SCHEMA = "th.obligations/v1"


@dataclass(frozen=True)
class Floor:
    entries: dict[str, tuple[str, ...]]  # lane -> suite_ids que no pueden faltar
    path: Path | None = None
    problem: str | None = None

    def required_for(self, lane: str) -> tuple[str, ...]:
        return tuple(self.entries.get(lane, ()))

    def to_json(self) -> dict:
        return {
            "schema_version": SCHEMA,
            "entries": {lane: list(self.entries[lane]) for lane in sorted(self.entries)},
        }


def load_floor(repo_root: Path) -> Floor:
    """Lee el suelo. Ausente no es error; ilegible sí: no se adivina qué es crítico."""
    path = repo_root / FLOOR_PATH
    if not path.exists():
        return Floor(entries={}, path=None)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        return Floor(entries={}, path=path, problem=f"{FLOOR_PATH} ilegible: {exc}")
    if payload.get("schema_version") != SCHEMA:
        return Floor(
            entries={},
            path=path,
            problem=(
                f"{FLOOR_PATH} declara schema_version {payload.get('schema_version')!r} "
                f"y se esperaba {SCHEMA!r}"
            ),
        )
    entries: dict[str, tuple[str, ...]] = {}
    for lane, suites in (payload.get("entries") or {}).items():
        if not isinstance(suites, list) or not all(isinstance(item, str) for item in suites):
            return Floor(entries={}, path=path, problem=f"{FLOOR_PATH}: la lane {lane} no es una lista de suites")
        entries[str(lane)] = tuple(sorted(suites))
    return Floor(entries=entries, path=path)


def missing_from_catalog(floor: Floor, lane: str, suite_ids: set[str]) -> tuple[str, ...]:
    """Obligaciones del suelo que el catálogo de la lane no contiene."""
    return tuple(suite for suite in floor.required_for(lane) if suite not in suite_ids)


def unknown_obligations(floor: Floor, catalog_suite_ids: set[str]) -> tuple[str, ...]:
    """Obligaciones del suelo que no existen en **ninguna** lane conocida.

    Un suelo que nombra suites que ya no son reales deja de proteger nada: es
    deuda disfrazada de control, y por eso el contrato del gate lo comprueba.
    """
    known: set[str] = set()
    for suites in floor.entries.values():
        known.update(suites)
    return tuple(sorted(suite for suite in known if suite not in catalog_suite_ids))
