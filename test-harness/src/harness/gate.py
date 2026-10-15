"""Agregación de obligaciones con evidencia (th.gate/v1).

`SATISFIED` no es "no hubo fallos": es "cada obligación tiene exactamente una
prueba pertinente y válida, y no queda ningún hueco". Un hueco bloquea, y un
hueco declarado es mejor que un verde inventado, así que la agregación devuelve
`gaps` con la razón de cada obligación sin cubrir.
"""

from __future__ import annotations

from dataclasses import dataclass

from .domain import GateVerdict, PlanAction, Receipt
from .inventory import check_receipt_against_inventory
from .policy import POLICY_VERSION

PROOF_KINDS = ("EXECUTE", "REUSE")


@dataclass(frozen=True)
class Coverage:
    """Una obligación cubierta: qué la cubre y con qué huella."""

    suite_id: str
    receipt_id: str
    fingerprint: str
    how: str  # EXECUTE (recibo de esta corrida) | REUSE (recibo anterior válido)


def aggregate(
    *,
    lane: str,
    repo_id: str,
    source_fingerprint: str,
    required: tuple[str, ...],
    actions: tuple[PlanAction, ...],
    receipts: dict[str, Receipt],
    coverage: tuple[Coverage, ...] = (),
    minimums: dict[str, int] | None = None,
    unknown_reasons: tuple[str, ...] = (),
    candidate_artifact_digest: str | None = None,
) -> GateVerdict:
    """Construye el veredicto a partir del plan y de los recibos disponibles.

    Reglas, en orden de dureza:
      1. Una obligación `BLOCK` no puede estar cubierta: produce hueco.
      2. Una obligación requiere prueba con recibo **PASSED** bien formado; un
         recibo en cualquier otro estado no es prueba, es un hueco con nombre.
      3. Si el plan quedó `UNKNOWN_INPUT`, el veredicto es `INCONCLUSIVE` aunque
         todo lo demás esté verde: no se sabe si faltan obligaciones.
    """
    by_suite_receipt = receipts
    minimums = minimums or {}
    proofs: list[dict[str, str]] = []
    gaps: list[dict[str, str]] = []

    blocked = {a.suite_id: a.reason for a in actions if a.action == "BLOCK"}
    planned = {a.suite_id: a for a in actions}

    for suite_id in required:
        if suite_id in blocked:
            gaps.append({"suite_id": suite_id, "reason": f"BLOCK: {blocked[suite_id]}"})
            continue
        action = planned.get(suite_id)
        if action is None:
            gaps.append(
                {
                    "suite_id": suite_id,
                    "reason": "el plan no contiene ninguna acción para esta obligación",
                }
            )
            continue
        if action.action not in PROOF_KINDS:
            gaps.append(
                {
                    "suite_id": suite_id,
                    "reason": f"acción {action.action} no produce prueba",
                }
            )
            continue
        receipt = by_suite_receipt.get(suite_id)
        if receipt is None:
            gaps.append(
                {
                    "suite_id": suite_id,
                    "reason": f"acción {action.action} sin recibo que la respalde",
                }
            )
            continue
        if not receipt.is_well_formed_pass():
            gaps.append(
                {
                    "suite_id": suite_id,
                    "reason": (
                        f"el recibo {receipt.receipt_id} no es un PASS bien formado "
                        f"(status={receipt.status}, exit={receipt.exit_code})"
                    ),
                }
            )
            continue
        minimum_problem = check_receipt_against_inventory(receipt, minimums.get(suite_id, 0))
        if minimum_problem:
            gaps.append({"suite_id": suite_id, "reason": minimum_problem})
            continue
        proofs.append(
            {
                "suite_id": suite_id,
                "receipt_id": receipt.receipt_id,
                "fingerprint": receipt.input_fingerprint,
            }
        )

    if unknown_reasons:
        verdict = "INCONCLUSIVE"
    elif gaps:
        verdict = "UNSATISFIED"
    elif len(proofs) == len(required):
        verdict = "SATISFIED"
    else:
        verdict = "UNSATISFIED"

    return GateVerdict(
        verdict=verdict,
        lane=lane,
        repo_id=repo_id,
        source_fingerprint=source_fingerprint,
        requirements=tuple(required),
        proofs=tuple(proofs),
        gaps=tuple(gaps),
        policy_version=POLICY_VERSION,
        candidate_artifact_digest=candidate_artifact_digest,
    )
