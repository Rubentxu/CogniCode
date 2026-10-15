"""Ledger (atomicidad, inmutabilidad, leases) y clasificación de procesos."""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

from adapters.ledger_store import LeaseError, Ledger, receipt_from_json, state_root
from adapters.process_runner import CaseCounts, classify, execute, parse_counts
from harness_support import CORE_SUITE, make_receipt


def _expect_lease_error(action) -> LeaseError:
    try:
        action()
    except LeaseError as error:
        return error
    raise AssertionError("se esperaba un error de integridad del ledger")


def test_receipts_are_immutable_and_the_ledger_only_appends():
    with tempfile.TemporaryDirectory(prefix="th-ledger-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        receipt = make_receipt()
        path = ledger.write_receipt(receipt)
        assert path.exists()

        # Reintentar lo mismo es inocuo: red de seguridad contra reejecuciones.
        assert ledger.write_receipt(receipt) == path

        conflicting = make_receipt(
            status="FAILED", exit_code=101, passed=1, failed=1, executed=2
        )
        _expect_lease_error(lambda: ledger.write_receipt(conflicting))

        events = [json.loads(line) for line in ledger.ledger_path.read_text().splitlines()]
        assert [event["event"] for event in events] == ["receipt_written"], events
        assert events[0]["status"] == "PASSED"


def test_corrupt_receipt_is_not_evidence():
    with tempfile.TemporaryDirectory(prefix="th-ledger-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        receipt = make_receipt()
        path = ledger.write_receipt(receipt)
        text = path.read_text(encoding="utf-8")
        path.write_text(text[: len(text) // 2], encoding="utf-8")

        read_back, problem = ledger.read_receipt(receipt.receipt_id)
        assert read_back is None, read_back
        assert "corrupto" in problem, problem


def test_receipt_with_inconsistent_arithmetic_is_rejected():
    payload = {
        "schema_version": "th.receipt/v1",
        "receipt_id": "rcpt-x",
        "run_id": "run-x",
        "repo_id": "demo",
        "suite_id": CORE_SUITE,
        "lane": "change",
        "status": "PASSED",
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
            "argv": ["cargo", "test", "-p", "demo-core", "--lib"],
            "features": [],
            "target_triple": "x86_64-unknown-linux-gnu",
        },
        "started_at": "2026-01-01T00:00:00Z",
        "finished_at": "2026-01-01T00:00:01Z",
        "exit_code": 0,
        "discovered_cases": 5,
        "executed_cases": 4,
        "passed_cases": 1,
        "failed_cases": 0,
        "skipped_cases": 0,
        "log_sha256": "c" * 64,
    }
    receipt, problem = receipt_from_json(payload)
    assert receipt is not None
    assert "cuentas no cuadran" in problem, problem

    payload["executed_cases"] = 1
    payload["exit_code"] = 101
    _, problem = receipt_from_json(payload)
    assert "exit_code" in problem, problem


def test_index_rebuilds_from_the_receipts_directory():
    with tempfile.TemporaryDirectory(prefix="th-ledger-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        ledger.write_receipt(make_receipt(receipt_id="rcpt-viejo"))
        ledger.write_receipt(make_receipt(receipt_id="rcpt-nuevo"))
        # El índice ordena por `finished_at`; se fija a mano para que el orden sea claro.
        path = ledger.receipt_path("rcpt-nuevo")
        payload = json.loads(path.read_text(encoding="utf-8"))
        payload["finished_at"] = "2030-01-01T00:00:00Z"
        path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")

        ledger.rebuild_index()
        assert ledger.receipts_for_suite(CORE_SUITE)[0] == "rcpt-nuevo"

        (ledger.root / "index.json").unlink()
        assert ledger.receipts_for_suite(CORE_SUITE)[0] == "rcpt-nuevo"


def test_index_notices_receipts_written_by_another_process():
    """Un índice rancio diría "no hay recibo previo" teniendo el recibo delante."""
    with tempfile.TemporaryDirectory(prefix="th-ledger-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        assert ledger.receipts_for_suite(CORE_SUITE) == ()  # construye el índice vacío
        other_process = Ledger(Path(tmp) / "state")
        other_process.write_receipt(make_receipt(receipt_id="rcpt-otro"))

        assert ledger.receipts_for_suite(CORE_SUITE) == ("rcpt-otro",)


def test_lease_is_exclusive_and_releasable():
    with tempfile.TemporaryDirectory(prefix="th-ledger-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        first = ledger.acquire_lease("run-1", purpose="test")
        assert first.acquired, first.reason

        second = ledger.acquire_lease("run-1", purpose="test")
        assert second.acquired is False
        assert "ya tiene dueño" in second.reason

        ledger.release_lease(first)
        third = ledger.acquire_lease("run-1", purpose="test")
        assert third.acquired is True
        ledger.release_lease(third)


def test_state_root_lives_outside_the_repository():
    with tempfile.TemporaryDirectory(prefix="th-state-") as tmp:
        root = state_root("git@github.com:example/repo.git", base=Path(tmp))
        assert root.parent.name == "cognicode-test-harness", root
        assert root.name.startswith("repo-"), root


def test_case_counts_sum_every_target_block():
    """`--lib --bins` produce varios bloques: quedarse con uno cuenta de menos."""
    output = (
        "running 3 tests\n"
        "test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out\n"
        "\n"
        "running 2 tests\n"
        "test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
    )
    counts = parse_counts(output)
    assert counts == CaseCounts(discovered=5, executed=5, passed=4, failed=0, skipped=1), counts


def test_classification_never_invents_green():
    cases = (
        (0, "running 1 tests\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "PASSED"),
        (101, "running 1 tests\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n", "FAILED"),
        (101, "error: could not compile `demo-core` (lib test) due to 2 previous errors\n", "INFRA_ERROR"),
        (101, "error[E0308]: mismatched types\n", "INFRA_ERROR"),
        (101, "algo raro sin marcadores conocidos\n", "INFRA_ERROR"),
        (0, "todo bien pero sin tests\n", "PASSED"),
    )
    for exit_code, output, expected in cases:
        status, _ = classify(exit_code, output, timed_out=False)
        assert status == expected, (exit_code, output, status, expected)


def test_timeout_is_not_a_pass():
    status, classification = classify(-15, "a medias\n", timed_out=True)
    assert status == "TIMED_OUT", status
    assert classification == "timeout"


def test_execute_reports_exit_and_output():
    with tempfile.TemporaryDirectory(prefix="th-run-") as tmp:
        outcome, text = execute(
            (
                sys.executable,
                "-c",
                "print('running 1 tests'); print('test result: ok. 1 passed; 0 failed; 0 ignored;')",
            ),
            cwd=Path(tmp),
        )
        assert outcome.status == "PASSED", outcome
        assert outcome.exit_code == 0
        assert outcome.counts.passed == 1
        assert "test result" in text


def test_execute_marks_timeout_and_kills_the_process():
    with tempfile.TemporaryDirectory(prefix="th-run-") as tmp:
        outcome, _ = execute(
            (sys.executable, "-c", "import time; time.sleep(30)"),
            cwd=Path(tmp),
            timeout_s=1.0,
        )
        assert outcome.status == "TIMED_OUT", outcome
        assert outcome.classification == "timeout"


def test_execute_reports_missing_binary_as_infra_error():
    with tempfile.TemporaryDirectory(prefix="th-run-") as tmp:
        outcome, _ = execute(("no-existe-este-binario-harness",), cwd=Path(tmp))
        assert outcome.status == "INFRA_ERROR", outcome
        assert "no se pudo lanzar" in outcome.detail, outcome.detail


def test_script_runner_does_not_demand_case_counts():
    script_receipt = make_receipt(
        suite_id="workspace:script:run-all-contracts",
        runner_name="script",
        argv=("bash", "scripts/ci/run-all-contracts.sh"),
        executed=0,
        passed=0,
        discovered=0,
    )
    assert script_receipt.is_well_formed_pass() is True

    cargo_receipt = make_receipt(executed=0, passed=0, discovered=0)
    assert cargo_receipt.is_well_formed_pass() is False
