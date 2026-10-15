"""End-to-end: la CLI sobre un repositorio real, con cargo de verdad.

Cubre lo que ningún test unitario puede cubrir: que plan, run, reuse, resume y
explain funcionen encadenados, que el JSON que sale valide contra el contrato, y
que el harness **no** escriba nada dentro del repositorio.
"""

from __future__ import annotations

import json
from pathlib import Path

from harness_support import (
    APP_SUITE,
    CORE_SUITE,
    FMT_SUITE,
    commit_change,
    demo_repo,
    git_dirty,
    run_cli,
)

SCHEMAS_DIR = Path(__file__).resolve().parent.parent / "contracts"


def _validate(schema_name: str, payload: dict) -> None:
    """Valida contra el esquema publicado. jsonschema es dependencia declarada del gate."""
    import jsonschema

    schema = json.loads((SCHEMAS_DIR / schema_name).read_text(encoding="utf-8"))
    jsonschema.validate(instance=payload, schema=schema)


def _state(repo: Path) -> Path:
    return repo.parent / "th-state"


def test_plan_json_matches_the_contract():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        proc = run_cli(repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr + proc.stdout
        envelope = json.loads(proc.stdout)
        plan = envelope["plan"]
        _validate("plan.schema.json", plan)
        assert set(plan["required_suites"]) == {CORE_SUITE, APP_SUITE, FMT_SUITE}, plan
        assert envelope["run_id"], envelope
        actions = {action["suite_id"]: action for action in plan["actions"]}
        assert actions[CORE_SUITE]["action"] == "EXECUTE", actions
        assert actions[FMT_SUITE]["action"] == "EXECUTE", actions


def test_plan_text_explains_every_decision():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// cambio en app\n")
        proc = run_cli(repo, state, "plan", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        assert "motivo:" in proc.stdout
        assert CORE_SUITE in proc.stdout  # aparece como excluida, con su razón
        assert "ningún fichero de su clausura cambió" in proc.stdout


def test_run_executes_writes_receipts_and_leaves_the_repo_clean():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        before = git_dirty(repo)

        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr + proc.stdout
        payload = json.loads(proc.stdout)
        _validate("gate-verdict.schema.json", payload["verdict"])
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]
        assert {receipt["suite_id"] for receipt in payload["executed"]} == {
            CORE_SUITE,
            APP_SUITE,
            FMT_SUITE,
        }, payload["executed"]
        for receipt in payload["executed"]:
            _validate("receipt.schema.json", receipt)
            assert receipt["status"] == "PASSED", receipt
            assert receipt["log_sha256"], receipt
            if receipt["runner"]["name"] == "cargo-test":
                assert receipt["passed_cases"] >= 1, receipt

        # La evidencia va al estado, no al repositorio.
        assert (state / "receipts").is_dir()
        assert (state / "ledger.jsonl").is_file()
        assert git_dirty(repo) == before, "el harness escribió dentro del repositorio"
        assert not (repo / "th-state").exists()


def test_second_run_reuses_without_executing_again():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        first = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert first.returncode == 0, first.stderr

        second = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert second.returncode == 0, second.stderr
        payload = json.loads(second.stdout)
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]
        assert payload["executed"] == [], payload["executed"]
        assert len(payload["reused"]) == 3, payload["reused"]


def test_changing_a_source_invalidates_only_its_suites():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// app v1\n")
        first = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert first.returncode == 0, first.stderr

        commit_change(repo, "crates/app/src/lib.rs", "// app v2 distinta\n")
        plan_proc = run_cli(
            repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main"
        )
        envelope = json.loads(plan_proc.stdout)
        excluded = {entry["suite_id"]: entry["reason"] for entry in envelope["excluded"]}
        assert CORE_SUITE in excluded, excluded  # su clausura no cambió: ni se planifica
        assert "ningún fichero de su clausura cambió" in excluded[CORE_SUITE], excluded[CORE_SUITE]

        proc = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        assert APP_SUITE in executed, executed  # su código cambió: se reejecuta
        assert FMT_SUITE in executed, executed  # su clausura es todo el workspace
        assert CORE_SUITE not in executed, executed  # no se reejecuta código que nadie tocó


def test_merge_lane_executes_everything_by_default():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        warm = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert warm.returncode == 0, warm.stderr

        proc = run_cli(repo, state, "--format", "json", "plan", "--lane", "merge", "--base", "main")
        assert proc.returncode == 0, proc.stderr
        plan = json.loads(proc.stdout)["plan"]
        actions = {action["suite_id"]: action["action"] for action in plan["actions"]}
        assert set(actions.values()) == {"EXECUTE"}, actions


def test_interrupted_receipt_is_not_a_pass_and_resume_reexecutes():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        plan_proc = run_cli(
            repo, state, "--format", "json", "plan", "--lane", "change", "--base", "main"
        )
        plan = json.loads(plan_proc.stdout)["plan"]
        run_id = "run-interrumpido-de-prueba"

        # Un proceso que murió a mitad: run RUNNING sin evento terminal.
        (state / "runs").mkdir(parents=True, exist_ok=True)
        (state / "runs" / f"{run_id}.json").write_text(
            json.dumps(
                {
                    "schema_version": "th.run/v1",
                    "run_id": run_id,
                    "lane": "change",
                    "repo_id": "demo",
                    "plan_id": plan["plan_id"],
                    "source_fingerprint": plan["source_fingerprint"],
                    "base_ref": "main",
                    "status": "RUNNING",
                    "started_at": "2026-01-01T00:00:00Z",
                    "finished_at": "",
                    "suites": {},
                    "pending": [],
                }
            ),
            encoding="utf-8",
        )
        (state / "receipts").mkdir(parents=True, exist_ok=True)
        interrupted = {
            "schema_version": "th.receipt/v1",
            "receipt_id": "rcpt-interrumpido",
            "run_id": run_id,
            "repo_id": "demo",
            "suite_id": CORE_SUITE,
            "lane": "change",
            "status": "RUNNING",
            "input_fingerprint": "a" * 64,
            "source_snapshot": {
                "git_head": "0" * 40,
                "effective_content_digest": "b" * 64,
                "dirty": False,
                "worktree_id": "wt",
            },
            "runner": {
                "name": "cargo-test",
                "version": "cargo 1.0.0",
                "argv": ["cargo", "test", "-p", "demo-core", "--lib", "--quiet"],
                "features": [],
                "target_triple": "x86_64-unknown-linux-gnu",
            },
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "",
            "exit_code": -1,
            "discovered_cases": 0,
            "executed_cases": 0,
            "passed_cases": 0,
            "failed_cases": 0,
            "skipped_cases": 0,
            "log_sha256": "",
        }
        (state / "receipts" / "rcpt-interrumpido.json").write_text(
            json.dumps(interrupted), encoding="utf-8"
        )

        proc = run_cli(
            repo,
            state,
            "--format",
            "json",
            "resume",
            "--run-id",
            run_id,
            "--lane",
            "change",
            "--base",
            "main",
        )
        assert proc.returncode == 0, proc.stderr + proc.stdout
        payload = json.loads(proc.stdout)
        assert payload["verdict"]["verdict"] == "SATISFIED", payload["verdict"]
        executed = {receipt["suite_id"] for receipt in payload["executed"]}
        assert CORE_SUITE in executed, executed  # el recibo interrumpido no se acepta como PASS
        run_state = json.loads((state / "runs" / f"{run_id}.json").read_text(encoding="utf-8"))
        assert run_state["status"] == "FINISHED", run_state
        assert run_state["verdict"]["verdict"] == "SATISFIED", run_state["verdict"]


def test_status_reports_the_last_run():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/core/src/lib.rs", "// cambio en core\n")
        run = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert run.returncode == 0, run.stderr
        proc = run_cli(repo, state, "--format", "json", "status")
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["status"] == "FINISHED", payload
        assert payload["counts"]["executed"] == 3, payload["counts"]


def test_explain_shows_the_decision_and_the_evidence():
    with demo_repo() as repo:
        state = _state(repo)
        commit_change(repo, "crates/app/src/lib.rs", "// app\n")
        run = run_cli(repo, state, "--format", "json", "run", "--lane", "change", "--base", "main")
        assert run.returncode == 0, run.stderr

        proc = run_cli(
            repo, state, "--format", "json", "explain", "--suite", APP_SUITE, "--base", "main"
        )
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["decision"]["action"] == "REUSE", payload
        assert payload["decision"]["input_fingerprint"], payload
        assert payload["reuse"]["kind"] == "Reusable", payload["reuse"]

        other = run_cli(
            repo, state, "--format", "json", "explain", "--suite", CORE_SUITE, "--base", "main"
        )
        payload = json.loads(other.stdout)
        assert payload["decision"]["action"] == "NOT_REQUIRED", payload


def test_status_without_runs_is_a_harness_error():
    with demo_repo() as repo:
        proc = run_cli(repo, _state(repo), "status")
        assert proc.returncode == 2, proc
        assert "no hay ningún run" in proc.stderr, proc.stderr


def test_parallel_workers_are_refused_with_a_reason():
    with demo_repo() as repo:
        proc = run_cli(repo, _state(repo), "run", "--lane", "change", "--max-workers", "4")
        assert proc.returncode == 2, proc
        assert "no está implementado" in proc.stderr, proc.stderr


def test_unknown_suite_explain_is_honest():
    with demo_repo() as repo:
        proc = run_cli(
            repo,
            _state(repo),
            "--format",
            "json",
            "explain",
            "--suite",
            "no-existe:lib#-",
            "--base",
            "main",
        )
        assert proc.returncode == 0, proc.stderr
        payload = json.loads(proc.stdout)
        assert payload["decision"]["action"] == "UNKNOWN", payload
