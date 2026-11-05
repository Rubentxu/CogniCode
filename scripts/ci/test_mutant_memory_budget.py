#!/usr/bin/env python3
"""Contrato: el paso de mutantes del gate mide de uno en uno y cabe en la máquina.

## Lo que está en juego

Dos cosas, y las dos son la misma clase de cosa: **que el veredicto del gate
sea atribuible al mutante que dice ser**.

### 1. El presupuesto de memoria

El paso de mutantes lanza los binarios de test bajo el techo que fija
`scripts/mem-capped-runner.sh`. El techo se paga por cada trabajo a la vez: si
el techo es de 6 GiB y hay ocho trabajos, son 48 GiB de espacio de direcciones,
más de los que la máquina tiene libres (medido 2026-11-05: 94 GiB totales,
unos 39 GiB libres).

Cuando la máquina se pasa de su presupuesto, el daño no lo paga el mutante que
se pasa: lo pagan sus vecinos. Un mutante que los tests matan en segundos se
queda sin presupuesto y se reporta como TIEMPO AGOTADO — un gate rojo que no
dice nada del código—. Medido el 2026-11-05 sobre los 574 mutantes del
alcance: con un techo de 6 GiB, 8 tiempos agotados; con un techo de 2 GiB,
ninguno, y los 574 atrapados.

Este contrato fija el producto de los dos números: techo x trabajos <= 16 GiB.

### 2. La atribución

`cargo-mutants` trabaja sobre UNA copia del árbol y sus trabajadores mutan
esa copia SIN cerrojo (`lab.rs`: apply -> build -> test -> revert). Con más de
un trabajo, escriben sobre el mismo fichero: uno aplica su mutación, otro la
sobrescribe, un tercero la revierte mientras el primero compila. El veredicto
de un mutante deja entonces de ser atribuible a ese mutante.

Medido el 2026-11-05 sobre el alcance del gate: una corrida con ocho
trabajos reportó SIETE supervivientes en `god_nodes.rs`, y los diez mutantes
de ese fichero salen ATRAPADOS medidos de uno en uno. La misma corrida
reportó además un número de mutantes inviables que cambió entre corridas
(1, 2, 4) sobre el mismo árbol.

Un gate que reporta un agujero que no existe es un gate en el que nadie cree,
y un gate que reporta una presa que vino de la mutación de otro es un gate
que no vigila nada. La certificación es POR MUTANTE: para que la certificación
sea atribuible, la medición es en serie.

Por eso este contrato fija el paralelismo por defecto en UN trabajo. La
memoria y la atribución son la misma pregunta — ¿puede este paso decir la
verdad? — y por eso viven en el mismo contrato.

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
    """The number of mutants the gate measures at the same time."""
    match = re.search(r"^\s*MUTANTS_JOBS=(\d+)", text, re.MULTILINE)
    return int(match.group(1)) if match else None


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
        "scripts/test-gate.sh no declara cuántos trabajos lanza a la vez: "
        "el presupuesto de memoria del paso de mutantes no se puede calcular",
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


def jobs_are_attributable(jobs: int) -> bool:
    """Whether a measurement with this many workers at once can be trusted."""
    return jobs == 1


def test_the_mutation_step_measures_one_mutant_at_a_time() -> None:
    """The certificate is per mutant, and per mutant means serial.

    `cargo-mutants` mutates one shared copy of the tree without a lock, so
    two workers at once write to the same file and each one can be measured
    with somebody else's mutation (or with none at all).
    """
    gate_text = GATE.read_text(encoding="utf-8")
    jobs = gate_job_cap(gate_text)
    check(
        jobs is not None,
        "scripts/test-gate.sh no declara cuántos trabajos lanza a la vez: "
        "la atribución del veredicto por mutante no se puede comprobar",
    )
    if jobs is None:
        return
    check(
        jobs_are_attributable(jobs),
        f"el paso de mutantes lanza {jobs} trabajos a la vez y el veredicto "
        f"por mutante deja de ser atribuible: cargo-mutants muta una única "
        f"copia del árbol sin cerrojo, y un trabajo puede medirse con la "
        f"mutación de otro (medido 2026-11-05: siete mutantes de "
        f"god_nodes.rs reportados como supervivientes en una corrida en "
        f"paralelo y los diez atrapados medidos de uno en uno)",
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


def test_the_detector_sees_a_planted_parallel_measurement() -> None:
    # The same, for the attribution: a gate that went back to measuring in
    # parallel has to fail the contract, not pass it quietly.
    check(
        not jobs_are_attributable(8),
        "el detector no ve un paso que mide en paralelo",
    )
    check(
        jobs_are_attributable(1),
        "el detector ve una medición en serie como un problema",
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
    print(
        "PASS — el paso de mutantes pide memoria dentro de su presupuesto "
        "y mide de uno en uno"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
