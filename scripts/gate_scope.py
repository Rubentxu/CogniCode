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

THE TURN THAT IS NOT REPEATED
-----------------------------
Measuring the crate whole costs about 39 minutes, and a second measurement of
a crate that has not changed produces the same verdicts: the hole did not age,
because nothing that could be mutated moved. So a turn that is DUE is skipped
when, and only when, all of this is proven:

    * the last line `Medición del calendario:` of the crate in the JOURNAL is a
      PASS, and its sha is 40 hex and an ancestor of HEAD;
    * `cargo-mutants` and `rustc` are the versions that line recorded;
    * `git diff` finds nothing between that sha and HEAD in the crate, in the
      workspace crates it depends on, and in `.cargo/mutants.toml`, and the
      working tree has no change or untracked file there.

Skipping is an act of trust earned on proof: every doubt (a missing or
unreadable journal, a short or foreign sha, another tool version, a diff that
cannot be read) measures the crate, and the answer says WHY in
`calendar-skip-refused`. An answer that skips names the commit it compared
with in `calendar-anchor`, so the gate can print it.

The anchor is NOT advanced here and the marker is not touched: the turn is a
crossing of a multiple of the period, so a skipped turn leaves the count
exactly where it was and the next crossing asks the same question again.

What the comparison does NOT cover: the versions of third-party crates in
`Cargo.lock`. The file changed in 13 of the last 120 merges, so comparing it
whole would make a period of 50 merges almost never skippable (a 0.3 %
chance of no change); that a patch bump of `serde` flips a mutant verdict has
not been measured, and nobody has had to claim it.

THE MUTATION MODE
-----------------
A batch pays for the mutants of the lines it changed (`diff`); the crate is
measured whole (`full`) on its turn of the calendar. The two halves hold each
other up: the diff keeps the cost of a batch proportional to what it touched,
and the calendar bounds how long a hole outside any diff can live.

The mode is `diff` only when the calendar is declared AND says it waits. Every
other case is `full`: a calendar that fires, one that cannot be counted, a run
without a calendar (nothing would ever measure the crate whole) and a radius
that could not be determined (there is no base to diff against). An uncertain
answer is more measurement, never less.

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
import os
import re
import subprocess
import sys
from pathlib import Path


class Undetermined(Exception):
    """The radius could not be determined, so the caller has to measure everything."""


class SkipRefused(Exception):
    """The turn of the calendar cannot be skipped, so the crate is measured whole."""


GIT_DEADLINE_S = 120

JOURNAL_REL = Path("docs") / "roadmap" / "JOURNAL.md"
MUTANTS_CONFIG_REL = Path(".cargo") / "mutants.toml"
# The line a block close writes in the JOURNAL after it measured the crate of the
# calendar whole. The fields are ONE promise in two places: this tuple is what
# the parser requires, and `AGENTS.md` carries the template that writes them.
RECEIPT_LABEL = "Medición del calendario:"
RECEIPT_KEYS = ("crate", "sha", "cargo-mutants", "rustc", "resultado")


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


def _last_receipt(repo: Path, crate: str) -> dict[str, str]:
    """The last `Medición del calendario:` line of `crate`, as its fields.

    The last line of the crate in the journal is the one that counts: a red
    measurement after a green one withdraws the anchor, and a green one after
    a red one restores it. The lines of another crate say nothing about this one.
    """
    path = repo / JOURNAL_REL
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise SkipRefused(f"no se pudo leer el JOURNAL ({JOURNAL_REL}): {error}") from error

    last: dict[str, str] | None = None
    for line in text.splitlines():
        body = line.strip().lstrip("-*` ")
        if not body.startswith(RECEIPT_LABEL):
            continue
        fields = {
            key: value.strip("`")
            for key, _, value in (
                token.partition("=") for token in body[len(RECEIPT_LABEL) :].split() if "=" in token
            )
        }
        if fields.get("crate") == crate:
            last = fields
    if last is None:
        raise SkipRefused(
            f"el JOURNAL no tiene ninguna linea «{RECEIPT_LABEL}» del crate {crate}"
        )
    missing = [key for key in RECEIPT_KEYS if not last.get(key)]
    if missing:
        raise SkipRefused(
            f"la ultima linea «{RECEIPT_LABEL}» de {crate} no lleva: {', '.join(missing)}"
        )
    return last


def _tool_versions(repo: Path) -> tuple[str, str]:
    """The versions of `cargo-mutants` and `rustc`, in the form the receipt records.

    A version that cannot be read is not a version that matches.
    """
    proc = _bounded(["cargo", "mutants", "--version"], repo)
    found = re.search(r"cargo-mutants\s+(\S+)", proc.stdout) if proc.returncode == 0 else None
    if not found:
        raise SkipRefused("no se pudo leer la version de cargo-mutants")
    mutants = found.group(1)

    proc = _bounded(["rustc", "--version"], repo)
    found = re.match(r"rustc\s+(\S+)\s+\((\S+)", proc.stdout) if proc.returncode == 0 else None
    if not found:
        raise SkipRefused("no se pudo leer la version de rustc")
    return mutants, f"{found.group(1)}/{found.group(2)}"


def _dependency_closure(crate: str, dependents: dict[str, set[str]]) -> set[str]:
    """The workspace crates `crate` depends on, directly or not.

    `dependents` answers "who depends on X"; this is the same graph read the
    other way round. A crate that nothing in the workspace feeds has an empty
    closure, and then its own directory is all there is to compare.
    """
    dependencies: dict[str, set[str]] = {}
    for provider, consumers in dependents.items():
        for consumer in consumers:
            dependencies.setdefault(consumer, set()).add(provider)
    seen: set[str] = set()
    frontier = [crate]
    while frontier:
        for provider in dependencies.get(frontier.pop(), ()):
            if provider not in seen:
                seen.add(provider)
                frontier.append(provider)
    return seen


def _skip_anchor(
    repo: Path, crate: str, roots: dict[str, str], dependents: dict[str, set[str]]
) -> str:
    """The commit of the last green whole-crate measurement, if it still holds.

    Returns its sha when the crate, what it is built from and the tools are
    those of that measurement. Raises `SkipRefused` with the reason otherwise:
    the caller then measures the crate whole.
    """
    receipt = _last_receipt(repo, crate)
    if receipt["resultado"] != "PASS":
        raise SkipRefused(f"la ultima medicion de {crate} no fue PASS ({receipt['resultado']})")
    sha = receipt["sha"]
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise SkipRefused(f"el SHA del recibo ({sha}) no son 40 hex")
    if not _is_ancestor(repo, sha, "HEAD"):
        raise SkipRefused(f"el SHA del recibo ({sha[:12]}) no es ancestro de HEAD")

    mutants, rustc = _tool_versions(repo)
    if mutants != receipt["cargo-mutants"]:
        raise SkipRefused(
            f"cargo-mutants es {mutants} y el recibo midio con {receipt['cargo-mutants']}"
        )
    if rustc != receipt["rustc"]:
        raise SkipRefused(f"rustc es {rustc} y el recibo midio con {receipt['rustc']}")

    members = [crate, *sorted(_dependency_closure(crate, dependents))]
    paths = [os.path.relpath(roots[name], repo) for name in members] + [str(MUTANTS_CONFIG_REL)]
    where = ", ".join(paths)
    committed = _bounded(["git", "diff", "--quiet", sha, "HEAD", "--", *paths], repo)
    if committed.returncode == 1:
        raise SkipRefused(f"hay cambios desde {sha[:12]} en {where}")
    if committed.returncode != 0:
        raise SkipRefused(
            f"`git diff --quiet` no pudo comparar con {sha[:12]} (rc={committed.returncode})"
        )
    # What is not committed is not in the history, so the anchor cannot vouch for it.
    if _git(["status", "--porcelain", "--untracked-files=all", "--", *paths], repo).strip():
        raise SkipRefused(f"hay cambios sin confirmar en {where}")
    return sha


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
    calendar_anchor = ""
    skip_refused = ""
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
        if calendar_state == "fires":
            # Only a turn that is DUE can be skipped, and only on proof. Any
            # doubt, including a git or a tool that does not answer, keeps the
            # turn: a skip that is not certain is less measurement.
            try:
                calendar_anchor = _skip_anchor(repo, args.calendar_crate, roots, dependents)
                calendar_state = "skipped"
                fires = False
            except (SkipRefused, Undetermined) as error:
                skip_refused = str(error)
        if fires:
            mutant_scope.add(args.calendar_crate)

    reason = calendar_reason
    # `diff` needs a calendar that has been counted and either is not due or
    # was due and is proven redundant: without a counted calendar, nothing
    # would ever measure the whole crate.
    mutant_mode = (
        "diff" if args.calendar_crate and calendar_state in ("waits", "skipped") else "full"
    )
    return {
        "status": "ok",
        "base": base,
        "touched": sorted(touched),
        "test-scope": sorted(test_scope),
        "test-scope-full": sorted(test_scope) == sorted(roots),
        "mutant-scope": sorted(mutant_scope),
        "mutant-mode": mutant_mode,
        "calendar": calendar_state,
        "calendar-count": calendar_count,
        "calendar-start": calendar_start,
        "calendar-next": _next_turn(calendar_count, args.calendar_every)
        if args.calendar_crate and calendar_state != "unknown"
        else 0,
        "calendar-anchor": calendar_anchor,
        "calendar-skip-refused": skip_refused,
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
        "mutant-mode": "full",
        "calendar": "unknown",
        "calendar-count": 0,
        "calendar-start": 0,
        "calendar-next": 0,
        "calendar-anchor": "",
        "calendar-skip-refused": "",
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
            "mutant-mode",
            "calendar",
            "calendar-anchor",
            "calendar-skip-refused",
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
            f"  modo de mutes:  {result['mutant-mode']} "
            + (
                "(solo los mutantes del diff del lote)"
                if result["mutant-mode"] == "diff"
                else "(el crate entero)"
            )
        )
        print(
            f"  calendario:     {result['calendar']} "
            f"({result['calendar-count']} fusiones desde el marcador; "
            f"{result['calendar-start']} al empezar el lote; "
            f"el siguiente turno, a {result['calendar-next']} fusiones)"
        )
        if result["calendar-anchor"]:
            print(
                f"  turno omitido:  sin cambios desde {result['calendar-anchor'][:12]} "
                "(ultima medicion PASS del crate entero)"
            )
        elif result["calendar-skip-refused"]:
            print(f"  turno medido:   {result['calendar-skip-refused']}")
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
    print(f"mutant-mode={result['mutant-mode']}")
    print(f"calendar={result['calendar']}")
    print(f"calendar-count={result['calendar-count']}")
    print(f"calendar-start={result['calendar-start']}")
    print(f"calendar-next={result['calendar-next']}")
    print(f"calendar-anchor={result['calendar-anchor']}")
    print(f"calendar-skip-refused={result['calendar-skip-refused']}")
    print(f"reason={result['reason']}")
    return 0 if result["status"] == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())
