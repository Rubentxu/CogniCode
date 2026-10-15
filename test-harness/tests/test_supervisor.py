"""Supervisor: concurrencia, cerrojos, corte y reanudación, GC e inventario.

Lo que se prueba aquí es lo que TH-S03 llama «prueba adversarial»: dos intentos
concurrentes al mismo run, un SIGKILL después de escribir el marcador `RUNNING`,
un fallo del supervisor, y la recolección de basura del ledger. Ninguno de esos
casos puede acabar en verde inventado.
"""

from __future__ import annotations

import json
import os
import signal
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

from harness_support import (  # noqa: F401 - ajusta sys.path
    commit_change,
    demo_repo,
    intervals,
    overlap,
    run_cli,
    wait_for_file,
)

from adapters.ledger_store import Ledger
from harness.domain import PlanAction
from harness.gate import aggregate
from harness.gc import apply_gc, plan_gc
from harness.inventory import (
    Inventory,
    check_receipt_against_inventory,
    inventory_from_receipts,
    load_inventory,
    write_inventory,
)
from harness_support import CORE_SUITE, make_fingerprint, make_receipt
from harness.validity import evaluate_reuse

SLOW_A = "workspace:script:slow-a"
SLOW_B = "workspace:script:slow-b"
SLOW_C = "workspace:script:slow-c"


def _state(repo: Path) -> Path:
    return repo.parent / "th-state"


def _mark_dir(repo: Path) -> Path:
    mark = repo.parent / "marks"
    mark.mkdir(exist_ok=True)
    return mark


def _env_for_run(repo: Path, sleep_s: str = "0.4") -> dict[str, str]:
    mark = _mark_dir(repo)
    return {"TH_TEST_MARK": str(mark), "TH_TEST_SLEEP": sleep_s}


def _run_with_env(repo: Path, state: Path, env: dict[str, str], *args: str) -> subprocess.CompletedProcess:
    full_env = dict(os.environ)
    full_env.update(env)
    return subprocess.run(
        [
            sys.executable,
            str(Path(__file__).resolve().parent.parent / "bin" / "harness.py"),
            "--repo",
            str(repo),
            "--state-dir",
            str(state),
            *args,
        ],
        capture_output=True,
        text=True,
        check=False,
        env=full_env,
    )


def test_the_gate_serializes_serial_suites_and_lets_the_others_share():
    """Cerrojos: una suite serial corre sola; dos normales pueden solaparse."""
    from adapters.pipeline_catalog import CatalogEntry
    from harness.orchestrator import ResourceGate

    serial = CatalogEntry(
        suite_id="s",
        package="",
        target="script:x.sh",
        argv=("bash", "x.sh"),
        lanes=("change",),
        origin="test",
        scope="script",
        kind="script",
        serial=True,
    )
    normal = CatalogEntry(
        suite_id="n",
        package="",
        target="script:y.sh",
        argv=("bash", "y.sh"),
        lanes=("change",),
        origin="test",
        scope="script",
        kind="script",
    )
    gate = ResourceGate()
    inside_serial = []
    inside_normal = []

    def worker(entry, log):
        with gate.hold(entry):
            log.append(1)
            time.sleep(0.05)
            log.append(0)

    with_serial = threading.Thread(target=worker, args=(serial, inside_serial))
    with_normal = threading.Thread(target=worker, args=(normal, inside_normal))
    with_serial.start()
    time.sleep(0.01)
    with_normal.start()
    with_serial.join()
    with_normal.join()

    # La barrera serial bloquea a la normal: no puede haber dos dentro a la vez.
    assert sum(inside_serial) <= 1 and sum(inside_normal) <= 1
    assert gate.lock_names()


def test_two_script_suites_overlap_with_two_workers():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "scripts/slow-a.sh", "# cambio declarado\n")
        env = _env_for_run(repo)
        proc = _run_with_env(
            repo,
            state,
            env,
            "--format",
            "json",
            "run",
            "--lane",
            "change",
            "--base",
            "main",
            "--max-workers",
            "2",
            "--only",
            SLOW_A,
            "--only",
            SLOW_B,
        )
        payload = json.loads(proc.stdout)
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        assert executed == {SLOW_A, SLOW_B}, (payload, proc.stderr)
        marks = intervals(_mark_dir(repo))
        assert set(marks) == {"slow-a", "slow-b"}, marks
        assert overlap(marks["slow-a"], marks["slow-b"]), marks


def test_a_serial_suite_never_overlaps_even_with_workers_available():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "scripts/slow-b.sh", "# cambio declarado\n")
        env = _env_for_run(repo, "0.5")
        # Declaración explícita: slow-b toca recursos compartidos.
        config = repo / "test-harness" / "config"
        config.mkdir(parents=True, exist_ok=True)
        (config / "suite-metadata.json").write_text(
            json.dumps(
                {
                    "schema_version": "th.suite-metadata/v1",
                    "rules": [{"match": SLOW_B, "serial": True}],
                }
            ),
            encoding="utf-8",
        )
        proc = _run_with_env(
            repo,
            state,
            env,
            "--format",
            "json",
            "run",
            "--lane",
            "change",
            "--base",
            "main",
            "--max-workers",
            "2",
            "--only",
            SLOW_B,
            "--only",
            SLOW_C,
        )
        payload = json.loads(proc.stdout)
        assert {r["suite_id"] for r in payload["executed"]} == {SLOW_B, SLOW_C}, payload
        marks = intervals(_mark_dir(repo))
        assert not overlap(marks["slow-b"], marks["slow-c"]), marks
        run_state = json.loads((state / "runs" / payload["run_id"]).with_suffix(".json").read_text())
        assert run_state["concurrency"]["serial_suites"] == [SLOW_B], run_state["concurrency"]


def test_sigkill_leaves_no_pass_and_resume_reexecutes():
    """SIGKILL a mitad: el marcador RUNNING no es un PASS y resume lo rehace."""
    with demo_repo() as repo:
        state = _state(repo)
        # Un cambio en un script es global: el plan exige las suites y `--only`
        # limita la ejecución a la que interesa medir.
        commit_change(repo, "scripts/slow-a.sh", "# cambio declarado\n")
        env = _env_for_run(repo, "30")
        run_id = "run-corte-de-prueba"
        process = subprocess.Popen(
            [
                sys.executable,
                str(Path(__file__).resolve().parent.parent / "bin" / "harness.py"),
                "--repo",
                str(repo),
                "--state-dir",
                str(state),
                "run",
                "--lane",
                "change",
                "--base",
                "main",
                "--only",
                SLOW_A,
                "--run-id",
                run_id,
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
            env={**os.environ, **env},
        )
        try:
            running_seen = wait_for_file(
                state / "receipts", timeout_s=20
            )
            assert running_seen, "no aparecieron recibos"
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                running = list((state / "receipts").glob("*.json"))
                if any(json.loads(p.read_text())["status"] == "RUNNING" for p in running):
                    break
                time.sleep(0.05)
            assert any(
                json.loads(p.read_text())["status"] == "RUNNING"
                for p in (state / "receipts").glob("*.json")
            ), "no se escribió el marcador RUNNING antes de lanzar"
        finally:
            os.killpg(os.getpgid(process.pid), signal.SIGKILL)
            process.wait(timeout=30)

        assert process.returncode != 0, "el proceso cortado no debería salir en verde"
        run_state = json.loads((state / "runs" / f"{run_id}.json").read_text())
        assert run_state["status"] == "RUNNING", run_state["status"]

        resumed = _run_with_env(
            repo,
            state,
            _env_for_run(repo, "0.1"),
            "--format",
            "json",
            "resume",
            "--run-id",
            run_id,
            "--lane",
            "change",
            "--base",
            "main",
            "--only",
            SLOW_A,
        )
        payload = json.loads(resumed.stdout)
        executed = {receipt["suite_id"]: receipt["status"] for receipt in payload["executed"]}
        assert executed.get(SLOW_A) == "PASSED", payload
        # `--only` limita la ejecución: lo demás queda pendiente y el gate lo dice.
        # Un veredicto satisfecho aquí sería justo el verde inventado que se evita.
        assert payload["verdict"]["verdict"] == "UNSATISFIED", payload["verdict"]
        assert set(payload["pending"]) >= {"demo-core:lib#-", "demo-app:lib#-"}, payload["pending"]
        gaps = {gap["suite_id"] for gap in payload["verdict"]["gaps"]}
        assert "demo-core:lib#-" in gaps, gaps
        assert SLOW_A not in gaps, gaps  # la suite interrumpida sí tiene PASS nuevo
        receipts = [json.loads(p.read_text()) for p in (state / "receipts").glob("*.json")]
        # Nunca hubo dos terminales para la misma suite en este run: el corte no dejó PASS.
        terminals = [
            r for r in receipts if r["suite_id"] == SLOW_A and r["status"] == "PASSED"
        ]
        assert len(terminals) == 1, terminals
        run_state = json.loads((state / "runs" / f"{run_id}.json").read_text())
        assert run_state["status"] == "FAILED", run_state["status"]


def test_two_runs_with_the_same_id_do_not_both_write():
    with demo_repo() as repo:
        state = _state(repo)
        run_id = "run-duplicado-de-prueba"
        commit_change(repo, "scripts/slow-a.sh", "# cambio declarado\n")
        env = dict(os.environ)
        env.update(_env_for_run(repo, "30"))
        first = subprocess.Popen(
            [
                sys.executable,
                str(Path(__file__).resolve().parent.parent / "bin" / "harness.py"),
                "--repo",
                str(repo),
                "--state-dir",
                str(state),
                "run",
                "--lane",
                "change",
                "--base",
                "main",
                "--only",
                SLOW_A,
                "--run-id",
                run_id,
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            start_new_session=True,
            env=env,
        )
        try:
            assert wait_for_file(state / "leases", timeout_s=20), "no se tomó el lease"
            second = _run_with_env(
                repo,
                state,
                env,
                "--format",
                "json",
                "run",
                "--lane",
                "change",
                "--base",
                "main",
                "--only",
                SLOW_A,
                "--run-id",
                run_id,
            )
            assert second.returncode == 2, (second.returncode, second.stdout, second.stderr)
            assert "ya tiene dueño" in second.stderr, second.stderr
        finally:
            os.killpg(os.getpgid(first.pid), signal.SIGKILL)
            first.wait(timeout=30)


def test_inventory_pin_flags_a_suite_that_lost_cases():
    pinned = make_receipt(suite_id=CORE_SUITE, executed=2, passed=2)
    problem = check_receipt_against_inventory(pinned, 40)
    assert problem and "inventario pinado" in problem, problem
    assert check_receipt_against_inventory(make_receipt(suite_id=CORE_SUITE, executed=40, passed=40), 40) is None
    # Un recibo que no es PASS no se juzga por el pin: ya es un hueco por sí mismo.
    assert check_receipt_against_inventory(make_receipt(status="FAILED", executed=0), 40) is None


def test_inventory_pin_invalidates_reuse_of_a_shrunken_receipt():
    expected = make_fingerprint(expected_min_cases=40)
    receipt = make_receipt(fingerprint=expected.digest(), executed=2, passed=2)
    decision = evaluate_reuse(receipt, expected, lane_reuse_allowed=True, lane_reason="change")
    assert decision.kind == "Invalidated", decision
    assert "inventario pinado" in decision.reason


def test_the_gate_does_not_accept_a_pass_below_the_pinned_minimum():
    verdict = aggregate(
        lane="change",
        repo_id="demo",
        source_fingerprint="c" * 64,
        required=("a",),
        actions=(PlanAction("a", "EXECUTE", "x"),),
        receipts={"a": make_receipt(suite_id="a", executed=3, passed=3)},
        minimums={"a": 10},
    )
    assert verdict.verdict == "UNSATISFIED", verdict.to_json()
    assert "inventario pinado" in verdict.gaps[0]["reason"], verdict.gaps


def test_inventory_pin_is_written_from_real_receipts():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio para el pin\n")
        run = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert run.returncode == 0, run.stderr
        written = run_cli(repo, state, "inventory", "--write")
        assert written.returncode == 0, written.stderr
        inventory = load_inventory(repo)
        assert inventory.problem is None, inventory
        assert inventory.expected_min_for(CORE_SUITE) >= 1, inventory.entries
        checked = run_cli(repo, state, "inventory", "--check")
        assert checked.returncode == 0, checked.stdout + checked.stderr


def test_inventory_from_receipts_only_pins_what_passed():
    receipts = [
        make_receipt(suite_id="a", executed=5, passed=5),
        make_receipt(suite_id="a", executed=9, passed=9),
        make_receipt(suite_id="b", status="FAILED", executed=0, passed=0, failed=3),
        make_receipt(suite_id="c", executed=0, passed=0, discovered=0),
    ]
    inventory = inventory_from_receipts(receipts)
    assert inventory.entries == {"a": 9}, inventory.entries
    with tempfile.TemporaryDirectory(prefix="th-inv-") as tmp:
        path = write_inventory(Path(tmp), inventory)
        assert json.loads(path.read_text())["schema_version"] == "th.inventory/v1"
        assert load_inventory(Path(tmp)).entries == {"a": 9}


def test_gc_keeps_evidence_and_only_removes_old_logs():
    with tempfile.TemporaryDirectory(prefix="th-gc-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        for index in range(3):
            run_id = f"run-{index}"
            ledger.write_run(
                run_id,
                {
                    "schema_version": "th.run/v1",
                    "run_id": run_id,
                    "lane": "change",
                    "status": "FINISHED",
                    "started_at": f"2026-01-0{index + 1}T00:00:00Z",
                    "finished_at": f"2026-01-0{index + 1}T00:01:00Z",
                    "suites": {},
                },
            )
            ledger.write_log(run_id, "suite:lib", b"salida de la suite")
        plan = plan_gc(ledger, keep_runs=1)
        assert plan.kept == ["run-2"], plan.to_json()
        assert len(plan.logs_to_delete) == 2, plan.to_json()
        assert plan.bytes_to_free > 0

        apply_gc(ledger, plan)
        assert not (ledger.logs_dir / "run-0").exists()
        assert not (ledger.logs_dir / "run-1").exists()
        assert (ledger.logs_dir / "run-2").exists()
        # La evidencia no se toca: los runs siguen ahí.
        assert {run["run_id"] for run in ledger.list_runs()} == {"run-0", "run-1", "run-2"}


def test_gc_dry_run_deletes_nothing():
    with tempfile.TemporaryDirectory(prefix="th-gc-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        ledger.write_run(
            "run-0",
            {"run_id": "run-0", "lane": "change", "status": "FINISHED", "started_at": "2026-01-01T00:00:00Z"},
        )
        ledger.write_run(
            "run-1",
            {"run_id": "run-1", "lane": "change", "status": "FINISHED", "started_at": "2026-01-02T00:00:00Z"},
        )
        ledger.write_log("run-0", "suite:lib", b"viejo")
        plan = plan_gc(ledger, keep_runs=1)
        assert plan.logs_to_delete, plan.to_json()
        # Un plan no borra nada por sí solo.
        assert (ledger.logs_dir / "run-0").exists()


def test_gc_preserves_a_run_that_is_still_running():
    with tempfile.TemporaryDirectory(prefix="th-gc-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        ledger.write_run(
            "run-viejo",
            {"run_id": "run-viejo", "lane": "change", "status": "FINISHED", "started_at": "2026-01-01T00:00:00Z"},
        )
        ledger.write_run(
            "run-nuevo",
            {"run_id": "run-nuevo", "lane": "change", "status": "FINISHED", "started_at": "2026-01-02T00:00:00Z"},
        )
        ledger.write_run(
            "run-vivo",
            {"run_id": "run-vivo", "lane": "change", "status": "RUNNING", "started_at": "2026-01-03T00:00:00Z"},
        )
        ledger.write_log("run-vivo", "suite:lib", b"en curso")
        plan = plan_gc(ledger, keep_runs=1)
        assert "run-vivo" in plan.kept, plan.to_json()
        assert not any("run-vivo" in item for item in plan.logs_to_delete), plan.to_json()
        assert any("RUNNING" in note for note in plan.notes), plan.notes


def test_a_context_with_an_unreadable_inventory_warns_instead_of_guessing():
    with demo_repo() as repo:
        package = repo / "test-harness"
        package.mkdir(exist_ok=True)
        (package / "inventory.json").write_text("{no es json", encoding="utf-8")
        state = _state(repo)
        proc = run_cli(repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main")
        envelope = json.loads(proc.stdout)
        assert any("inventario" in warning for warning in envelope["warnings"]), envelope["warnings"]
        # Y sin pin no se inventa un mínimo: las suites de Cargo siguen exigiendo 1.
        inventory = load_inventory(repo)
        assert inventory.problem and inventory.entries == {}
