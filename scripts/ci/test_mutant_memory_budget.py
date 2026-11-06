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
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RUNNER = REPO / "scripts/mem-capped-runner.sh"
GATE = REPO / "scripts/test-gate.sh"

# The owner of the budget: one file, so the numbers that decide how much
# memory the mutation step may ask for have one place to live.
BUDGET_OWNER = REPO / "scripts/mutants-budget.sh"

# The route a person takes when they want a look at one file: a focal
# mutation run. It is the shape of run that produced the OOM of
# 2026-11-05, because it had no ceiling at all.
FOCAL = REPO / "scripts/mutate-file.sh"

AGENTS_MD = REPO / "AGENTS.md"

# A file of the workspace, used by the plan: the plan does not mutate
# anything, so the path only has to be a plausible one.
A_FILE = "crates/cognicode-core/src/lib.rs"

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


def owner_kib(text: str, name: str) -> int | None:
    """A number the owner of the budget declares, in KiB."""
    match = re.search(rf"^\s*{name}=(\d+)", text, re.MULTILINE)
    return int(match.group(1)) if match else None


def focal_plan(jobs: str | None = None) -> dict[str, str] | None:
    """What the focal route would run, as the plan it prints.

    The plan is the route's own answer, not a reading of its source: a
    contract that greps a script can be satisfied by a comment. The plan
    is what the route is about to do.
    """
    args = [str(FOCAL), "--plan", A_FILE]
    if jobs is not None:
        args += ["--jobs", jobs]
    try:
        result = subprocess.run(args, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    plan: dict[str, str] = {}
    for line in result.stdout.splitlines():
        if "=" in line:
            key, _, value = line.partition("=")
            plan[key.strip()] = value.strip()
    return plan


def test_the_focal_mutation_route_carries_the_ceiling() -> None:
    """A focal run asks the machine for the same budget the gate does.

    The ceiling is what keeps a runaway mutant from taking the machine
    with it. Measured 2026-11-05 at 19:40: the kernel's OOM killer
    killed a `cognicode-core` test binary of 37 GB resident and, in the
    same sweep, `coredns`, a container's JVM and two processes of
    another container — none of them this repository's.
    """
    check(
        FOCAL.is_file(),
        "scripts/mutate-file.sh no existe: la corrida focal de mutación no "
        "lleva techo, y es la forma de corrida que produjo el OOM del "
        "2026-11-05",
    )
    if not FOCAL.is_file():
        return

    plan = focal_plan()
    check(
        plan is not None,
        "scripts/mutate-file.sh --plan no devuelve un plan: el contrato no "
        "puede comprobar lo que la ruta haría",
    )
    if plan is None:
        return

    ceiling = int(plan.get("ceiling_kib", "0"))
    jobs = int(plan.get("jobs", "0"))
    budget = int(plan.get("budget_kib", "0"))
    check(ceiling > 0, f"la ruta focal no dice qué techo usa: {plan}")
    check(jobs > 0, f"la ruta focal no dice cuántos trabajos lanza: {plan}")
    check(
        budget > 0,
        f"la ruta focal no declara el presupuesto que se respeta: {plan}",
    )
    if ceiling and jobs and budget:
        check(
            ceiling * jobs <= budget,
            f"la ruta focal pide {ceiling * jobs // 1024} MiB a la vez "
            f"(techo {ceiling // 1024} MiB x {jobs} trabajos) y el "
            f"presupuesto es {budget // 1024} MiB: la máquina se queda sin "
            f"memoria y los mutantes que los tests matan se reportan como "
            f"tiempos agotados",
        )
    check(
        plan.get("runner", "").endswith("mem-capped-runner.sh"),
        f"la ruta focal no ejecuta el binario de test bajo el runner con "
        f"techo: {plan}",
    )
    check(
        plan.get("gitignore") == "true",
        f"la ruta focal copia el árbol entero: son 6,6 GB de copia por "
        f"mutante: {plan}",
    )
    check(
        plan.get("target_dir", "").endswith("mutants-focal"),
        f"la ruta focal no tiene directorio de destino propio: el binario "
        f"del último mutante se queda en la caché compartida y la suite "
        f"posterior ve fallos fantasma (AGENTS.md, regla 8): {plan}",
    )


def test_the_focal_route_caps_its_jobs_at_the_budget() -> None:
    """Asking for more workers than the budget allows does not buy them.

    The jobs are derived from the budget, not taken from the command
    line: the person who asks for eight workers on a machine with less
    free memory than that is the one who pays.
    """
    if not FOCAL.is_file():
        return
    plan = focal_plan(jobs="64")
    check(
        plan is not None,
        "scripts/mutate-file.sh --plan no devuelve un plan: no se puede "
        "comprobar si acota los trabajos al presupuesto",
    )
    if plan is None:
        return

    ceiling = int(plan.get("ceiling_kib", "0"))
    jobs = int(plan.get("jobs", "0"))
    budget = int(plan.get("budget_kib", "0"))
    check(
        jobs < 64,
        f"la ruta focal obedece los 64 trabajos que se le piden: {plan}",
    )
    if ceiling and jobs and budget:
        check(
            ceiling * jobs <= budget,
            f"la ruta focal pide {ceiling * jobs // 1024} MiB a la vez y el "
            f"presupuesto es {budget // 1024} MiB: {plan}",
        )


def test_one_owner_for_the_budget() -> None:
    """The numbers that decide the memory live in one file.

    Two scripts naming the same budget are two sources of truth, and the
    second one rots: the step that reads the stale one asks for more
    memory than the machine has, and the certificate says the gate fits.
    """
    check(
        BUDGET_OWNER.is_file(),
        "scripts/mutants-budget.sh no existe: el presupuesto y los techos "
        "no tienen un dueño único",
    )
    if not BUDGET_OWNER.is_file():
        return

    text = BUDGET_OWNER.read_text(encoding="utf-8")
    budget = owner_kib(text, "MUTANTS_BUDGET_KIB")
    gate_ceiling = owner_kib(text, "MUTANTS_GATE_VMEM_KIB")
    focal_ceiling = owner_kib(text, "MUTANTS_FOCAL_VMEM_KIB")
    check(
        budget is not None,
        "scripts/mutants-budget.sh no declara MUTANTS_BUDGET_KIB: el "
        "presupuesto de memoria de las corridas de mutación no está escrito",
    )
    check(
        gate_ceiling is not None,
        "scripts/mutants-budget.sh no declara MUTANTS_GATE_VMEM_KIB: el "
        "techo del paso de mutantes del gate no está escrito",
    )
    check(
        focal_ceiling is not None,
        "scripts/mutants-budget.sh no declara MUTANTS_FOCAL_VMEM_KIB: el "
        "techo de la corrida focal no está escrito",
    )
    if budget is None or gate_ceiling is None or focal_ceiling is None:
        return

    check(
        budget == BUDGET_KIB,
        f"el presupuesto del dueño ({budget // 1024} MiB) no es el que el "
        f"contrato conoce ({BUDGET_KIB // 1024} MiB): el contrato vigila "
        f"un número que nadie usa",
    )
    check(
        gate_ceiling <= budget,
        f"el techo del gate ({gate_ceiling // 1024} MiB) es mayor que el "
        f"presupuesto ({budget // 1024} MiB)",
    )
    check(
        focal_ceiling <= budget,
        f"el techo de la corrida focal ({focal_ceiling // 1024} MiB) es "
        f"mayor que el presupuesto ({budget // 1024} MiB)",
    )

    gate_text = GATE.read_text(encoding="utf-8")
    check(
        "MUTANTS_GATE_VMEM_KIB" in gate_text,
        "scripts/test-gate.sh no carga el techo del dueño: el valor del "
        "paso de mutantes del gate vive en dos sitios",
    )


def test_the_detector_sees_a_planted_runaway_route() -> None:
    # A detector that never fires is worse than no detector: it says the
    # route fits when it does not.
    check(
        budget_kib(8 * 1024 * 1024, 4) > BUDGET_KIB,
        "el detector no ve un techo que se pasa con cuatro trabajos",
    )
    check(
        budget_kib(4 * 1024 * 1024, 4) <= BUDGET_KIB,
        "el detector ve un techo que cabe como un problema",
    )


def test_no_document_recommends_a_mutation_run_without_a_ceiling() -> None:
    """The command that has no ceiling is the one people copy.

    `AGENTS.md` is the entry point every agent reads, and it recommends
    a focal mutation run: the run that has no ceiling, no target
    directory of its own and no budget. The route that carries all three
    is the one it has to name.
    """
    text = AGENTS_MD.read_text(encoding="utf-8")
    check(
        "cargo mutants --file" not in text,
        "AGENTS.md sigue recomendando una corrida de mutación sin techo "
        "(`cargo mutants --file`): es la forma de corrida que produjo el "
        "OOM del 2026-11-05",
    )
    check(
        "mutate-file.sh" in text,
        "AGENTS.md no nombra la ruta focal de mutación, que es la que lleva "
        "el techo, el directorio de destino propio y el presupuesto",
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
