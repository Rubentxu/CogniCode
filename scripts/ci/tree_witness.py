#!/usr/bin/env python3
"""Tree witness: what did the suites leave in the working tree?

A test that writes into the repository is a test with shared state: it breaks
rule 1 of AGENTS.md and it dirties the `git status` of every other worktree.
The way it hides is `.gitignore`: each accident that git was told to ignore is
an accident nobody sees again, and the file grew an entry per accident
instead of the accident being stopped.

The witness is the cheap check that stops it. It records the state of the
tree before the suites and compares it afterwards; whatever is new is residue.

    tree_witness.py snapshot FILE [--root DIR]
    tree_witness.py compare  FILE [--root DIR]

Exit codes: 0 nothing new, 1 residue (named, one per line), 2 the witness
could not tell. A witness that cannot tell fails closed: no snapshot, an
unreadable snapshot or a directory that is not a repository is an error,
never a clean bill.

What the state is. `git status --porcelain --ignored`, so ignored files count
(they are exactly what hides), plus the full list of untracked files, so a
new file inside a directory that was already untracked is not folded into the
directory's one line. Ignored directories stay folded: expanding `node_modules`
turns a one-second check into a 190 000-line one. The consequence is declared
here instead of discovered later: a new file inside an ignored directory that
already existed is not seen. A tracked file that was already modified before
the snapshot and is modified again is not seen either: the comparison is by
entry, not by content.

What is not residue. Only what the harness itself writes into the tree, named
below with the reason. Everything else a suite wants to write goes into a
temp directory it owns.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# (prefix at the root of the tree, why the harness writes it). A prefix is
# matched against the path from the root, so `crates/mutants.out/` is NOT the
# report: only the one at the root is.
ALLOWED_ROOT_PREFIXES: tuple[tuple[str, str], ...] = (
    ("mutants.out/", "report of the gate's mutation step (cargo-mutants writes it in the cwd)"),
    ("mutants.out.old/", "the previous report, which cargo-mutants renames on the next run"),
    ("target/", "build output; the build directory is where compilation is allowed to write"),
)

# A path component that is tooling state wherever it appears.
ALLOWED_COMPONENTS: tuple[tuple[str, str], ...] = (
    ("__pycache__", "bytecode the Python contracts leave when they import a sibling module"),
)


class WitnessError(Exception):
    """The witness could not tell. Always an error, never a clean bill."""


def _git(root: Path, *args: str) -> str:
    try:
        proc = subprocess.run(
            ["git", *args],
            cwd=root,
            capture_output=True,
            timeout=120,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise WitnessError(f"git {' '.join(args)} could not run in {root}: {error}") from error
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise WitnessError(f"git {' '.join(args)} failed in {root}: {detail}")
    return proc.stdout.decode("utf-8", "replace")


def state_of(root: Path) -> list[str]:
    """The sorted entries of the tree: `XY path`, as `git status --porcelain` says."""
    entries: set[str] = set()

    fields = _git(root, "status", "--porcelain", "-z", "--ignored").split("\0")
    index = 0
    while index < len(fields):
        field = fields[index]
        index += 1
        if len(field) < 4:
            continue
        entries.add(field)
        # A rename or a copy carries its origin as one more field; the origin
        # is not an entry.
        if field[0] in "RC" or field[1] in "RC":
            index += 1

    for path in _git(root, "ls-files", "-z", "--others", "--exclude-standard").split("\0"):
        if path:
            entries.add(f"?? {path}")

    return sorted(entries)


def _path_of(entry: str) -> str:
    return entry[3:]


def is_allowed(path: str) -> bool:
    if any(path.startswith(prefix) for prefix, _ in ALLOWED_ROOT_PREFIXES):
        return True
    components = path.rstrip("/").split("/")
    return any(component == name for component in components for name, _ in ALLOWED_COMPONENTS)


def residue(before: list[str], after: list[str]) -> list[str]:
    """What is in `after` and was not in `before`, minus what the harness writes."""
    known = set(before)
    return [entry for entry in after if entry not in known and not is_allowed(_path_of(entry))]


def _snapshot(root: Path, target: Path) -> int:
    entries = state_of(root)
    try:
        target.write_text(json.dumps(entries), encoding="utf-8")
    except OSError as error:
        raise WitnessError(f"cannot write the snapshot {target}: {error}") from error
    print(f"tree-witness: snapshot of {len(entries)} entries -> {target}")
    return 0


def _read_snapshot(source: Path) -> list[str]:
    try:
        entries = json.loads(source.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise WitnessError(f"cannot read the snapshot {source}: {error}") from error
    if not isinstance(entries, list) or not all(isinstance(entry, str) for entry in entries):
        raise WitnessError(f"the snapshot {source} is not a list of entries")
    return entries


def _compare(root: Path, source: Path) -> int:
    before = _read_snapshot(source)
    found = residue(before, state_of(root))
    if not found:
        print("tree-witness: the suites left nothing new in the working tree")
        return 0
    print(f"tree-witness: FAIL - the suites left {len(found)} new entries in the working tree:")
    for entry in found:
        print(f"  {entry}")
    print(
        "A test or contract that needs a file writes it into a temp directory it owns "
        "(AGENTS.md, rule 1); `.gitignore` is not the place to hide it."
    )
    return 1


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("command", choices=("snapshot", "compare"))
    parser.add_argument("file", type=Path, help="where the snapshot is written / read")
    parser.add_argument("--root", type=Path, default=ROOT, help="the tree to watch")
    args = parser.parse_args(argv)
    try:
        if args.command == "snapshot":
            return _snapshot(args.root, args.file)
        return _compare(args.root, args.file)
    except WitnessError as error:
        print(f"tree-witness: ERROR - {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
