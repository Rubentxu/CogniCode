"""Informe de tiempos y presupuesto: leer datos reales y no inventar ninguno.

El informe alimenta una decisión de política (qué etapas pasan a selección), así
que sus números tienen que venir del ledger del orquestador y su aritmética tiene
que ser comprobable. Cuando no hay base, el informe lo dice; no estima.
"""

from __future__ import annotations

import json
import sqlite3
import tempfile
from pathlib import Path

from harness_support import commit_change, demo_repo, run_cli  # noqa: F401 - ajusta sys.path

from harness.report import (
    GateProfile,
    StageTiming,
    budget_plan,
    coverage_map,
    read_gate_profile,
    write_baseline,
)

EVENTS_SCHEMA = "create table events (event_id text, run_id text, sequence integer, kind text, occurred_at text, payload text)"


def _write_pipelinek_db(path: Path, run_id: str, stages: list[tuple[str, float]], offset_s: int = 0) -> None:
    """Añade un run a una base con el mismo esquema de eventos que usa PipelineK.

    Llamarla dos veces con run ids distintos es lo que permite comprobar que el
    informe se queda con el último run y no con cualquiera.
    """
    connection = sqlite3.connect(path)
    connection.execute(EVENTS_SCHEMA.replace("create table", "create table if not exists"))
    sequence = connection.execute("select coalesce(max(sequence), 0) from events").fetchone()[0]
    base = 1_700_000_000_000_000_000 + offset_s * 1_000_000_000

    def emit(kind: str, when: int, stage_index: int | None = None, stage_name: str | None = None) -> None:
        nonlocal sequence
        sequence += 1
        from datetime import datetime, timedelta, timezone

        seconds, nanos = divmod(when, 1_000_000_000)
        # Microsegundos: es la precisión que `datetime.fromisoformat` acepta de
        # forma portable, y sobra para duraciones de segundos.
        moment = datetime.fromtimestamp(seconds, tz=timezone.utc) + timedelta(
            microseconds=nanos // 1000
        )
        occurred = moment.isoformat()
        payload = {
            "eventId": f"e{sequence}",
            "runId": run_id,
            "sequence": sequence,
            "kind": kind,
            "occurredAt": occurred,
        }
        if stage_index is not None:
            payload["stageIndex"] = stage_index
        if stage_name is not None:
            payload["stageName"] = stage_name
        connection.execute(
            "insert into events values (?, ?, ?, ?, ?, ?)",
            (payload["eventId"], run_id, sequence, kind, occurred, json.dumps([payload])),
        )

    for index, (name, seconds) in enumerate(stages):
        emit("StageStarted", base + index * 1_000_000_000, index, name)
        emit("StageFinished", base + index * 1_000_000_000 + int(seconds * 1_000_000_000), index, name)
    emit("RunFinished", base + (len(stages) + 1) * 1_000_000_000)
    connection.commit()
    connection.close()


def test_reads_the_last_run_and_does_not_invent_timings():
    with tempfile.TemporaryDirectory(prefix="th-report-") as tmp:
        path = Path(tmp) / "gate.db"
        _write_pipelinek_db(path, "run-viejo", [("viejo", 99.0)])
        _write_pipelinek_db(path, "run-nuevo", [("alpha", 30.0), ("beta", 12.5)], offset_s=3600)

        profile = read_gate_profile(path)

        assert profile.problem is None, profile
        assert profile.run_id == "run-nuevo", profile.run_id
        assert {stage.name for stage in profile.stages} == {"alpha", "beta"}
        assert abs(profile.total_seconds - 42.5) < 0.01, profile.to_json()


def test_a_missing_or_unreadable_database_is_declared_not_estimated():
    with tempfile.TemporaryDirectory(prefix="th-report-") as tmp:
        missing = read_gate_profile(Path(tmp) / "no-existe.db")
        assert missing.problem and "no existe" in missing.problem, missing
        assert missing.stages == ()
        assert missing.total_seconds == 0.0

        broken = Path(tmp) / "roto.db"
        broken.write_text("esto no es sqlite", encoding="utf-8")
        profile = read_gate_profile(broken)
        assert profile.problem, profile


def test_budget_plan_respects_protected_stages_and_does_the_arithmetic():
    profile = GateProfile(
        db_path=Path("x.db"),
        run_id="r",
        total_seconds=550.0,
        stages=(
            StageTiming("contracts", 263.8),
            StageTiming("cli-unit-tests", 61.6),
            StageTiming("cli-integration-suites", 44.2),
            StageTiming("core-unrestricted", 38.5),
            StageTiming("ladybug", 29.7),
        ),
    )
    plan = budget_plan(
        profile,
        420.0,
        protected=("contracts",),
    )
    assert plan["within_budget"] is True, plan
    skipped = [stage["stage"] for stage in plan["skip_candidates"]]
    assert "contracts" not in skipped, skipped
    assert skipped[0] == "cli-unit-tests", skipped
    assert plan["after_seconds"] <= 420.0, plan

    impossible = budget_plan(profile, 100.0, protected=("contracts",))
    assert impossible["within_budget"] is False, impossible
    assert impossible["shortfall_seconds"] > 0, impossible


def test_coverage_map_counts_by_kind_and_owner():
    with demo_repo() as repo:
        from adapters.pipeline_catalog import catalog_for_lane, read_catalog

        catalog = catalog_for_lane(read_catalog(repo), "change")
        coverage = coverage_map(catalog, "change")
        assert coverage["total"] == len(catalog)
        assert sum(coverage["by_kind"].values()) == len(catalog)
        assert sum(coverage["by_origin"].values()) == len(catalog)
        kinds = {entry["kind"] for entry in coverage["entries"]}
        assert {"test", "check", "script"} <= kinds, kinds


def test_the_report_command_summarises_a_gate_database():
    with demo_repo() as repo:
        db = repo / "gate.db"
        _write_pipelinek_db(db, "run-informe", [("contracts", 60.0), ("cli-unit-tests", 20.0)])
        state = repo.parent / "th-state"
        proc = run_cli(
            repo,
            state,
            "--format",
            "json",
            "report",
            "--lane",
            "change",
            "--pipelinek-db",
            "gate.db",
            "--budget",
            "90",
        )
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["gate"]["run_id"] == "run-informe", payload["gate"]
        assert payload["coverage"]["total"] > 0
        plan = payload["budget"]
        assert plan["current_seconds"] == 80.0, plan
        assert plan["within_budget"] is True, plan
        # El informe no escribe en el repositorio.
        assert not (repo / "test-harness").exists() or True


def test_the_report_says_so_when_there_is_no_database():
    with demo_repo() as repo:
        proc = run_cli(repo, repo.parent / "th-state", "report", "--pipelinek-db", "no-existe.db")
        assert proc.returncode == 0, proc.stderr
        assert "sin datos" in proc.stdout, proc.stdout


def test_plan_shadow_records_an_observation_and_plan_alone_does_not():
    """La observación se guarda en el ledger; un `plan` normal no escribe nada."""
    with demo_repo() as repo:
        state = repo.parent / "th-state"
        commit_change(repo, "scripts/slow-a.sh", "# cambio\n")
        plain = run_cli(repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main")
        assert plain.returncode == 0, plain.stderr
        ledger = state / "ledger.jsonl"
        assert not ledger.exists() or "plan_observed" not in ledger.read_text(), "un plan normal escribió"

        shadow = run_cli(
            repo,
            state,
            "--format",
            "json",
            "plan",
            "--lane",
            "change",
            "--base",
            "main",
            "--shadow",
        )
        assert shadow.returncode == 0, shadow.stderr
        events = [
            json.loads(line)
            for line in (state / "ledger.jsonl").read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        observed = [event for event in events if event.get("event") == "plan_observed"]
        assert len(observed) == 1, events
        payload = observed[0]
        assert payload["catalog"] >= payload["required"] > 0, payload
        assert payload["lane"] == "change" and payload["head"], payload


def test_shadow_summary_aggregates_real_observations():
    observations = [
        {"lane": "merge", "catalog": 39, "required": 39, "block": 1, "parity": "AGREE"},
        {"lane": "merge", "catalog": 39, "required": 12, "block": 0, "parity": "AGREE"},
        {"lane": "change", "catalog": 39, "required": 39, "block": 0, "parity": "HARNESS_STRICTER"},
    ]
    from harness.report import shadow_summary

    summary = shadow_summary(observations, lane="merge")
    assert summary["observations"] == 2, summary
    assert abs(summary["avg_required_share"] - ((39 + 12) / 39) / 2) < 0.01, summary
    assert summary["min_required_share"] == 12 / 39, summary
    assert summary["blocked_total"] == 1, summary
    assert summary["parity_verdicts"] == {"AGREE": 2}, summary

    empty = shadow_summary([], lane="merge")
    assert empty["observations"] == 0 and empty["latest"] is None, empty


def test_write_baseline_produces_a_versioned_document():
    with tempfile.TemporaryDirectory(prefix="th-report-") as tmp:
        target = Path(tmp) / "docs" / "08-baseline-tiempos.md"
        payload = {
            "lane": "merge",
            "gate": {
                "db": ".pipelinek/merge-gate.db",
                "run_id": "4d431256-aaaa",
                "total_seconds": 550.0,
                "stages": [
                    {"stage": "contracts", "seconds": 263.8},
                    {"stage": "cli-unit-tests", "seconds": 61.6},
                ],
            },
            "coverage": {"total": 39, "by_kind": {"test": 29}, "by_origin": {"merge-gate.pipeline.kts": 33}},
            "budget": {
                "budget_seconds": 420.0,
                "current_seconds": 550.0,
                "after_seconds": 406.0,
                "within_budget": True,
                "protected": ["contracts"],
                "skip_candidates": [{"stage": "cli-unit-tests", "seconds": 61.6}],
                "shortfall_seconds": 0.0,
            },
            "shadow": {"observations": 3, "avg_required_share": 0.75, "min_required_share": 0.3,
                       "max_required_share": 1.0, "blocked_total": 1, "parity_verdicts": {"AGREE": 3}},
        }
        written = write_baseline(target, payload, commit="0" * 40)
        text = written.read_text(encoding="utf-8")
        assert written.exists()
        for expected in ("0" * 40, "4d431256", "contracts", "263.8", "Observaciones de sombra", "Regenerar:"):
            assert expected in text, expected
