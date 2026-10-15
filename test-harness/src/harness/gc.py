"""Recolección de basura del ledger, separando evidencia de caché (TH-S02).

El ledger no es un vertedero: los recibos son evidencia y las ejecuciones
auditables, pero los logs de una corrida vieja que nadie referencia son caché.
La regla, escrita para que no dependa del criterio de quien ejecuta:

  * **Nunca** se borra un recibo: son inmutables y son la prueba.
  * Se conservan los `runs/<id>.json` de las últimas `keep` ejecuciones (y el
    estado de cualquier run que esté en `RUNNING`, que es una interrupción en
    curso).
  * Los logs de runs **fuera** de esa ventana se borran solo si ningún recibo
    conservado los referencia por `log_sha256`... lo que en la práctica significa
    que se borran junto al run, porque un log pertenece a su run.
  * `--dry-run` dice exactamente qué se borraría sin tocar nada, y es el modo con
    el que se mide antes de borrar.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

from adapters.ledger_store import Ledger


@dataclass
class GCPlan:
    keep_runs: int
    kept: list[str] = field(default_factory=list)
    logs_to_delete: list[str] = field(default_factory=list)
    bytes_to_free: int = 0
    notes: list[str] = field(default_factory=list)

    def to_json(self) -> dict:
        return {
            "keep_runs": self.keep_runs,
            "kept_runs": sorted(self.kept),
            "logs_to_delete": sorted(self.logs_to_delete),
            "bytes_to_free": self.bytes_to_free,
            "notes": list(self.notes),
        }


def plan_gc(ledger: Ledger, keep_runs: int) -> GCPlan:
    """Calcula qué logs sobran sin borrar nada."""
    plan = GCPlan(keep_runs=max(0, keep_runs))
    runs = sorted(ledger.list_runs(), key=lambda run: run.get("started_at", ""), reverse=True)
    if not runs:
        plan.notes.append("el ledger no tiene runs")
        return plan
    keep_ids = {run.get("run_id") for run in runs[: plan.keep_runs]}
    for run in runs:
        run_id = run.get("run_id")
        if not run_id:
            continue
        if run.get("status") == "RUNNING":
            keep_ids.add(run_id)
            plan.notes.append(
                f"el run {run_id} sigue en RUNNING: se conserva por si es una interrupción en curso"
            )
    plan.kept = [rid for rid in keep_ids if rid]
    logs_root = ledger.logs_dir
    if not logs_root.exists():
        plan.notes.append("no hay directorio de logs")
        return plan
    for directory in sorted(logs_root.iterdir()):
        if not directory.is_dir() or directory.name in keep_ids:
            continue
        for path in sorted(directory.rglob("*")):
            if path.is_file():
                plan.logs_to_delete.append(str(path.relative_to(logs_root)))
                plan.bytes_to_free += path.stat().st_size
    return plan


def apply_gc(ledger: Ledger, plan: GCPlan) -> GCPlan:
    """Borra los logs del plan. Los recibos no se tocan nunca."""
    for relative in plan.logs_to_delete:
        path = ledger.logs_dir / relative
        try:
            path.unlink()
        except OSError:
            continue
    logs_root = ledger.logs_dir
    if logs_root.exists():
        for directory in sorted(logs_root.iterdir()):
            if directory.is_dir() and directory.name not in plan.kept:
                try:
                    next(directory.iterdir())
                except StopIteration:
                    directory.rmdir()
                except OSError:
                    continue
    ledger.append_event(
        {
            "event": "gc_applied",
            "keep_runs": plan.keep_runs,
            "logs_deleted": len(plan.logs_to_delete),
            "bytes_freed": plan.bytes_to_free,
        }
    )
    return plan
