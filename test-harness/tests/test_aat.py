"""AAT ejecutado: cada fila se mata con una señal concreta, o se declara.

El AAT no pide «el mutante compila», pide que el validador lo **mate** con una
señal concreta y que quede dicho cuál. Estas pruebas inyectan la mutación de cada
fila ejecutable y comprueban el rechazo exacto; las filas que dependen de sandbox,
nextest o artefactos de release se declaran `NOT_RUN` en `aat/AAT.md` con su
motivo, porque fingir esa cobertura sería justo lo contrario de lo que el AAT
mide.
"""

from __future__ import annotations

import json
import tempfile
from pathlib import Path

from harness_support import commit_change, demo_repo  # noqa: F401 - ajusta sys.path

from adapters.git_repo import ChangeSet
from adapters.ledger_store import Ledger, receipt_from_json
from adapters.pipeline_catalog import catalog_for_lane, read_catalog
from adapters.process_runner import classify
from harness.domain import PlanAction
from harness.gate import aggregate
from harness.obligations import Floor, load_floor, missing_from_catalog, unknown_obligations
from harness.planner import build_plan
from harness_support import CORE_SUITE, make_fingerprint, make_receipt
from harness.validity import evaluate_reuse

CLIPPY = "workspace:cargo:clippy#-"
FMT = "workspace:cargo:fmt#-"


def _aggregate(**overrides):
    payload = dict(
        lane="merge",
        repo_id="demo",
        source_fingerprint="c" * 64,
        required=("a",),
        actions=(PlanAction("a", "EXECUTE", "x"),),
        receipts={"a": make_receipt(suite_id="a")},
    )
    payload.update(overrides)
    return aggregate(**payload)


def test_aat_02_a_required_suite_without_receipt_turns_the_gate_red():
    """Falta la evidencia de un escenario requerido: hueco, no silencio."""
    verdict = _aggregate(receipts={})
    assert verdict.verdict == "UNSATISFIED", verdict.to_json()
    assert "sin recibo" in verdict.gaps[0]["reason"], verdict.gaps


def test_aat_03_removing_features_changes_the_fingerprint_and_the_reuse():
    """Quitar un `--features` es cambiar la suite: huella distinta, no reutilizable."""
    with_features = make_fingerprint(features=("evidence-kernel",))
    without = make_fingerprint(features=())
    assert with_features.digest() != without.digest()
    receipt = make_receipt(fingerprint=without.digest(), features=())
    decision = evaluate_reuse(
        receipt, with_features, lane_reuse_allowed=True, lane_reason="change"
    )
    assert decision.kind == "Invalidated", decision
    assert "features" in decision.reason or "huella" in decision.reason, decision.reason


def test_aat_04_a_missing_critical_obligation_leaves_the_plan_indeterminate():
    """Borrar la obligación de doctests del catálogo no puede pasar desapercibido."""
    with demo_repo() as repo:
        catalog = read_catalog(repo)
        floor = Floor(entries={"merge": (FMT, "cognicode-cli:doc#-")})
        pruned = tuple(entry for entry in catalog if entry.suite_id != "cognicode-cli:doc#-")
        assert missing_from_catalog(floor, "merge", {e.suite_id for e in pruned}) == (
            "cognicode-cli:doc#-",
        )
        plan_result = build_plan(
            lane="merge",
            repo_id="demo",
            changeset=ChangeSet(
                repo_root=repo, base_ref=None, merge_base=None, head="0" * 40, changed=("a.rs",), dirty=True
            ),
            graph=_real_graph(repo),
            catalog=pruned,
            ledger=None,
            toolchain=_toolchain(repo),
            obligation_floor=floor,
            run_id="run-aat-04",
            created_at="2026-01-01T00:00:00Z",
        )
        assert plan_result.plan.status == "UNKNOWN_INPUT", plan_result.plan.status
        assert any("cognicode-cli:doc#-" in reason for reason in plan_result.plan.unknown_reasons), (
            plan_result.plan.unknown_reasons
        )


def test_aat_06_removing_clippy_from_the_gate_is_detected_as_indeterminate():
    with demo_repo() as repo:
        catalog = read_catalog(repo)
        floor = Floor(entries={"merge": (CLIPPY,)})
        pruned = tuple(entry for entry in catalog if entry.suite_id != CLIPPY)
        plan_result = build_plan(
            lane="merge",
            repo_id="demo",
            changeset=ChangeSet(
                repo_root=repo, base_ref=None, merge_base=None, head="0" * 40, changed=("a.rs",), dirty=True
            ),
            graph=_real_graph(repo),
            catalog=pruned,
            ledger=None,
            toolchain=_toolchain(repo),
            obligation_floor=floor,
            run_id="run-aat-06",
            created_at="2026-01-01T00:00:00Z",
        )
        assert plan_result.plan.status == "UNKNOWN_INPUT", plan_result.plan.status
        assert any(CLIPPY in reason for reason in plan_result.plan.unknown_reasons), (
            plan_result.plan.unknown_reasons
        )


def test_aat_06b_the_real_floor_matches_the_real_catalog():
    """Un suelo que nombra suites inexistentes deja de proteger: se comprueba."""
    with demo_repo() as repo:
        floor_path = repo / "test-harness" / "obligations.json"
        floor_path.parent.mkdir(parents=True, exist_ok=True)
        floor_path.write_text(
            json.dumps({"schema_version": "th.obligations/v1", "entries": {"change": [FMT]}}),
            encoding="utf-8",
        )
        floor = load_floor(repo)
        assert floor.problem is None, floor
        catalog = read_catalog(repo)
        assert unknown_obligations(floor, {entry.suite_id for entry in catalog}) == ()
        assert missing_from_catalog(floor, "change", {entry.suite_id for entry in catalog}) == ()


def test_aat_16_a_receipt_from_another_repository_is_not_reused():
    """Mismo nombre de directorio, otro repositorio: no es la misma identidad."""
    expected = make_fingerprint()
    receipt = make_receipt(fingerprint=expected.digest())  # repo_id = "demo-repo"
    decision = evaluate_reuse(
        receipt,
        expected,
        lane_reuse_allowed=True,
        lane_reason="change",
        expected_repo_id="git@github.com:otro/repo.git",
    )
    assert decision.kind == "Invalidated", decision
    assert "otro repositorio" in decision.reason, decision.reason

    same = evaluate_reuse(
        receipt,
        expected,
        lane_reuse_allowed=True,
        lane_reason="change",
        expected_repo_id="demo-repo",
    )
    assert same.kind == "Reusable", same


def test_aat_20_a_hostile_suite_id_cannot_escape_the_state_root():
    """Un id de suite no puede escribir fuera del directorio de logs."""
    with tempfile.TemporaryDirectory(prefix="th-aat-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        hostile = "../../../etc/passwd"
        path, _ = ledger.write_log("run-hostil", hostile, b"contenido")
        resolved = path.resolve()
        assert str(resolved).startswith(str((ledger.root / "logs").resolve())), resolved
        assert ".." not in path.parts, path
        receipt = make_receipt(suite_id=hostile)
        assert ledger.write_receipt(receipt).exists()
        stored = sorted(p.name for p in ledger.receipts_dir.glob("*.json"))
        assert stored == [f"{receipt.receipt_id}.json"], stored

        # Un symlink plantado donde el harness va a escribir no puede redirigir la
        # escritura fuera del state root: la escritura es atómica por rename, así
        # que reemplaza el enlace en vez de seguirlo.
        outside = Path(tmp) / "fuera-del-state.txt"
        outside.write_text("intacto", encoding="utf-8")
        planted = ledger.logs_dir / "run-symlink"
        planted.mkdir(parents=True, exist_ok=True)
        link = planted / "suite-a.log"
        link.symlink_to(outside)
        written, _ = ledger.write_log("run-symlink", "suite-a", b"log de verdad")
        assert outside.read_text(encoding="utf-8") == "intacto", "la escritura siguió el symlink"
        assert not written.is_symlink(), written
        assert written.read_bytes() == b"log de verdad"


def test_aat_24_the_word_pass_in_a_failing_log_does_not_pass():
    """El resultado sale del código de salida y de las cuentas, no de un grep."""
    output = (
        "test result: PASS ok. 0 passed; 0 failed; 0 ignored;\n"
        "error: could not compile `demo-core` (lib test) due to 2 previous errors\n"
    )
    status, classification = classify(101, output, timed_out=False)
    assert status == "INFRA_ERROR", (status, classification)

    # Y un recibo que se inventa PASSED con un log de error sigue sin ser PASS.
    payload = make_receipt().to_json()
    payload["status"] = "PASSED"
    payload["exit_code"] = 101
    receipt, problem = receipt_from_json(payload)
    assert problem and "exit_code" in problem, problem


def test_aat_25_a_ci_change_forces_everything():
    with demo_repo() as repo:
        commit_change(repo, "scripts/ci/algo.sh", "echo hola\n")
        catalog = catalog_for_lane(read_catalog(repo), "change")
        from harness.planner import select

        selection = select(
            lane="change",
            changeset=ChangeSet(
                repo_root=repo,
                base_ref=None,
                merge_base=None,
                head="0" * 40,
                changed=("scripts/ci/algo.sh",),
                dirty=True,
            ),
            graph=_real_graph(repo),
            catalog=catalog,
            repo_root=repo,
        )
        assert {entry.suite_id for entry, _ in selection.required} == {
            entry.suite_id for entry in catalog
        }, selection
        assert any("scripts/ci" in reason for reason in selection.global_reasons), selection.global_reasons


def test_aat_27_a_suite_that_flipped_with_the_same_fingerprint_is_not_stable():
    """PASS una vez y FAIL otra con la misma huella: no se certifica estabilidad."""
    from harness.orchestrator import _unstable_suites, Context

    with tempfile.TemporaryDirectory(prefix="th-aat-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        fingerprint = make_fingerprint().digest()
        ledger.write_receipt(
            make_receipt(
                suite_id="a", receipt_id="rcpt-pass", fingerprint=fingerprint, executed=3, passed=3
            )
        )
        ledger.write_receipt(
            make_receipt(
                suite_id="a",
                receipt_id="rcpt-fail",
                fingerprint=fingerprint,
                status="FAILED",
                exit_code=101,
                executed=3,
                passed=2,
                failed=1,
            )
        )
        context = Context(
            repo_root=Path(tmp),
            repo_id="demo",
            ledger=ledger,
            toolchain=None,
            changeset=None,
            graph=None,
            catalog=(),
        )
        unstable = _unstable_suites(context, ("a",))
        assert "a" in unstable, unstable
        assert unstable["a"] == {"PASSED", "FAILED"}, unstable
        # Con una sola ejecución verde no hay inestabilidad que declarar.
        ledger.write_receipt(receipt_from_json(make_receipt(suite_id="b", receipt_id="rcpt-b").to_json())[0])
        assert _unstable_suites(context, ("b",)) == {}


def test_aat_28_a_partial_receipt_does_not_hide_the_valid_ones():
    with tempfile.TemporaryDirectory(prefix="th-aat-") as tmp:
        ledger = Ledger(Path(tmp) / "state")
        good = make_receipt(receipt_id="rcpt-bueno", suite_id="a")
        ledger.write_receipt(good)
        broken = ledger.receipt_path("rcpt-roto")
        broken.write_text('{"schema_version": "th.receipt/v1", "receipt_id": "rcpt-roto"', encoding="utf-8")

        read_back, problem = ledger.read_receipt("rcpt-roto")
        assert read_back is None and "corrupto" in problem, problem
        index = ledger.rebuild_index()
        assert index["by_suite"]["a"] == ["rcpt-bueno"], index["by_suite"]
        assert all("rcpt-roto" != rid for ids in index["by_suite"].values() for rid in ids)


def test_aat_30_rc_zero_with_zero_tests_is_not_a_pass():
    vacuous = make_receipt(executed=0, passed=0, discovered=0)
    assert vacuous.is_well_formed_pass() is False
    verdict = _aggregate(receipts={"a": vacuous})
    assert verdict.verdict == "UNSATISFIED", verdict.to_json()
    assert verdict.gaps, verdict


def _real_graph(repo: Path):
    from adapters.cargo_workspace import load_graph

    return load_graph(repo, offline=True)


def _toolchain(repo: Path):
    from harness.planner import ToolchainInfo

    return ToolchainInfo.read(repo)
