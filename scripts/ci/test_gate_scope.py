#!/usr/bin/env python3
"""The contract of the gate's radius: `scripts/gate_scope.py`.

The gate of this repository decides how much it measures with the answer of
one script. That script is therefore part of the harness of the gate, and a
harness that reduces what it measures is measured itself: these checks are the
proof that the radius is the inverse dependency closure, that an undecidable
radius is the WHOLE gate, and that the calendar fires when it says it does.

The fixture is a real cargo workspace with two crates, `alpha` (a leaf) and
`beta` (which depends on `alpha`), because the closure cannot be tested
against a hand-written list of dependencies: the whole point is that cargo
decides who depends on whom.
"""
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SELECTOR = REPO / "scripts" / "gate_scope.py"
GATE = REPO / "scripts" / "test-gate.sh"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def _git(repo: Path, *args: str) -> str:
    proc = subprocess.run(["git", *args], cwd=repo, capture_output=True, text=True)
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


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("PASS — el radio del gate es el cierre inverso, falla cerrado y el calendario fires")
    return 0


if __name__ == "__main__":
    sys.exit(main())
