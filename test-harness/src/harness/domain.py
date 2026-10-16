"""Tipos del dominio del test harness.

Puro: sin I/O, sin git, sin cargo, sin rutas del sistema. Los adaptadores viven
en `test-harness/adapters/` y hablan con el mundo; aquí solo hay datos con
serialización canónica, porque de ella dependen las huellas y los contratos JSON
(`contracts/*.schema.json`).

Regla que gobierna todo el módulo: un estado que no sea `PASSED` **no** es
verde. `ReuseDecision` es una evaluación de hoy sobre un recibo que fue un hecho
ayer, y las dos cosas no se mezclan.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Any

SCHEMA_PLAN = "th.plan/v1"
SCHEMA_RECEIPT = "th.receipt/v1"
SCHEMA_GATE = "th.gate/v1"

LANES = (
    "change",
    "merge",
    "integration",
    "certification",
    "release-candidate",
    "release",
)

# Estados terminales de un recibo. `RUNNING` no es terminal y nunca cuenta como
# PASS: es el estado que deja un proceso interrumpido.
RECEIPT_STATUSES = (
    "PASSED",
    "FAILED",
    "CANCELLED",
    "TIMED_OUT",
    "INFRA_ERROR",
    "MISSING",
    "QUARANTINED",
    "SKIPPED",
    "RUNNING",
)

PLAN_STATUSES = ("READY", "BLOCKED", "UNKNOWN_INPUT")
ACTION_KINDS = ("EXECUTE", "REUSE", "BLOCK")
VERDICTS = ("SATISFIED", "UNSATISFIED", "INCONCLUSIVE")


def canonical_json(payload: Any) -> str:
    """Serialización canónica: claves ordenadas, sin espacios, UTF-8.

    Las huellas se calculan sobre esto y no sobre `json.dumps` por defecto: el
    orden de claves de un diccionario de Python depende de cómo se construyó, y
    dos procesos que describan la misma entrada tienen que producir la misma
    huella o la invalidación se vuelve ruido.
    """
    return json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def digest_of(payload: Any) -> str:
    return hashlib.sha256(canonical_json(payload).encode("utf-8")).hexdigest()


def digest_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


@dataclass(frozen=True)
class SuiteRef:
    """Una obligación ejecutable: paquete y target de Cargo, con su argv real."""

    suite_id: str
    package: str
    target: str
    argv: tuple[str, ...]
    lanes: tuple[str, ...]
    packages_in_closure: tuple[str, ...] = ()
    freshness: str = "reusable"  # reusable | always-fresh
    external_state: bool = False

    def to_json(self) -> dict[str, Any]:
        return {
            "suite_id": self.suite_id,
            "package": self.package,
            "target": self.target,
            "argv": list(self.argv),
            "lanes": list(self.lanes),
            "packages_in_closure": list(self.packages_in_closure),
            "freshness": self.freshness,
            "external_state": self.external_state,
        }


@dataclass(frozen=True)
class PlanAction:
    suite_id: str
    action: str
    reason: str
    dependencies: tuple[str, ...] = ()
    receipt_id: str | None = None

    def to_json(self) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "suite_id": self.suite_id,
            "action": self.action,
            "reason": self.reason,
            "dependencies": list(self.dependencies),
        }
        if self.receipt_id:
            payload["receipt_id"] = self.receipt_id
        return payload


@dataclass
class ExecutionPlan:
    plan_id: str
    repo_id: str
    source_fingerprint: str
    lane: str
    status: str
    required_suites: tuple[str, ...]
    actions: tuple[PlanAction, ...]
    policy_version: str
    unknown_reasons: tuple[str, ...] = ()
    created_at: str = ""
    # Perfil de selección con el que se construyó el plan: 'gate' (todo) o 'dev'
    # (bucle local, sólo lane `change`). Va en el plan y en el esquema para que un
    # consumidor no pueda confundir un plan recortado a propósito con el del gate.
    profile: str = "gate"

    def to_json(self) -> dict[str, Any]:
        return {
            "schema_version": SCHEMA_PLAN,
            "plan_id": self.plan_id,
            "repo_id": self.repo_id,
            "source_fingerprint": self.source_fingerprint,
            "lane": self.lane,
            "status": self.status,
            "required_suites": list(self.required_suites),
            "actions": [a.to_json() for a in self.actions],
            "unknown_reasons": list(self.unknown_reasons),
            "policy_version": self.policy_version,
            "created_at": self.created_at,
            "profile": self.profile,
        }

    def counts(self) -> dict[str, int]:
        out = {"EXECUTE": 0, "REUSE": 0, "BLOCK": 0}
        for action in self.actions:
            out[action.action] = out.get(action.action, 0) + 1
        return out


@dataclass(frozen=True)
class SourceSnapshot:
    git_head: str
    effective_content_digest: str
    dirty: bool
    worktree_id: str

    def to_json(self) -> dict[str, Any]:
        return {
            "git_head": self.git_head,
            "effective_content_digest": self.effective_content_digest,
            "dirty": self.dirty,
            "worktree_id": self.worktree_id,
        }


@dataclass(frozen=True)
class RunnerInfo:
    name: str
    version: str
    argv: tuple[str, ...]
    features: tuple[str, ...]
    target_triple: str

    def to_json(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "version": self.version,
            "argv": list(self.argv),
            "features": list(self.features),
            "target_triple": self.target_triple,
        }


@dataclass
class Receipt:
    receipt_id: str
    run_id: str
    repo_id: str
    suite_id: str
    lane: str
    status: str
    input_fingerprint: str
    source_snapshot: SourceSnapshot
    runner: RunnerInfo
    started_at: str
    finished_at: str
    exit_code: int
    discovered_cases: int
    executed_cases: int
    passed_cases: int
    failed_cases: int
    skipped_cases: int
    log_sha256: str

    def to_json(self) -> dict[str, Any]:
        return {
            "schema_version": SCHEMA_RECEIPT,
            "receipt_id": self.receipt_id,
            "run_id": self.run_id,
            "repo_id": self.repo_id,
            "suite_id": self.suite_id,
            "lane": self.lane,
            "status": self.status,
            "input_fingerprint": self.input_fingerprint,
            "source_snapshot": self.source_snapshot.to_json(),
            "runner": self.runner.to_json(),
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "exit_code": self.exit_code,
            "discovered_cases": self.discovered_cases,
            "executed_cases": self.executed_cases,
            "passed_cases": self.passed_cases,
            "failed_cases": self.failed_cases,
            "skipped_cases": self.skipped_cases,
            "log_sha256": self.log_sha256,
        }

    def is_terminal(self) -> bool:
        return self.status in RECEIPT_STATUSES and self.status != "RUNNING"

    def is_well_formed_pass(self) -> bool:
        """Invariante de PASS que el esquema JSON no puede expresar.

        `PASSED` exige `exit_code=0`, cero fallos, cuenta cuadrando y un log con
        digest al que apuntar. Además, si el runner es Cargo, exige al menos un
        caso ejecutado: una suite que no ejecutó nada (filtro que no casa, feature
        que apaga los tests) no es un verde, es un vacío. Los runners que no
        cuentan casos (scripts) se conforman con exit 0 y evidencia de log.
        """
        if self.status != "PASSED":
            return False
        if self.exit_code != 0 or self.failed_cases != 0:
            return False
        if not self.log_sha256:
            return False
        if self.executed_cases != self.passed_cases + self.failed_cases + self.skipped_cases:
            return False
        if self.runner.name == "cargo-test" and self.executed_cases < 1:
            # Una corrida de doc-tests puede no encontrar ninguno: `cargo test
            # --doc` recorre los ejemplos de la documentación, y un paquete sin
            # ejemplos ejecutables devuelve 0 casos con exit 0. MEDIDO 2026-10-16:
            # `cognicode-cli:doc#-` es exactamente eso, y rechazarlo convertía el
            # plan de la lane change en UNSATISFIED permanente; con fail-fast,
            # además, el run se detenía ahí y las 17 suites siguientes ni se
            # intentaban. El suelo sigue en pie para el resto: una corrida de
            # tests con filtro que no casa sigue siendo un vacío, no un verde.
            return "--doc" in self.runner.argv
        return True

    def digest(self) -> str:
        return digest_of(self.to_json())


@dataclass
class ReuseDecision:
    """Evaluación de hoy sobre un recibo de ayer."""

    kind: str  # Reusable | Execute | Invalidated | Undetermined | NotReusable | Blocked
    reason: str
    receipt_id: str | None = None

    def to_json(self) -> dict[str, Any]:
        payload: dict[str, Any] = {"kind": self.kind, "reason": self.reason}
        if self.receipt_id:
            payload["receipt_id"] = self.receipt_id
        return payload


@dataclass
class Requirement:
    suite_id: str
    required_for_lane: tuple[str, ...] = field(default_factory=tuple)
    expected_min_cases: int = 1


@dataclass
class GateVerdict:
    verdict: str
    lane: str
    repo_id: str
    source_fingerprint: str
    requirements: tuple[str, ...]
    proofs: tuple[dict[str, str], ...]
    gaps: tuple[dict[str, str], ...]
    policy_version: str
    candidate_artifact_digest: str | None = None

    def to_json(self) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "schema_version": SCHEMA_GATE,
            "verdict": self.verdict,
            "lane": self.lane,
            "repo_id": self.repo_id,
            "source_fingerprint": self.source_fingerprint,
            "requirements": list(self.requirements),
            "proofs": [dict(p) for p in self.proofs],
            "gaps": [dict(g) for g in self.gaps],
            "policy_version": self.policy_version,
        }
        if self.candidate_artifact_digest:
            payload["candidate_artifact_digest"] = self.candidate_artifact_digest
        return payload

    def exit_code(self) -> int:
        if self.verdict == "SATISFIED":
            return 0
        if self.verdict == "UNSATISFIED":
            return 1
        return 3
