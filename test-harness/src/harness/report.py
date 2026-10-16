"""Informe reproducible de tiempos y cobertura (W0 del roadmap del harness).

La decisión de qué etapas del merge pueden pasar a ejecutarse por impacto necesita
dos cosas que hasta ahora solo existían repartidas entre un recibo y una consulta
ad-hoc a la base de PipelineK: **cuánto cuesta cada etapa hoy** y **qué obligación
del harness le corresponde**. Este módulo las junta y las imprime, sin escribir
nada en el repositorio y sin tocar la base del orquestador (se abre en solo
lectura).

Dos reglas de honestidad sobre los números:

  * La duración sale del ledger del orquestador (`events`: `StageStarted` /
    `StageFinished`), no de una estimación. Cuando no hay base, el informe dice
    que no la hay en vez de inventar tiempos.
  * El presupuesto es aritmética declarada: dice cuánto habría que dejar de
    ejecutar para llegar a él, y qué etapas lo permitirían de mayor a menor
    coste. No decide nada por sí solo.
"""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path

STAGE_STARTED = "StageStarted"
STAGE_FINISHED = "StageFinished"
RUN_FINISHED = "RunFinished"


@dataclass
class StageTiming:
    name: str
    seconds: float
    index: int | None = None

    def to_json(self) -> dict:
        return {"stage": self.name, "seconds": round(self.seconds, 1), "index": self.index}


@dataclass
class GateProfile:
    db_path: Path | None
    run_id: str | None = None
    total_seconds: float = 0.0
    stages: tuple[StageTiming, ...] = ()
    problem: str | None = None

    def to_json(self) -> dict:
        return {
            "db": str(self.db_path) if self.db_path else None,
            "run_id": self.run_id,
            "total_seconds": round(self.total_seconds, 1),
            "stages": [stage.to_json() for stage in self.stages],
            "problem": self.problem,
        }


def _parse(value: str) -> datetime | None:
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def read_gate_profile(db_path: Path) -> GateProfile:
    """Perfil del último run registrado en la base de PipelineK (solo lectura)."""
    path = Path(db_path)
    if not path.exists():
        return GateProfile(db_path=path, problem=f"no existe la base {path}")
    try:
        connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    except sqlite3.Error as exc:
        return GateProfile(db_path=path, problem=f"no se pudo abrir la base: {exc}")

    events: list[dict] = []
    try:
        for (payload,) in connection.execute("select payload from events order by sequence"):
            try:
                items = json.loads(payload)
            except json.JSONDecodeError:
                continue
            for item in items if isinstance(items, list) else [items]:
                if isinstance(item, dict) and item.get("kind") in (
                    STAGE_STARTED,
                    STAGE_FINISHED,
                    RUN_FINISHED,
                ):
                    events.append(item)
    except sqlite3.Error as exc:
        return GateProfile(db_path=path, problem=f"la tabla de eventos no es legible: {exc}")
    finally:
        connection.close()

    if not events:
        return GateProfile(db_path=path, problem="la base no tiene eventos de etapa")

    runs: dict[str, list[dict]] = {}
    for event in events:
        run_id = event.get("runId")
        if run_id:
            runs.setdefault(run_id, []).append(event)
    last_run = max(
        runs,
        key=lambda run_id: max(event.get("sequence", 0) for event in runs[run_id]),
    )

    starts: dict[tuple, datetime] = {}
    timings: list[StageTiming] = []
    for event in sorted(runs[last_run], key=lambda item: item.get("sequence", 0)):
        kind = event.get("kind")
        if kind not in (STAGE_STARTED, STAGE_FINISHED):
            continue
        when = _parse(event.get("occurredAt", ""))
        if when is None:
            continue
        key = (event.get("stageIndex"), event.get("stageName"))
        if kind == STAGE_STARTED:
            starts[key] = when
            continue
        started = starts.pop(key, None)
        if started is None:
            continue
        timings.append(
            StageTiming(
                name=str(event.get("stageName")),
                seconds=(when - started).total_seconds(),
                index=event.get("stageIndex"),
            )
        )

    return GateProfile(
        db_path=path,
        run_id=last_run,
        total_seconds=sum(timing.seconds for timing in timings),
        stages=tuple(timings),
    )


def budget_plan(profile: GateProfile, budget_seconds: float, *, protected: tuple[str, ...] = ()) -> dict:
    """Qué habría que dejar de ejecutar para caber en el presupuesto.

    `protected` son las etapas que no se pueden tocar (por dueño único, por
    política o porque no hay alternativa donde ejecutarlas). El plan las respeta y
    dice, si aun así no cabe, cuánto falta.
    """
    candidates = sorted(
        (stage for stage in profile.stages if stage.name not in protected),
        key=lambda stage: stage.seconds,
        reverse=True,
    )
    to_skip: list[StageTiming] = []
    remaining = profile.total_seconds
    for stage in candidates:
        if remaining <= budget_seconds:
            break
        to_skip.append(stage)
        remaining -= stage.seconds
    return {
        "budget_seconds": budget_seconds,
        "current_seconds": round(profile.total_seconds, 1),
        "after_seconds": round(remaining, 1),
        "within_budget": remaining <= budget_seconds,
        "protected": list(protected),
        "skip_candidates": [stage.to_json() for stage in to_skip],
        "shortfall_seconds": round(max(0.0, remaining - budget_seconds), 1),
    }


@dataclass
class HarnessTimings:
    """Duración observada por el propio harness, de su run más reciente."""

    run_id: str | None = None
    by_suite: dict[str, float] = field(default_factory=dict)

    def to_json(self) -> dict:
        return {
            "run_id": self.run_id,
            "suites": {key: round(self.by_suite[key], 1) for key in sorted(self.by_suite)},
        }


def harness_timings(ledger, lane: str | None = None) -> HarnessTimings:
    run = ledger.latest_run(lane)
    if not run:
        return HarnessTimings()
    by_suite: dict[str, float] = {}
    for suite_id, info in (run.get("suites") or {}).items():
        duration = (info or {}).get("duration_s")
        if isinstance(duration, (int, float)):
            by_suite[suite_id] = float(duration)
    return HarnessTimings(run_id=run.get("run_id"), by_suite=by_suite)


def read_shadow_observations(ledger) -> list[dict]:
    """Lee las observaciones `plan_observed` del ledger append-only del harness.

    Las escribe la etapa de sombra en cada merge. Un log de gate no se puede
    agregar; el ledger sí, y es el dato con el que se decide la adopción.
    """
    path = getattr(ledger, "ledger_path", None)
    if path is None or not Path(path).exists():
        return []
    observations: list[dict] = []
    for line in Path(path).read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict) and payload.get("event") == "plan_observed":
            observations.append(payload)
    return observations


def shadow_summary(observations: list[dict], *, lane: str | None = None) -> dict:
    """Agrega las observaciones: cuánto exigiría la selección en los PR reales."""
    selected = [item for item in observations if lane is None or item.get("lane") == lane]
    if not selected:
        return {
            "observations": 0,
            "lane": lane,
            "avg_required_share": 0.0,
            "blocked_total": 0,
            "parity_verdicts": {},
            "latest": None,
        }
    shares = [
        (item.get("required", 0) / item["catalog"]) if item.get("catalog") else 0.0
        for item in selected
    ]
    verdicts: dict[str, int] = {}
    for item in selected:
        verdict = item.get("parity") or "sin oráculo"
        verdicts[verdict] = verdicts.get(verdict, 0) + 1
    return {
        "observations": len(selected),
        "lane": lane,
        "avg_required_share": sum(shares) / len(shares),
        "min_required_share": min(shares),
        "max_required_share": max(shares),
        "blocked_total": sum(int(item.get("block", 0)) for item in selected),
        "parity_verdicts": verdicts,
        "latest": selected[-1],
    }


def write_baseline(path: Path, payload: dict, *, commit: str) -> Path:
    """Escribe el informe como documento versionado con fecha y commit.

    El baseline no se mantiene a mano: se regenera con el comando y queda en el
    repositorio con la corrida de la que salió. Así una comparación futura tiene
    una referencia concreta en vez de «lo que midió alguien».
    """
    from datetime import timezone

    stamp = datetime.now(timezone.utc).replace(microsecond=0).isoformat()
    gate = payload.get("gate", {})
    coverage = payload.get("coverage", {})
    budget = payload.get("budget")
    shadows = payload.get("shadow", {})
    lines: list[str] = [
        "# Baseline de tiempos y cobertura del gate de merge",
        "",
        "Documento generado, no escrito a mano: se regenera con el comando de abajo.",
        "",
        f"- **Fecha**: {stamp}",
        f"- **Commit del árbol medido**: `{commit or 'desconocido'}`",
        f"- **Lane**: `{payload.get('lane')}`",
        f"- **Run de PipelineK**: `{(gate.get('run_id') or 'sin datos')[:12] if gate.get('run_id') else 'sin datos'}` "
        f"({'/'.join([str(gate.get('db'))]) if gate.get('db') else 'sin base'})",
        f"- **Etapas medidas**: {len(gate.get('stages') or [])}",
        f"- **Total medido**: {gate.get('total_seconds', 0)} s",
        "",
        "Regenerar:",
        "",
        "```shell",
        f"python3 test-harness/bin/harness.py report --lane {payload.get('lane')} \\",
        "  --pipelinek-db .pipelinek/merge-gate.db --budget 420 \\",
        "  --protect contracts --protect fmt --protect clippy \\",
        "  --write-baseline test-harness/docs/08-baseline-tiempos.md",
        "```",
        "",
        "## Etapas medidas (mayor a menor)",
        "",
        "| Etapa | Segundos |",
        "|---|---:|",
    ]
    for stage in sorted(gate.get("stages") or [], key=lambda item: -item["seconds"]):
        lines.append(f"| `{stage['stage']}` | {stage['seconds']} |")
    lines += [
        "",
        "## Cobertura del catálogo",
        "",
        f"- Obligaciones de la lane: {coverage.get('total', 0)}",
        f"- Por tipo: {coverage.get('by_kind', {})}",
        f"- Por pipeline de origen: {coverage.get('by_origin', {})}",
        "",
    ]
    if budget:
        lines += [
            "## Presupuesto",
            "",
            f"- Presupuesto declarado: {budget['budget_seconds']} s",
            f"- Actual: {budget['current_seconds']} s → {budget['after_seconds']} s dejando de ejecutar "
            f"{len(budget['skip_candidates'])} etapa(s)",
            f"- Cabe: {'sí' if budget['within_budget'] else 'no'} "
            f"(faltan {budget['shortfall_seconds']} s)",
            f"- Protegidas: {', '.join(budget['protected']) or 'ninguna'}",
            "",
        ]
    if shadows.get("observations"):
        lines += [
            "## Observaciones de sombra",
            "",
            f"- Planes observados en la lane: {shadows['observations']}",
            f"- Obligaciones exigidas de media: {shadows['avg_required_share']:.0%} "
            f"(min {shadows['min_required_share']:.0%}, max {shadows['max_required_share']:.0%})",
            f"- Bloqueadas por estado externo: {shadows['blocked_total']}",
            f"- Paridad con el oráculo: {shadows['parity_verdicts']}",
            "",
        ]
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def coverage_map(catalog, lane: str) -> dict:
    """Inventario de obligaciones de la lane, con dueño y tipo."""
    by_kind: dict[str, int] = {}
    by_origin: dict[str, int] = {}
    entries = []
    for entry in catalog:
        by_kind[entry.kind] = by_kind.get(entry.kind, 0) + 1
        by_origin[entry.origin] = by_origin.get(entry.origin, 0) + 1
        entries.append(
            {
                "suite_id": entry.suite_id,
                "kind": entry.kind,
                "scope": entry.scope,
                "origin": entry.origin,
                "lanes": list(entry.lanes),
                "serial": entry.serial,
                "advisory": entry.advisory,
            }
        )
    return {
        "lane": lane,
        "total": len(entries),
        "by_kind": by_kind,
        "by_origin": by_origin,
        "entries": entries,
    }
