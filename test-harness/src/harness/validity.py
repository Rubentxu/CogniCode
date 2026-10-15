"""Huellas de entrada y evaluación de reutilización (TH-S02).

Dos funciones puras y una regla: la huella describe **entradas**, no resultados, y
la decisión de reutilizar es un cálculo de hoy que compara la huella esperada con
la del recibo. Sin reloj, sin red, sin git.

Un `PASSED` cuyo recibo no se puede verificar no se degrada a "válido": se
responde `Undetermined`, y `Undetermined` no certifica.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .domain import Receipt, ReuseDecision, digest_of
from .policy import POLICY_VERSION

# Versión del esquema de huella. Cambiarla invalida toda la evidencia anterior,
# que es exactamente lo que se quiere cuando cambia *qué* se considera entrada.
FINGERPRINT_VERSION = "th-fingerprint/2"


@dataclass(frozen=True)
class FingerprintInputs:
    """Todo lo que puede cambiar el resultado de una suite, menos el código.

    Lo que **no** entra: mtime, rutas absolutas del checkout, el nombre del
    usuario, el PID. La huella es equivalencia de entradas, no una promesa de que
    no haya flakiness (TH-S02 lo dice explícitamente).
    """

    suite_id: str
    argv: tuple[str, ...]
    features: tuple[str, ...]
    target_triple: str
    runner_name: str
    runner_version: str
    rustc_version: str
    cargo_lock_digest: str
    toolchain_digest: str
    closure_digest: str
    expected_min_cases: int
    policy_version: str = POLICY_VERSION

    def canonical(self) -> dict[str, Any]:
        return {
            "fingerprint_version": FINGERPRINT_VERSION,
            "suite_id": self.suite_id,
            "argv": list(self.argv),
            "features": list(self.features),
            "target_triple": self.target_triple,
            "runner_name": self.runner_name,
            "runner_version": self.runner_version,
            "rustc_version": self.rustc_version,
            "cargo_lock_digest": self.cargo_lock_digest,
            "toolchain_digest": self.toolchain_digest,
            "closure_digest": self.closure_digest,
            "expected_min_cases": self.expected_min_cases,
            "policy_version": self.policy_version,
        }

    def digest(self) -> str:
        return digest_of(self.canonical())


def evaluate_reuse(
    receipt: Receipt | None,
    expected: FingerprintInputs,
    *,
    lane_reuse_allowed: bool,
    lane_reason: str,
    freshness: str = "reusable",
    external_state: bool = False,
    integrity_ok: bool = True,
) -> ReuseDecision:
    """Decide qué hacer con una suite, en el orden que fija TH-S02.

    El orden importa: un recibo corrupto se detecta *antes* de mirar la huella,
    un recibo no terminal nunca se reutiliza, y la política de lane se evalúa al
    final porque es la que puede decir "ejecuta aunque todo lo demás cuadre".
    """
    if receipt is None:
        return ReuseDecision("NotReusable", "no hay recibo previo para esta suite")
    if not integrity_ok:
        return ReuseDecision(
            "Undetermined",
            f"el recibo {receipt.receipt_id} no supera la verificación de integridad "
            "(dígito o forma inválida): no es evidencia",
            receipt.receipt_id,
        )
    if not receipt.is_terminal():
        return ReuseDecision(
            "NotReusable",
            f"el recibo {receipt.receipt_id} está en estado {receipt.status}: sin evento "
            "terminal no hay PASS, sólo un proceso que dejó de escribir",
            receipt.receipt_id,
        )
    if receipt.status != "PASSED":
        return ReuseDecision(
            "NotReusable",
            f"el recibo {receipt.receipt_id} terminó en {receipt.status}, no en PASSED",
            receipt.receipt_id,
        )
    if not receipt.is_well_formed_pass():
        return ReuseDecision(
            "Undetermined",
            f"el recibo {receipt.receipt_id} dice PASSED pero no cuadra "
            f"(exit={receipt.exit_code}, ejecutados={receipt.executed_cases}, "
            f"pasados={receipt.passed_cases}, fallidos={receipt.failed_cases}): recibo roto",
            receipt.receipt_id,
        )
    if receipt.suite_id != expected.suite_id:
        return ReuseDecision(
            "Invalidated",
            f"el recibo es de la suite {receipt.suite_id} y se evalúa {expected.suite_id}",
            receipt.receipt_id,
        )
    if tuple(receipt.runner.argv) != tuple(expected.argv):
        return ReuseDecision(
            "Invalidated",
            "el argv del recibo no es el de la obligación de hoy",
            receipt.receipt_id,
        )
    if tuple(receipt.runner.features) != tuple(expected.features):
        return ReuseDecision(
            "Invalidated",
            "el perfil de features cambió desde el recibo",
            receipt.receipt_id,
        )
    if receipt.runner.target_triple != expected.target_triple:
        return ReuseDecision(
            "Invalidated",
            "el target triple cambió desde el recibo",
            receipt.receipt_id,
        )
    if receipt.input_fingerprint != expected.digest():
        return ReuseDecision(
            "Invalidated",
            "la huella de entradas cambió (código, manifest, lock, toolchain, fixtures o política)",
            receipt.receipt_id,
        )
    if freshness == "always-fresh":
        return ReuseDecision(
            "Execute",
            "la suite está declarada siempre-fresca (estado externo o no determinismo)",
            receipt.receipt_id,
        )
    if external_state:
        return ReuseDecision(
            "Execute",
            "la suite depende de estado externo no capturado en la huella",
            receipt.receipt_id,
        )
    if not lane_reuse_allowed:
        return ReuseDecision("Execute", lane_reason, receipt.receipt_id)
    return ReuseDecision(
        "Reusable",
        f"recibo válido y huella idéntica; reutilizable en esta lane ({lane_reason})",
        receipt.receipt_id,
    )
