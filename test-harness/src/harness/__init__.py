"""Núcleo del test harness (puro) más la orquestación.

`domain`, `policy`, `validity` y `gate` no tocan el mundo: son funciones y datos
con serialización canónica, y por eso se pueden probar sin repositorio. La
orquestación (`planner`, `orchestrator`, `cli`) usa los adaptadores de
`test-harness/adapters/`.
"""

from __future__ import annotations

__version__ = "0.1.0"

from .domain import (  # noqa: F401  (API pública del paquete)
    ExecutionPlan,
    GateVerdict,
    Receipt,
    ReuseDecision,
    canonical_json,
    digest_of,
)
from .policy import POLICY_VERSION  # noqa: F401

__all__ = [
    "ExecutionPlan",
    "GateVerdict",
    "POLICY_VERSION",
    "Receipt",
    "ReuseDecision",
    "canonical_json",
    "digest_of",
    "__version__",
]
