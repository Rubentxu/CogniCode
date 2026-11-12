#!/usr/bin/env python3
"""Contrato del gate: el radio que mide y el paso de mutantes que corre.

Dos contratos que vigilan la misma cosa, `scripts/test-gate.sh`: que lo que el
gate dice haber medido sea lo que midió.

## Parte 1: el radio del gate (`scripts/gate_scope.py`)


The gate of this repository decides how much it measures with the answer of
one script. That script is therefore part of the harness of the gate, and a
harness that reduces what it measures is measured itself: these checks are the
proof that the radius is the inverse dependency closure, that an undecidable
radius is the WHOLE gate, and that the calendar fires when it says it does.

The fixture is a real cargo workspace with two crates, `alpha` (a leaf) and
`beta` (which depends on `alpha`), because the closure cannot be tested
against a hand-written list of dependencies: the whole point is that cargo
decides who depends on whom.

## Parte 2: el paso de mutantes mide de uno en uno y cabe en la máquina


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
    python3 scripts/ci/test_gate_contracts.py
"""
from __future__ import annotations

import json
import os
import re
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SELECTOR = REPO / "scripts" / "gate_scope.py"
GATE = REPO / "scripts" / "test-gate.sh"
RUNNER = REPO / "scripts/mem-capped-runner.sh"

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


# --- Parte 1: el radio del gate -----------------------------------------------


def _git(repo: Path, *args: str) -> str:
    proc = subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True, timeout=120)
    if proc.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {proc.stderr.strip()}")
    return proc.stdout.strip()


def _identity(key: str) -> str:
    """The identity this repository commits with."""
    proc = subprocess.run(
        ["git", "config", "--get", f"user.{key}"],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=120,
    )
    return proc.stdout.strip()


def _commit(repo: Path, message: str) -> None:
    """A commit in the fixture.

    The identity is the one this machine is configured with, and the message
    is a conventional one: the policy that guards the commits of the operator
    is global, and a fixture that violated it would be measuring the policy
    instead of the selector.
    """
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", message)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _fixture(root: Path) -> tuple[str, str]:
    """A cargo workspace of two crates, with two commits on top of the base.

    Returns (base commit, head commit).
    """
    _write(
        root / "Cargo.toml",
        '[workspace]\nmembers = ["alpha", "beta", "gamma"]\nresolver = "2"\n',
    )
    _write(
        root / "alpha" / "Cargo.toml",
        '[package]\nname = "alpha"\nversion = "0.1.0"\nedition = "2021"\n',
    )
    _write(root / "alpha" / "src" / "lib.rs", "pub fn value() -> u32 {\n    1\n}\n")
    _write(
        root / "beta" / "Cargo.toml",
        '[package]\nname = "beta"\nversion = "0.1.0"\nedition = "2021"\n\n'
        '[dependencies]\nalpha = { path = "../alpha" }\n',
    )
    _write(
        root / "beta" / "src" / "lib.rs",
        "pub fn doubled() -> u32 {\n    alpha::value() * 2\n}\n",
    )
    # A third crate, a leaf that nothing depends on and that depends on
    # nothing: it is OUTSIDE the closure of `alpha`, and it is what tells a
    # narrowed radius from a whole workspace.
    _write(
        root / "gamma" / "Cargo.toml",
        '[package]\nname = "gamma"\nversion = "0.1.0"\nedition = "2021"\n',
    )
    _write(root / "gamma" / "src" / "lib.rs", "pub fn other() -> u32 {\n    7\n}\n")

    _git(root, "init", "-q", "-b", "main")
    # The identity of the operator, copied from this repository: the guard that
    # signs the commits is global, and a fixture without an identity of its own
    # is refused by it before the selector is ever asked.
    _git(root, "config", "user.email", _identity("email"))
    _git(root, "config", "user.name", _identity("name"))
    _git(root, "add", "-A")
    _commit(root, "chore: base")
    base = _git(root, "rev-parse", "HEAD")

    # The batch touches the LEAF. `beta` does not change, but it can break,
    # because it depends on what changed.
    _write(root / "alpha" / "src" / "lib.rs", "pub fn value() -> u32 {\n    2\n}\n")
    _git(root, "add", "-A")
    _commit(root, "chore: touch the leaf")
    # The second commit of the batch does not touch a package: the batch is
    # two commits long, and only the first one touches code.
    _write(root / "NOTES.md", "una nota que no es codigo\n")
    _commit(root, "chore: a note")
    return base, _git(root, "rev-parse", "HEAD")


def _radius(root: Path, *extra: str) -> tuple[dict[str, str], int]:
    """Ask the selector about the fixture and read its answer."""
    proc = subprocess.run(
        [sys.executable, "-I", str(SELECTOR), "--repo-root", str(root), *extra],
        capture_output=True,
        text=True,
        timeout=120,
    )
    answer: dict[str, str] = {}
    for line in proc.stdout.splitlines():
        if "=" in line:
            key, _, value = line.partition("=")
            answer[key] = value
    return answer, proc.returncode


def test_the_radius_is_the_inverse_closure_of_what_the_batch_touched() -> None:
    """The batch touches the leaf; the dependent is in the radius, because it
    can break. A radius that stopped at the touched packages would run the
    tests of a crate that depends on the change, without running the tests of
    the crate that depends on it."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        base, _ = _fixture(root)
        answer, rc = _radius(
            root,
            "--base",
            base,
            "--affordable",
            "alpha",
            "--calendar-crate",
            "alpha",
            "--calendar-every",
            "50",
            "--calendar-marker",
            base,
        )
        check(rc == 0, f"el selector no pudo decidir el radio: {answer.get('reason')}")
        check(
            answer.get("touched") == "alpha",
            f"el lote toco `alpha` y el selector dijo {answer.get('touched')!r}",
        )
        check(
            answer.get("test-scope") == "alpha beta",
            "el radio de test no es el cierre inverso: un crate que depende del "
            f"que cambio quedo fuera ({answer.get('test-scope')!r})",
        )
        check(
            answer.get("test-scope-full") == "false",
            "el radio de test se declaro completo cuando no lo es",
        )


def test_a_change_in_the_dependent_does_not_pull_its_dependency_in() -> None:
    """The batch touches `beta`, the crate that DEPENDS on `alpha`. The radius
    is `beta` and nothing else: a change in a dependent cannot break its
    dependency, and `gamma` is outside the closure for the same reason the
    other probe proves it. This is the direction a selector that guesses by
    path or by name gets wrong — it would pull `alpha` in because the file
    sits next to the change — and it is what makes this validation a probe
    with a KNOWN expected scope, not a restatement of the implementation."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        base, _ = _fixture(root)
        # Discard the fixture's batch (it touches `alpha`): this probe's batch
        # must touch the dependent and only the dependent.
        _git(root, "reset", "-q", "--hard", base)
        _write(
            root / "beta" / "src" / "lib.rs",
            "pub fn doubled() -> u32 {\n    4\n}\n",
        )
        _commit(root, "chore: touch the dependent")
        answer, rc = _radius(
            root,
            "--base",
            base,
            "--affordable",
            "alpha",
            "--calendar-crate",
            "alpha",
            "--calendar-every",
            "50",
            "--calendar-marker",
            base,
        )
        check(rc == 0, f"el selector no pudo decidir el radio: {answer.get('reason')}")
        check(
            answer.get("touched") == "beta",
            f"el lote toco `beta` y el selector dijo {answer.get('touched')!r}",
        )
        check(
            answer.get("test-scope") == "beta",
            "el radio de test arrastro a la dependencia: un cambio en el "
            f"dependiente no puede romper a `alpha`, y el radio dijo "
            f"{answer.get('test-scope')!r}",
        )
        check(
            answer.get("test-scope-full") == "false",
            "el radio de test se declaro completo cuando no lo es",
        )


def test_an_undetermined_radius_is_the_whole_gate() -> None:
    """A base that cannot be resolved is not an empty radius: it is every
    package. An empty radius would mean "nothing to check", and a gate that
    checks nothing and says green is worse than a slow gate."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        _fixture(root)
        answer, rc = _radius(
            root,
            "--base",
            "no-existe-este-commit",
            "--affordable",
            "alpha",
            "--calendar-crate",
            "alpha",
            "--calendar-every",
            "50",
            "--calendar-marker",
            "no-existe-este-commit",
        )
        check(rc != 0, "un radio que no se puede determinar no puede salir con rc=0")
        check(
            answer.get("status") == "undetermined",
            f"un radio indeciso se declaro {answer.get('status')!r}",
        )
        check(
            answer.get("test-scope") == "alpha beta gamma",
            "un radio indeciso no es el gate entero: "
            f"{answer.get('test-scope')!r}",
        )
        check(
            answer.get("test-scope-full") == "true",
            "un radio indeciso no se declara completo",
        )
        check(
            bool(answer.get("reason")),
            "un radio indeciso no dice POR QUE es indeciso",
        )
        check(
            answer.get("mutant-scope") == "alpha",
            "un radio indeciso no midio los mutantes: "
            f"{answer.get('mutant-scope')!r}",
        )


def test_the_calendar_fires_when_the_batch_crosses_its_period() -> None:
    """The turn is a crossing, not a remainder. A batch of two commits with a
    period of two crosses the multiple and fires; the same batch with a
    period of three does not."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        base, _ = _fixture(root)
        fires, rc = _radius(
            root,
            "--base",
            base,
            "--affordable",
            "alpha",
            "--calendar-crate",
            "alpha",
            "--calendar-every",
            "2",
            "--calendar-marker",
            base,
        )
        check(rc == 0, "el calendario no pudo decidir: {}".format(fires.get("reason")))
        check(
            fires.get("calendar") == "fires",
            f"un lote que cruza el multiple del calendario no lo dispara: "
            f"{fires.get('calendar')!r}",
        )
        check(
            fires.get("mutant-scope") == "alpha",
            "el crate del calendario no esta en el radio de mutantes cuando "
            f"su turno toca: {fires.get('mutant-scope')!r}",
        )

        waits, rc = _radius(
            root,
            "--base",
            base,
            "--affordable",
            "alpha",
            "--calendar-crate",
            "alpha",
            "--calendar-every",
            "3",
            "--calendar-marker",
            base,
        )
        check(rc == 0, "el calendario no pudo decidir: {}".format(waits.get("reason")))
        check(
            waits.get("calendar") == "waits",
            f"un lote que NO cruza el multiple dispara el calendario: "
            f"{waits.get('calendar')!r}",
        )


def test_a_waiting_calendar_says_how_far_its_next_turn_is() -> None:
    """`waits` con una cuenta mayor que el periodo no es un turno vencido: el
    ultimo cruce ya paso y el siguiente esta por delante. El radio lo dice, para
    que el log no deje leer «85 fusiones, cada 50» como un hueco sin acotar."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        base, _ = _fixture(root)
        for every, expected in (("3", "1"), ("50", "48")):
            answer, rc = _radius(
                root,
                "--base",
                base,
                "--affordable",
                "alpha",
                "--calendar-crate",
                "alpha",
                "--calendar-every",
                every,
                "--calendar-marker",
                base,
            )
            check(rc == 0, "el calendario no pudo decidir: {}".format(answer.get("reason")))
            check(
                answer.get("calendar") == "waits",
                f"con periodo {every} el lote no deberia disparar: {answer.get('calendar')!r}",
            )
            check(
                answer.get("calendar-next") == expected,
                f"con periodo {every} faltan {expected} fusiones para el proximo turno, "
                f"el radio dice {answer.get('calendar-next')!r}",
            )


def test_an_uncertain_calendar_measures_the_crate_now() -> None:
    """The calendar failing to count is not a calendar that never fires: the
    crate is measured now. An uncertain measurement must never mean less
    measurement."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        base, head = _fixture(root)
        answer, rc = _radius(
            root,
            "--base",
            base,
            "--affordable",
            "alpha",
            "--calendar-crate",
            "alpha",
            "--calendar-every",
            "50",
            "--calendar-marker",
            head,
        )
        check(rc == 0, "el selector no pudo decidir el radio: {}".format(answer.get("reason")))
        check(
            answer.get("calendar") == "unknown",
            f"un calendario que no se puede contar no se declara "
            f"{answer.get('calendar')!r}",
        )
        check(
            answer.get("mutant-scope") == "alpha",
            "un calendario indeciso dejo de medir el crate: "
            f"{answer.get('mutant-scope')!r}",
        )
        check(
            bool(answer.get("reason")),
            "un calendario indeciso no dice POR QUE es indeciso",
        )


def test_a_batch_that_touched_no_package_leaves_the_whole_workspace_standing() -> None:
    """A test that reads a file outside its crate has no dependency edge to be
    selected by, so a batch that touched no package cannot narrow the radius."""
    with tempfile.TemporaryDirectory(prefix="gate-scope-") as tmp:
        root = Path(tmp)
        base, _ = _fixture(root)
        head = _git(root, "rev-parse", "HEAD")
        _git(root, "checkout", "-q", base)
        _write(root / "NOTES.md", "una nota que no es codigo\n")
        _git(root, "add", "-A")
        _commit(root, "chore: solo notas")
        answer, rc = _radius(root, "--base", base, "--affordable", "alpha")
        check(rc == 0, "el selector no pudo decidir el radio: {}".format(answer.get("reason")))
        check(
            answer.get("touched") == "",
            f"un lote que no toco ningun paquete dijo {answer.get('touched')!r}",
        )
        check(
            answer.get("test-scope-full") == "true",
            "un lote sin paquetes tocados acoto el radio: "
            f"{answer.get('test-scope')!r}",
        )
        _git(root, "checkout", "-q", head)


def test_the_gate_script_declares_the_calendar_it_runs() -> None:
    """The gate and this contract are the same promise in two places. The
    numbers that decide when the crate of the graph logic is measured have to
    be written in the gate, with the period as a number and the marker as a
    commit."""
    text = GATE.read_text(encoding="utf-8")

    every = re.search(r"MUTANT_CALENDAR_EVERY_N=(\d+)", text)
    check(
        every is not None,
        "scripts/test-gate.sh no declara MUTANT_CALENDAR_EVERY_N: el periodo "
        "del calendario no esta escrito, y un calendario sin periodo no acota "
        "la edad de un hueco",
    )
    marker = re.search(r"MUTANT_CALENDAR_MARKER=\"?([0-9a-f]{7,40})\"?", text)
    check(
        marker is not None,
        "scripts/test-gate.sh no declara MUTANT_CALENDAR_MARKER: el calendario "
        "no tiene punto de partida, y una cuenta sin origen no se audita",
    )
    package = re.search(r"MUTANT_CALENDAR_PACKAGE=\"?([A-Za-z0-9_-]+)\"?", text)
    check(
        package is not None,
        "scripts/test-gate.sh no declara MUTANT_CALENDAR_PACKAGE: el calendario "
        "no dice que crate se mide en su turno",
    )
    check(
        "gate_scope.py" in text,
        "scripts/test-gate.sh no pregunta a scripts/gate_scope.py: el gate "
        "decide su radio en vez de leerlo, y dos decisiones son dos "
        "fuentes de verdad",
    )
    check(
        "MUTANTS_JOBS=1" in text,
        "scripts/test-gate.sh no declara MUTANTS_JOBS=1: el veredicto por "
        "mutante deja de ser atribuible",
    )
    check(
        "MUTANTS_GATE_VMEM_KIB" in text,
        "scripts/test-gate.sh no carga el techo del dueño: el valor del paso de "
        "mutantes del gate vive en dos sitios",
    )


def test_the_gate_does_not_hardcode_the_package_it_mutates() -> None:
    """The scope of the mutation step is the answer of the selector. A literal
    package in the command line would be a second decision about the radius,
    and the second one is the one that rots."""
    text = GATE.read_text(encoding="utf-8")
    mutants_line = [
        line
        for line in text.splitlines()
        if "cargo mutants" in line and not line.strip().startswith("#")
    ]
    check(
        bool(mutants_line),
        "scripts/test-gate.sh no tiene una linea de comando de `cargo mutants`",
    )
    for line in mutants_line:
        check(
            "-p cognicode-" not in line,
            "scripts/test-gate.sh fija el paquete que muta en la linea de "
            f"comando ({line.strip()}): el radio es la respuesta del selector, "
            "no una constante del script",
        )


# --- Parte 2: el paso de mutantes ---------------------------------------------


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


# --- Parte 3: los dos pasos de test reparten UNA selección ---------------------


def _gate_step_selection(label: str) -> list[str] | None:
    """The arguments the gate gives nextest in the step called `label`, after
    the packages of the radius.

    The contract reads the command line the gate runs, not a copy of it: a
    copy would be a second promise about the same step.
    """
    for line in GATE.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped.startswith(f'run_test_half "{label}"'):
            continue
        tokens = shlex.split(stripped)
        packages = "${TEST_PKG_ARGS[@]}"
        if packages not in tokens:
            return None
        return tokens[tokens.index(packages) + 1 :]
    return None


def _partition_fixture(root: Path) -> None:
    """A workspace with a test in each kind of target the gate can meet.

    `full` has a library, a binary and an integration test, one test each.
    `onlybin` is a package with a binary and no library, the shape of
    `cognicode-sandbox`: a radius made of it alone is a radius the gate has
    to be able to run. `notests` is a library without a single test: a radius
    made of it alone is a gate that would check nothing.

    The fixture declares the `gate` profile, empty, so the contract can run
    the step lines of the gate as they are written.
    """
    _write(
        root / "Cargo.toml",
        '[workspace]\nmembers = ["full", "onlybin", "notests"]\nresolver = "2"\n',
    )
    _write(root / ".config" / "nextest.toml", "[profile.gate]\n")
    _write(
        root / "notests" / "Cargo.toml",
        '[package]\nname = "notests"\nversion = "0.1.0"\nedition = "2021"\n',
    )
    _write(root / "notests" / "src" / "lib.rs", "pub fn nothing() {}\n")
    _write(
        root / "full" / "Cargo.toml",
        '[package]\nname = "full"\nversion = "0.1.0"\nedition = "2021"\n',
    )
    _write(
        root / "full" / "src" / "lib.rs",
        "pub fn one() -> u32 {\n    1\n}\n\n"
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn in_lib() {\n"
        "        assert_eq!(super::one(), 1);\n    }\n}\n",
    )
    _write(
        root / "full" / "src" / "main.rs",
        "fn main() {\n    println!(\"{}\", full::one());\n}\n\n"
        "#[cfg(test)]\nmod tests {\n    #[test]\n    fn in_bin() {}\n}\n",
    )
    _write(
        root / "full" / "tests" / "together.rs",
        "#[test]\nfn in_tests() {\n    assert_eq!(full::one(), 1);\n}\n",
    )
    _write(
        root / "onlybin" / "Cargo.toml",
        '[package]\nname = "onlybin"\nversion = "0.1.0"\nedition = "2021"\n',
    )
    _write(
        root / "onlybin" / "src" / "main.rs",
        "fn main() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n"
        "    fn in_only_bin() {}\n}\n",
    )


# Options of `nextest run` that decide what a step does with what it selected,
# not what it selects: `nextest list` does not take them. The profile is
# dropped too, because the fixture is not the repository and has no profile
# of its own; what is under test is the selection.
_RUN_ONLY_PREFIXES = ("--no-tests",)


def _as_list_args(selection: list[str]) -> list[str]:
    return [token for token in selection if not token.startswith(_RUN_ONLY_PREFIXES)]


def _nextest(root: Path, verb: str, *args: str) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(root / "target")
    for key in [k for k in env if k.startswith("NEXTEST_")]:
        del env[key]
    return subprocess.run(
        ["cargo", "nextest", verb, *args],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
        timeout=600,
    )


def _listed(root: Path, *args: str) -> set[str] | None:
    proc = _nextest(root, "list", *args, "--message-format", "json")
    if proc.returncode != 0:
        return None
    suites = json.loads(proc.stdout)["rust-suites"]
    return {
        f"{binary_id} {name}"
        for binary_id, suite in suites.items()
        for name in suite["testcases"]
    }


def _names(tests: set[str]) -> set[str]:
    return {test.rsplit(" ", 1)[-1].rsplit("::", 1)[-1] for test in tests}


def test_the_two_test_steps_split_one_selection() -> None:
    """Each test of the radius runs once, and in the step its kind belongs to.

    The unit step is the tests inside the sources (libraries AND binaries);
    the integration step is the targets under `tests/`. Measured on
    2026-11-12 over the workspace: `--lib` then `--tests` ran 9,254 tests for
    5,667 distinct ones, because `--tests` takes the library and the binaries
    too, and the 566 tests of the binaries never ran in the unit step.
    """
    unit = _gate_step_selection("unit tests")
    integration = _gate_step_selection("integration tests")
    check(
        unit is not None and integration is not None,
        "scripts/test-gate.sh no tiene las dos lineas `run_test_half \"unit tests\"` "
        "y `run_test_half \"integration tests\"` con los paquetes del radio: el "
        "contrato no puede leer lo que el gate corre",
    )
    if unit is None or integration is None:
        return

    with tempfile.TemporaryDirectory(prefix="gate-steps-") as tmp:
        root = Path(tmp)
        _partition_fixture(root)

        everything = _listed(root, "--workspace", "--tests")
        in_unit = _listed(root, "--workspace", *_as_list_args(unit))
        in_integration = _listed(root, "--workspace", *_as_list_args(integration))
        check(
            None not in (everything, in_unit, in_integration),
            "`cargo nextest list` no responde en la fixture con la seleccion "
            f"del gate (unitario: {unit}, integracion: {integration})",
        )
        if everything is None or in_unit is None or in_integration is None:
            return

        twice = in_unit & in_integration
        check(
            not twice,
            f"los dos pasos corren los mismos tests: {sorted(_names(twice))}; "
            "el paso de integracion selecciona objetivos lib o bin",
        )
        missing = everything - (in_unit | in_integration)
        check(
            not missing,
            f"ningun paso corre {sorted(_names(missing))}: la particion deja "
            "tests fuera del gate",
        )
        check(
            {"in_lib", "in_bin", "in_only_bin"} <= _names(in_unit),
            "el paso unitario no corre los tests de la lib y de los binarios "
            f"(corre {sorted(_names(in_unit))})",
        )
        check(
            _names(in_integration) == {"in_tests"},
            "el paso de integracion corre algo distinto de los objetivos de "
            f"`tests/` (corre {sorted(_names(in_integration))})",
        )



def _gate_function(name: str) -> str | None:
    """The source of the shell function `name` of the gate, as it is written."""
    found = re.search(
        rf"^{re.escape(name)}\(\) \{{\n.*?^\}}\n",
        GATE.read_text(encoding="utf-8"),
        re.M | re.S,
    )
    return found.group(0) if found else None


def _gate_test_steps() -> str | None:
    """The gate's own test steps, from step 1 to step 3, as they are written.

    The block is taken whole — the two halves, the release check between
    them, and the call that judges the whole selection — so the order the
    contract runs is the order the gate runs: a guard the gate stops calling,
    or calls before the second half, is a guard the contract sees missing.
    """
    lines = GATE.read_text(encoding="utf-8").splitlines(keepends=True)
    start = next((i for i, l in enumerate(lines) if l.startswith("# 1. Unit tests")), None)
    end = next((i for i, l in enumerate(lines) if l.startswith("# 3. Mutants")), None)
    if start is None or end is None or end <= start:
        return None
    return "".join(lines[start:end])


def test_an_empty_half_passes_and_a_radius_without_tests_fails() -> None:
    """A half of the selection can be empty; the whole selection cannot.

    `cognicode-sandbox` has no integration target and `cognicode-graph-wasm`
    has no unit test: a radius made of one of them leaves a half with nothing
    to run, and that half is not a failure. A radius with no test at all is
    a gate that checks nothing, and a gate that checks nothing and says green
    is worse than a slow gate.

    The contract runs the gate's own functions and its own block of test
    steps, in the fixture, with the real nextest: the exit code a half is
    judged by is the one nextest gives, and the order of the calls is the
    gate's, not one the contract writes.
    """
    parts = {
        "run_test_half": _gate_function("run_test_half"),
        "require_a_test_in_the_radius": _gate_function("require_a_test_in_the_radius"),
        "the test steps": _gate_test_steps(),
    }
    absent = [name for name, source in parts.items() if source is None]
    check(
        not absent,
        f"scripts/test-gate.sh no tiene {absent}: el contrato no puede correr "
        "los pasos de test del gate",
    )
    if absent:
        return

    with tempfile.TemporaryDirectory(prefix="gate-halves-") as tmp:
        root = Path(tmp)
        _partition_fixture(root)
        env = dict(os.environ)
        env["CARGO_TARGET_DIR"] = str(root / "target")
        for key in [k for k in env if k.startswith("NEXTEST_")]:
            del env[key]

        for package, expected_rc in (("full", 0), ("onlybin", 0), ("notests", 1)):
            driver = "\n".join(
                [
                    "set -uo pipefail",
                    "overall_rc=0",
                    "EMPTY_HALVES=0",
                    "RUN_UNIT=true",
                    "RUN_INTEGRATION=true",
                    f"TEST_PKG_ARGS=(-p {package})",
                    parts["run_test_half"],
                    parts["require_a_test_in_the_radius"],
                    parts["the test steps"],
                    'exit "$overall_rc"',
                ]
            )
            proc = subprocess.run(
                ["bash", "-c", driver],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                timeout=600,
            )
            check(
                proc.returncode == expected_rc,
                f"los pasos de test del gate con el radio «{package}» salen "
                f"rc={proc.returncode}, se esperaba {expected_rc}: "
                f"{proc.stdout.strip().splitlines()[-2:]}",
            )
            if package == "notests":
                # rc=1 alone is what any failing step gives: the red has to be
                # the one that says the radius has no test.
                check(
                    "no tiene ningun test" in proc.stdout,
                    "el radio sin tests sale en rojo, pero no por la guarda "
                    f"del radio: {proc.stdout.strip().splitlines()[-3:]}",
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
        "PASS — el radio del gate es el cierre inverso, falla cerrado y el "
        "calendario dispara; el paso de mutantes pide memoria dentro de su "
        "presupuesto y mide de uno en uno"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
