"""Núcleo puro: huellas, decisión de reutilización y agregación del gate.

Aquí se prueban las reglas que un accidente convertiría en verde falso: recibo
roto, recibo no terminal, PASS vacuo, hueco declarado y veredicto indeterminado.
"""

from __future__ import annotations

# `harness_support` va primero, y no es estilo: es quien pone
# `test-harness/src` y `test-harness` en sys.path. Importar `adapters` o
# `harness` antes sólo funciona si otro módulo del mismo proceso ya lo hizo,
# así que el test pasaba en la suite completa y fallaba al correr solo.
from harness_support import CORE_SUITE, make_fingerprint, make_receipt

from harness.domain import PlanAction, digest_of
from harness.gate import Coverage, aggregate
from harness.policy import POLICY_VERSION, reuse_allowed
from harness.validity import FINGERPRINT_VERSION, evaluate_reuse
from harness_support import CORE_SUITE, make_fingerprint, make_receipt


def test_fingerprint_is_stable_and_input_sensitive():
    base = make_fingerprint()
    assert base.digest() == make_fingerprint().digest()
    assert base.digest() != make_fingerprint(closure_digest="0" * 64).digest()
    assert base.digest() != make_fingerprint(cargo_lock_digest="1" * 64).digest()
    assert base.digest() != make_fingerprint(features=("ladybug",)).digest()
    assert base.digest() != make_fingerprint(policy_version="otra").digest()
    assert base.canonical()["fingerprint_version"] == FINGERPRINT_VERSION


def test_reuse_requires_identical_fingerprint():
    expected = make_fingerprint()
    receipt = make_receipt(fingerprint=expected.digest())
    ok = evaluate_reuse(receipt, expected, lane_reuse_allowed=True, lane_reason="change")
    assert ok.kind == "Reusable", ok.reason
    assert ok.receipt_id == receipt.receipt_id

    stale = evaluate_reuse(
        make_receipt(fingerprint="0" * 64),
        expected,
        lane_reuse_allowed=True,
        lane_reason="change",
    )
    assert stale.kind == "Invalidated", stale.reason
    assert "huella" in stale.reason


def test_reuse_rejects_non_terminal_and_broken_receipts():
    expected = make_fingerprint()
    running = make_receipt(status="RUNNING", fingerprint=expected.digest(), exit_code=-1)
    decision = evaluate_reuse(running, expected, lane_reuse_allowed=True, lane_reason="change")
    assert decision.kind == "NotReusable", decision.reason
    assert "terminal" in decision.reason

    broken = make_receipt(status="PASSED", exit_code=0, failed=2, passed=1, executed=3)
    decision = evaluate_reuse(broken, expected, lane_reuse_allowed=True, lane_reason="change")
    assert decision.kind == "Undetermined", decision.reason

    unverified = evaluate_reuse(
        make_receipt(fingerprint=expected.digest()),
        expected,
        lane_reuse_allowed=True,
        lane_reason="change",
        integrity_ok=False,
    )
    assert unverified.kind == "Undetermined", unverified.reason


def test_reuse_rejects_vacuous_pass():
    """Una suite que no ejecutó ningún caso no es un verde: es un vacío."""
    vacuous = make_receipt(executed=0, passed=0, discovered=0)
    assert vacuous.is_well_formed_pass() is False
    decision = evaluate_reuse(
        vacuous, make_fingerprint(), lane_reuse_allowed=True, lane_reason="change"
    )
    assert decision.kind == "Undetermined", decision.reason


def test_reuse_executes_when_lane_forbids_it():
    expected = make_fingerprint()
    receipt = make_receipt(fingerprint=expected.digest())
    decision = evaluate_reuse(
        receipt,
        expected,
        lane_reuse_allowed=False,
        lane_reason="certification exige ejecución fresca",
    )
    assert decision.kind == "Execute", decision.reason


def test_policy_never_reuses_outside_change_by_default():
    assert reuse_allowed("change")[0] is True
    assert reuse_allowed("merge")[0] is False
    assert reuse_allowed("merge", allow_reuse_flag=True)[0] is True
    for lane in ("integration", "certification", "release-candidate", "release"):
        assert reuse_allowed(lane, allow_reuse_flag=True)[0] is False, lane


def test_gate_is_satisfied_only_with_one_valid_proof_per_requirement():
    actions = (
        PlanAction("a", "EXECUTE", "cambió el paquete"),
        PlanAction("b", "REUSE", "recibo válido", receipt_id="rcpt-b"),
    )
    receipts = {
        "a": make_receipt(suite_id="a", fingerprint="a" * 64),
        "b": make_receipt(suite_id="b", fingerprint="b" * 64),
    }
    coverage = (
        Coverage("a", "rcpt-test", "a" * 64, "EXECUTE"),
        Coverage("b", "rcpt-test", "b" * 64, "REUSE"),
    )
    verdict = aggregate(
        lane="change",
        repo_id="demo",
        source_fingerprint="c" * 64,
        required=("a", "b"),
        actions=actions,
        receipts=receipts,
        coverage=coverage,
    )
    assert verdict.verdict == "SATISFIED", verdict.to_json()
    assert verdict.gaps == ()
    assert {p["suite_id"] for p in verdict.proofs} == {"a", "b"}
    assert verdict.policy_version == POLICY_VERSION
    assert verdict.exit_code() == 0


def test_gate_reports_gaps_for_missing_failed_or_blocked():
    actions = (
        PlanAction("a", "EXECUTE", "x"),
        PlanAction("b", "EXECUTE", "y"),
        PlanAction("c", "BLOCK", "requiere sandbox"),
    )
    receipts = {
        "a": make_receipt(suite_id="a"),
        "b": make_receipt(
            suite_id="b", status="FAILED", exit_code=101, passed=1, failed=1, executed=2
        ),
    }
    verdict = aggregate(
        lane="merge",
        repo_id="demo",
        source_fingerprint="c" * 64,
        required=("a", "b", "c", "d"),
        actions=actions,
        receipts=receipts,
    )
    assert verdict.verdict == "UNSATISFIED", verdict.to_json()
    gaps = {gap["suite_id"]: gap["reason"] for gap in verdict.gaps}
    assert set(gaps) == {"b", "c", "d"}, gaps
    assert "PASS bien formado" in gaps["b"], gaps["b"]
    assert "BLOCK" in gaps["c"], gaps["c"]
    assert verdict.exit_code() == 1


def test_gate_is_inconclusive_when_the_input_is_unknown():
    verdict = aggregate(
        lane="change",
        repo_id="demo",
        source_fingerprint="c" * 64,
        required=("a",),
        actions=(PlanAction("a", "EXECUTE", "x"),),
        receipts={"a": make_receipt(suite_id="a")},
        unknown_reasons=("no se pudo calcular el merge-base",),
    )
    assert verdict.verdict == "INCONCLUSIVE", verdict.to_json()
    assert verdict.exit_code() == 3


def test_gate_does_not_accept_a_reuse_without_receipt():
    verdict = aggregate(
        lane="change",
        repo_id="demo",
        source_fingerprint="c" * 64,
        required=("a",),
        actions=(PlanAction("a", "REUSE", "recibo válido", receipt_id="rcpt-a"),),
        receipts={},
    )
    assert verdict.verdict == "UNSATISFIED", verdict.to_json()
    assert "sin recibo" in verdict.gaps[0]["reason"]


def test_canonical_json_is_order_independent():
    assert digest_of({"b": 1, "a": [1, 2]}) == digest_of({"a": [1, 2], "b": 1})


def test_the_demo_suite_id_is_the_one_the_pipeline_declares():
    """Guardia barata: si el id cambia, los tests de selección comparan otra cosa."""
    assert CORE_SUITE == "demo-core:lib#-"
