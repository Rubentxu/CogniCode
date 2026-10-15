"""Pin del inventario de casos por suite (TH-S02).

Una suite que ejecuta **cero** casos por una feature mal puesta no es un verde: es
un hueco con forma de verde. El recibo ya lo detecta cuando no hay ningún caso
(`executed_cases == 0`), pero no distingue un filtro que no casa de una suite que
ejecutaba 40 casos y ahora hace 3 porque `#[cfg]` apagó el resto.

Para eso existe este pin: un fichero versionado (`test-harness/inventory.json`)
con el mínimo de casos esperado por suite, obtenido de ejecuciones reales y
revisable en el PR. El plan lo lleva a la huella de entradas (dos expectativas
distintas son dos entradas distintas) y el gate exige `executed_cases >=
expected_min_cases` para dar por bueno un PASS.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

INVENTORY_PATH = Path("test-harness/inventory.json")
SCHEMA = "th.inventory/v1"


@dataclass(frozen=True)
class Inventory:
    entries: dict[str, int]
    path: Path | None = None
    problem: str | None = None

    def expected_min_for(self, suite_id: str) -> int:
        return int(self.entries.get(suite_id, 0))

    def to_json(self) -> dict:
        return {
            "schema_version": SCHEMA,
            "entries": {key: self.entries[key] for key in sorted(self.entries)},
        }


def load_inventory(repo_root: Path) -> Inventory:
    """Lee el pin. Ausente no es un error; ilegible sí: no se adivina un mínimo."""
    path = repo_root / INVENTORY_PATH
    if not path.exists():
        return Inventory(entries={}, path=None)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        return Inventory(entries={}, path=path, problem=f"{INVENTORY_PATH} ilegible: {exc}")
    if payload.get("schema_version") != SCHEMA:
        return Inventory(
            entries={},
            path=path,
            problem=(
                f"{INVENTORY_PATH} declara schema_version {payload.get('schema_version')!r} "
                f"y se esperaba {SCHEMA!r}"
            ),
        )
    entries = {}
    for key, value in (payload.get("entries") or {}).items():
        try:
            entries[str(key)] = int(value)
        except (TypeError, ValueError):
            return Inventory(
                entries={}, path=path, problem=f"{INVENTORY_PATH}: el mínimo de {key} no es un entero"
            )
    return Inventory(entries=entries, path=path)


def write_inventory(repo_root: Path, inventory: Inventory) -> Path:
    path = repo_root / INVENTORY_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(inventory.to_json(), indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return path


def inventory_from_receipts(receipts: list) -> Inventory:
    """Deriva el pin de recibos reales: solo PASSED con casos ejecutados.

    El mínimo que se escribe es el número de casos **distintos que pasaron** en la
    última ejecución verde: es un suelo observable, no una aspiración. Un suite que
    nunca pasó no entra en el pin, porque no hay nada que pinear.
    """
    entries: dict[str, int] = {}
    for receipt in receipts:
        if receipt.status != "PASSED" or not receipt.is_well_formed_pass():
            continue
        executed = int(receipt.executed_cases)
        if executed <= 0:
            continue
        current = entries.get(receipt.suite_id, 0)
        if executed > current:
            entries[receipt.suite_id] = executed
    return Inventory(entries=entries)


def check_receipt_against_inventory(receipt, expected_min: int) -> str | None:
    """Motivo por el que un PASS no cumple el pin, o None si lo cumple."""
    if expected_min <= 0:
        return None
    if receipt.status != "PASSED":
        return None
    if receipt.runner.name != "cargo-test":
        return None
    if int(receipt.executed_cases) >= expected_min:
        return None
    return (
        f"la suite ejecutó {receipt.executed_cases} caso(s) y el inventario pinado "
        f"espera al menos {expected_min}: una feature o un filtro apagaron pruebas "
        "que antes corrían"
    )
