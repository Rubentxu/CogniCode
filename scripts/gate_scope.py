#!/usr/bin/env python3
"""The radius of a gate run: the packages a batch touched, and the ones that can break.

WHAT THIS ANSWERS
-----------------
A gate that always measures everything is a gate whose cost does not depend on
what changed. This script answers the question that lets the gate look at less:

    for a batch (a base commit and the tree on top of it), which packages of the
    workspace did the batch touch, which of them can break because of that, and
    which of them are worth mutating

The three answers are different on purpose:

    touched       the packages whose own files the batch changed
    test scope    the touched packages AND everything that depends on them
    mutant scope the touched packages that are affordable to mutate, plus the
                  crate that carries the graph logic when its turn comes

WHY A PACKAGE GRAPH AND NOT A LIST OF FILES
--------------------------------------------
A file filter asks "does this path look like something this crate cares
about?", and the answer is a guess. A package graph asks "can this crate's
behaviour change because of what changed?", and the answer is a fact: a package
can only be affected by a change in itself or in one of its dependencies. Cargo
already knows that graph, so this script asks cargo and not a heuristic.

This matters because the previous attempt at a fast gate got it wrong. The
retired `scripts/test-fast.sh` selected by path and its fallback path ran
`cargo test --workspace --lib` only, so an integration test that had just been
written never ran: the suite was green because it had not looked. A gate that
reports green without looking is worse than a slow gate, because it is trusted.

FAILING CLOSED, PER ANSWER
--------------------------
Every uncertain answer is MORE measurement, never less.

    radius       If the base cannot be resolved, cargo cannot describe the
                 workspace, or the diff cannot be read, the answer is the whole
                 workspace and every package the gate can mutate, with the
                 reason in the same breath.

    calendar     If the calendar cannot be counted, the crate that carries the
                 graph logic is measured NOW. An uncertain calendar must not
                 turn into a calendar that never fires.

An empty scope would mean "nothing to check", which is the one answer a gate
must never give by accident.

The decision lives HERE and nowhere else, so the caller has no branch of its
own that can be wrong: the gate reads the answer, and the answer is always an
answer.

THE CALENDAR
------------
The crate that carries the graph logic is measured on a turn, not on every
batch. The unit of the calendar is the commit on the certified line: it is
countable from git, it is auditable by anyone, and it needs no state file that
could disagree with the history. The marker is a commit that declares where
the counting starts and the period is how many commits the crate waits between
measurements, so the age of a hole is bounded by that many commits.

The turn is a CROSSING, not a remainder. A batch does not always have the same
size, so a rule of the form "measure when the count is a multiple of N" can
step over the multiple and never fire. The rule here is "measure when the
batch crosses a multiple of N commits since the marker": the position at the
start of the batch and the position at the end are both counted, and the batch
fires when the two fall in different periods. Every multiple is crossed by
exactly one batch, because the batches tile the history.

Usage:
    scripts/gate_scope.py --field test-scope
    scripts/gate_scope.py --field mutant-scope --affordable cognicode-graph-algos
    scripts/gate_scope.py --explain --calendar-crate cognicode-graph-algos \\
        --calendar-every 50 --calendar-marker <sha>

Exit:
    0  the radius was determined
    1  it could not be determined: the answer on stdout is the WHOLE gate, and
       the reason is in `reason`
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


class Undetermined(Exception):
    """The radius could not be determined, so the caller has to measure everything."""


GIT_DEADLINE_S = 120


def _bounded(args: list[str], cwd: Path) -> "subprocess.CompletedProcess[str]":
    """Run a command with a deadline; a command that does not answer is an
    undetermined radius, never a hang."""
    try:
        return subprocess.run(
            args, cwd=cwd, capture_output=True, text=True, timeout=GIT_DEADLINE_S
        )
    except subprocess.TimeoutExpired as error:
        raise Undetermined(
            f"`{' '.join(args)}` no respondio en {GIT_DEADLINE_S} s"
        ) from error


def _run(args: list[str], cwd: Path) -> str:
    """Run a command and return its stdout, or fail closed."""
    proc = _bounded(args, cwd)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout).strip().splitlines()
        raise Undetermined(
            f"`{' '.join(args)}` no se pudo ejecutar: "
            f"{detail[0] if detail else f'rc={proc.returncode}'}"
        )
    return proc.stdout


def _git(args: list[str], repo: Path) -> str:
    return _run(["git", *args], repo)


def _is_ancestor(repo: Path, ancestor: str, descendant: str) -> bool:
    """Whether `ancestor` is on the history of `descendant`."""
    return _bounded(["git", "merge-base", "--is-ancestor", ancestor, descendant], repo).returncode == 0


def _workspace(repo: Path) -> tuple[dict[str, str], dict[str, set[str]]]:
    """The packages of the workspace and who depends on them.

    Returns the package name to its root directory, and the package name to the
    set of packages that depend on it. Cargo is the authority for both: a
    second implementation of the dependency graph here would drift exactly the
    way a hardcoded path once did.
    """
    raw = _run(["cargo", "metadata", "--format-version", "1", "--no-deps"], repo)
    try:
        metadata = json.loads(raw)
    except json.JSONDecodeError as error:
        raise Undetermined(f"`cargo metadata` no devolvió JSON legible: {error}") from error

    members = {package["name"] for package in metadata["packages"]}
    roots = {
        package["name"]: str(Path(package["manifest_path"]).parent.resolve())
        for package in metadata["packages"]
    }
    dependents: dict[str, set[str]] = {name: set() for name in members}
    for package in metadata["packages"]:
        for dependency in package["dependencies"]:
            name = dependency["name"]
            if name in members:
                dependents[name].add(package["name"])
    return roots, dependents


def _resolve_base(repo: Path, requested: str | None) -> str:
    """The commit the batch starts from.

    The caller may name it. Without a name, the base is where this branch
    diverged from the main branch, which is the right answer for a branch. On
    the main branch itself there is no divergence to measure against, and the
    caller has to say where the batch started: guessing would silently measure
    an empty batch.
    """
    if requested:
        return _git(["rev-parse", "--verify", f"{requested}^{{commit}}"], repo).strip()

    head = _git(["rev-parse", "HEAD"], repo).strip()
    main_ref = None
    for candidate in ("origin/main", "main"):
        probe = _bounded(["git", "rev-parse", "--verify", f"{candidate}^{{commit}}"], repo)
        if probe.returncode == 0:
            main_ref = candidate
            break
    if main_ref is None:
        raise Undetermined("no hay ninguna rama `main` que sirva de referencia")

    main_sha = _git(["rev-parse", "--verify", f"{main_ref}^{{commit}}"], repo).strip()
    if head == main_sha:
        raise Undetermined(
            "HEAD es la propia `main`: sin una base explicita no hay lote que medir "
            "(pasa `--base <commit>` con el commit donde empezo el lote)"
        )
    return _git(["merge-base", "HEAD", main_ref], repo).strip()


def _touched_packages(repo: Path, base: str, roots: dict[str, str]) -> set[str]:
    """The packages whose own files the batch changed.

    The diff is against the working tree, not against the last commit: the gate
    measures the tree it is about to certify, and a tree with uncommitted
    changes is a tree whose tests may fail.
    """
    output = _git(["diff", "--name-only", base], repo)
    changed = [line for line in output.splitlines() if line.strip()]

    touched: set[str] = set()
    for name, root in roots.items():
        prefix = root if root.endswith("/") else root + "/"
        for path in changed:
            absolute = str((repo / path).resolve())
            if absolute.startswith(prefix):
                touched.add(name)
                break
    return touched


def _closure(touched: set[str], dependents: dict[str, set[str]]) -> set[str]:
    """The touched packages and everything that depends on them."""
    scope = set(touched)
    frontier = list(touched)
    while frontier:
        current = frontier.pop()
        for dependent in dependents.get(current, ()):
            if dependent not in scope:
                scope.add(dependent)
                frontier.append(dependent)
    return scope


def _position(repo: Path, marker: str, upto: str) -> int:
    """How many commits of the certified line sit between the marker and `upto`.

    The line is the first-parent chain: the main line plus the batch's own
    commits, which is what the certified tree is made of.
    """
    raw = _git(["rev-list", "--count", "--first-parent", f"{marker}..{upto}"], repo)
    return int(raw.strip())


def _next_turn(count: int, every: int) -> int:
    """Merges left until the count reaches the next multiple of the period."""
    return (count // every + 1) * every - count


def _calendar_fires(
    repo: Path, marker: str, base: str, every: int
) -> tuple[bool, int, int]:
    """Whether the crate on the calendar is measured for this batch.

    Returns (fires, position at the end of the batch, position at its start).
    The batch fires when the two positions fall in different periods, which is
    the crossing of a multiple of `every` commits since the marker.

    A marker that is not on the history of the base means the count cannot be
    established, and that is a failure to measure, not an excuse to skip.
    """
    if every < 1:
        raise Undetermined(f"el periodo del calendario no es un numero: {every}")
    if not _is_ancestor(repo, marker, base):
        raise Undetermined(
            f"el marcador del calendario ({marker[:12]}) no esta en la historia de la base"
        )
    head = _git(["rev-parse", "HEAD"], repo).strip()
    start = _position(repo, marker, base)
    end = _position(repo, marker, head)
    return (end // every) > (start // every), end, start


def _declared_roots(roots: dict[str, str], args: argparse.Namespace) -> set[str]:
    """The packages the gate declares it may mutate, checked against reality."""
    affordable = {name for name in (args.affordable or "").split(",") if name}
    unknown = affordable - set(roots)
    if unknown:
        raise Undetermined(
            f"los paquetes mutables declarados no estan en el workspace: "
            f"{', '.join(sorted(unknown))}"
        )
    return affordable


def decide(args: argparse.Namespace) -> dict[str, object]:
    """Everything the gate needs to know about the radius of this batch."""
    repo = Path(args.repo_root).resolve()
    if not (repo / ".git").exists():
        raise Undetermined(f"el repositorio {repo} no existe o no es un repositorio")

    base = _resolve_base(repo, args.base)
    roots, dependents = _workspace(repo)
    touched = _touched_packages(repo, base, roots)
    affordable = _declared_roots(roots, args)

    # A batch that touched no package of the workspace leaves every test
    # standing: the whole workspace is the scope, because a test that reads a
    # file outside its crate has no dependency edge to be selected by.
    test_scope = _closure(touched, dependents) if touched else set(roots)

    mutant_scope = touched & affordable
    calendar_state = "waits"
    calendar_count = 0
    calendar_start = 0
    calendar_reason = ""
    if args.calendar_crate:
        if args.calendar_crate not in roots:
            raise Undetermined(
                f"el crate del calendario ({args.calendar_crate}) no esta en el workspace"
            )
        try:
            fires, calendar_count, calendar_start = _calendar_fires(
                repo, args.calendar_marker, base, args.calendar_every
            )
            calendar_state = "fires" if fires else "waits"
        except Undetermined as error:
            # Fail closed: an uncertain calendar measures the crate NOW. An
            # uncertain calendar must not become a calendar that never fires.
            fires = True
            calendar_state = "unknown"
            calendar_reason = str(error)
        if fires:
            mutant_scope.add(args.calendar_crate)

    reason = calendar_reason
    return {
        "status": "ok",
        "base": base,
        "touched": sorted(touched),
        "test-scope": sorted(test_scope),
        "test-scope-full": sorted(test_scope) == sorted(roots),
        "mutant-scope": sorted(mutant_scope),
        "calendar": calendar_state,
        "calendar-count": calendar_count,
        "calendar-start": calendar_start,
        "calendar-next": _next_turn(calendar_count, args.calendar_every)
        if args.calendar_crate and calendar_state != "unknown"
        else 0,
        "workspace": sorted(roots),
        "reason": reason,
    }


def _whole_gate(args: argparse.Namespace, reason: str) -> dict[str, object]:
    """The answer when the radius could not be determined: the whole gate.

    The workspace is read one more time on purpose. If even that fails there
    is nothing to fall back to but an empty scope, and an empty scope is the
    one answer a gate must never give, so that case raises instead.
    """
    repo = Path(args.repo_root).resolve()
    roots, _ = _workspace(repo)
    affordable = {name for name in (args.affordable or "").split(",") if name}
    affordable = affordable & set(roots)
    mutant_scope = set(affordable)
    if args.calendar_crate and args.calendar_crate in roots:
        mutant_scope.add(args.calendar_crate)
    return {
        "status": "undetermined",
        "base": "",
        "touched": [],
        "test-scope": sorted(roots),
        "test-scope-full": True,
        "mutant-scope": sorted(mutant_scope),
        "calendar": "unknown",
        "calendar-count": 0,
        "calendar-start": 0,
        "calendar-next": 0,
        "workspace": sorted(roots),
        "reason": reason,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="El radio de una corrida del gate: los paquetes que el lote toco."
    )
    parser.add_argument(
        "--base",
        default=None,
        help="el commit donde empezo el lote; sin el, la divergencia con `main`",
    )
    parser.add_argument(
        "--repo-root", default=".", help="la raiz del repositorio que se mide"
    )
    parser.add_argument(
        "--affordable",
        default="",
        help="los paquetes que el gate puede mutar por coste, separados por comas",
    )
    parser.add_argument(
        "--calendar-crate", default="", help="el crate que se mide en su turno"
    )
    parser.add_argument(
        "--calendar-every",
        type=int,
        default=1,
        help="cada cuantas fusiones se mide el crate del calendario",
    )
    parser.add_argument(
        "--calendar-marker", default="", help="el commit donde empieza la cuenta del calendario"
    )
    parser.add_argument(
        "--field",
        default="",
        choices=[
            "",
            "status",
            "base",
            "touched",
            "test-scope",
            "test-scope-full",
            "mutant-scope",
            "calendar",
        ],
        help="un unico campo, separado por espacios, para consumo del shell",
    )
    parser.add_argument(
        "--explain",
        action="store_true",
        help="el radio entero, en prosa, para el log del gate",
    )
    args = parser.parse_args()

    try:
        result = decide(args)
    except Undetermined as error:
        result = _whole_gate(args, str(error))

    if args.explain:
        if result["status"] != "ok":
            print(f"radio: NO DETERMINADO - {result['reason']}")
            print("el gate corre ENTERO: no se acota nada")
        else:
            print(f"radio: base {result['base'][:12]}")
        print(f"  tocado:         {', '.join(result['touched']) or '(ninguno)'}")
        print(f"  radio de test:  {', '.join(result['test-scope'])}")
        print(f"  radio de mutes: {', '.join(result['mutant-scope']) or '(vacio)'}")
        print(
            f"  calendario:     {result['calendar']} "
            f"({result['calendar-count']} fusiones desde el marcador; "
            f"{result['calendar-start']} al empezar el lote; "
            f"el siguiente turno, a {result['calendar-next']} fusiones)"
        )
        if result["reason"]:
            print(f"  motivo:         {result['reason']}")
        return 0 if result["status"] == "ok" else 1

    if args.field:
        value = result[args.field]
        if isinstance(value, bool):
            print("true" if value else "false")
        elif isinstance(value, list):
            print(" ".join(value))
        else:
            print(value)
        return 0 if result["status"] == "ok" else 1

    # The machine interface: one `key=value` per line, so the caller reads it
    # without evaluating anything it did not write.
    print(f"status={result['status']}")
    print(f"base={result['base']}")
    print(f"touched={' '.join(result['touched'])}")
    print(f"test-scope={' '.join(result['test-scope'])}")
    print(f"test-scope-full={'true' if result['test-scope-full'] else 'false'}")
    print(f"mutant-scope={' '.join(result['mutant-scope'])}")
    print(f"calendar={result['calendar']}")
    print(f"calendar-count={result['calendar-count']}")
    print(f"calendar-start={result['calendar-start']}")
    print(f"calendar-next={result['calendar-next']}")
    print(f"reason={result['reason']}")
    return 0 if result["status"] == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())
