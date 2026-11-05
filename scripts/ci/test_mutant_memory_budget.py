#!/usr/bin/env python3
"""Contrato: el presupuesto de memoria del paso de mutantes cabe en la máquina.

## Lo que está en juego

El paso de mutantes del gate lanza ocho trabajos a la vez, y cada binario de
test crece hasta el techo que fija `scripts/mem-capped-runner.sh`. El techo se
paga ocho veces a la vez: ocho binarios pidiendo 6 GiB cada uno son 48 GiB de
espacio de direcciones, más de los que la máquina tiene libres (medido
2026-11-05: 94 GiB totales, unos 39 GiB libres).

Cuando la máquina se pasa de su presupuesto de memoria, el daño no lo paga el
mutante que se pasa: lo pagan sus vecinos. Un mutante que los tests matan en
segundos se queda sin presupuesto y se reporta como TIEMPO AGOTADO — un gate
rojo que no dice nada del código—. Medido el 2026-11-05 sobre los 574 mutantes
del alcance: con un techo de 6 GiB, 8 tiempos agotados; con un techo de 2 GiB,
ninguno, y los 574 atrapados.

Este contrato fija el producto de los dos números: techo x trabajos <= 16 GiB.

Run:
    python3 scripts/ci/test_mutant_memory_budget.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RUNNER = REPO / "scripts/mem-capped-runner.sh"
GATE = REPO / "scripts/test-gate.sh"

# The budget the whole step may ask for at the same time. It is a quarter of
# the memory a machine of this class has free, with room for the rest of the
# workspace.
BUDGET_KIB = 16 * 1024 * 1024

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def runner_ceiling_kib(text: str) -> int | None:
    """The default address-space ceiling the runner imposes, in KiB."""
    match = re.search(r"MUTANT_TEST_VMEM_KB:-(\d+)", text)
    return int(match.group(1)) if match else None


def gate_job_cap(text: str) -> int | None:
    """The largest number of mutants the gate runs at the same time."""
    match = re.search(r"-gt\s+(\d+)\s*\]\s*;\s*then\s*\w+JOBS=(\d+)", text)
    return int(match.group(2)) if match else None


def budget_kib(ceiling_kib: int, jobs: int) -> int:
    return ceiling_kib * jobs


def test_the_memory_budget_of_the_mutation_step_fits() -> None:
    runner_text = RUNNER.read_text(encoding="utf-8")
    gate_text = GATE.read_text(encoding="utf-8")

    ceiling = runner_ceiling_kib(runner_text)
    check(
        ceiling is not None,
        "scripts/mem-capped-runner.sh no declara un techo por defecto: el "
        "paso de mutantes no tendría memoria acotada",
    )
    jobs = gate_job_cap(gate_text)
    check(
        jobs is not None,
        "scripts/test-gate.sh no declara un tope de trabajos en paralelo: el "
        "presupuesto de memoria del paso de mutantes no se puede calcular",
    )
    if ceiling is None or jobs is None:
        return

    asked = budget_kib(ceiling, jobs)
    check(
        asked <= BUDGET_KIB,
        f"el paso de mutantes pide {asked // 1024} MiB a la vez "
        f"(techo {ceiling // 1024} MiB x {jobs} trabajos) y el presupuesto es "
        f"{BUDGET_KIB // 1024} MiB: la máquina se queda sin memoria y los "
        f"mutantes que los tests matan se reportan como tiempos agotados",
    )


def test_the_detector_sees_a_planted_oversubscription() -> None:
    # A detector that never fires is worse than no detector: it says the
    # budget fits when it does not.
    check(
        budget_kib(6 * 1024 * 1024, 8) > BUDGET_KIB,
        "el detector no ve un presupuesto que se pasa",
    )
    check(
        budget_kib(2 * 1024 * 1024, 8) <= BUDGET_KIB,
        "el detector ve un presupuesto que cabe como un problema",
    )


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("PASS — el paso de mutantes pide memoria dentro de su presupuesto")
    return 0


if __name__ == "__main__":
    sys.exit(main())
