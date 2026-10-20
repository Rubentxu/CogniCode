#!/usr/bin/env python3
"""Ratchet: a code comment explains the code; it does not cite the paperwork.

The rule
--------
A comment inside the code explains what the code does. It does not carry:

    - a task identifier        (H12, C7, B6.W3, W4, a section mark)
    - a decision identifier    (ADR-024)
    - a specification path     (openspec/specs/...)
    - a pull request reference (PR #390)
    - a date                   (2026-10-20)

Those are all true statements, and all of them expire. A comment that says
"see H12" makes the reader go and find H12; a comment that says what the code
does makes the reader stay. The first kind of comment is also the kind that
rots: the paperwork moves on and the comment keeps pointing at a room nobody
opens any more.

The rule covers the product's code: Rust, TypeScript, JavaScript and Kotlin.
The contract layer is a measurement layer rather than the product, and it
keeps its own convention — a contract records when it measured something, and
that record is the point of the measurement.

Why this is a ratchet and not a lint
------------------------------------
The debt is large and it is unevenly distributed: a handful of files carry
most of it. A lint that failed on every one of those files would be a wall
nobody climbs. A lint that only looked at the diff would be a wall that
appears and disappears with the diff.

So this compares the branch against the commit it came from:

    the offending comment lines this branch touches
        may not exceed the offending comment lines it inherited.

There is no constant in this file to raise. The debt may only shrink, and it
shrinks on its own as the code is touched: a change that removes offending
comments lowers the number for every later change, and a change that adds one
raises it for every later change. That is the whole point.

Only the files this branch touches are measured. A file nobody touched has
the same debt on both sides, so it cancels out, and measuring it again would
cost a full pass over the tree for nothing.

The comparison is fail-closed. If the base cannot be resolved, this does not
pass vacuously.

Approving growth deliberately
-----------------------------
Set `APPROVED_STANDARD_GROWTH` to the number of additional offending lines
this repository has accepted, with a comment naming who approved it and why.
It starts at 0. It is the one escape hatch, and it is a one-line diff.

Resolution of the base commit
-----------------------------
`GITHUB_BASE_SHA` (or `pull_request.base.sha` from `GITHUB_EVENT_PATH`) is
authoritative; otherwise `origin/main`. Both come from the checkout, so this
is runnable locally the same way it runs in CI.

Run:
    python3 scripts/ci/test_comment_standard.py
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

# The scanner that knows where a comment starts and ends. Shared with the
# architectural ratchet, which has to blank the comments out before it counts
# anything else. One scanner, so the two cannot disagree about what a comment
# is.
import comment_scan

REPO_ROOT = Path(__file__).resolve().parents[2]

# The escape hatch. Zero means: this repository has never accepted growing the
# debt, and every change that grows it is expected to fail.
APPROVED_STANDARD_GROWTH = 0

# The suffixes of the files whose comments carry the rule.
CODE_SUFFIXES = frozenset({".rs", ".ts", ".tsx", ".js", ".jsx", ".kt", ".kts"})

# Each pattern is a reference a comment has no business carrying. They are
# anchored so that ordinary words cannot trip them: a word boundary around
# the identifier, and no letter or digit directly after it.
FORBIDDEN: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "a task identifier",
        re.compile(
            r"(?<![A-Za-z0-9_])"
            r"(?:H\d+(?:\.\d+)?"
            r"|H-\d+"
            r"|C\d+"
            r"|D\d+"
            r"|M\d+\.\d+"
            r"|STATE-\d+"
            r"|ST-\d+"
            r"|B\d+\.W\d+"
            r"|PRF-[A-Z]+-\d+"
            r"|PRF[\s-]F\d+(?:\.W\d+)*"
            r"|W\d+"
            r"|§\s?\d+)"
            r"(?![A-Za-z0-9_])"
        ),
    ),
    (
        "a decision identifier",
        re.compile(r"(?<![A-Za-z0-9_])ADR[-\s]?\d+(?![A-Za-z0-9_])"),
    ),
    (
        "a specification path",
        re.compile(r"openspec/(?:changes|specs)/"),
    ),
    (
        "a pull request reference",
        re.compile(r"(?<![A-Za-z0-9_])PR\s?#?\s?\d+(?![A-Za-z0-9_])"),
    ),
    (
        "a date",
        re.compile(r"(?<![0-9])20\d{2}-\d{2}-\d{2}(?![0-9])"),
    ),
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    """Record a finding. The runner reads the list; nothing raises."""
    if not condition:
        failures.append(message)


def git(*args: str) -> str | None:
    """Run a git command in the repo. None if git is unusable here."""
    try:
        completed = subprocess.run(
            ["git", *args],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return completed.stdout if completed.returncode == 0 else None


def resolve_base() -> str | None:
    """The commit this branch is measured against."""
    explicit = os.environ.get("GITHUB_BASE_SHA")
    if explicit:
        return explicit
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    if event_path and Path(event_path).is_file():
        try:
            payload = json.loads(Path(event_path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            payload = {}
        base = payload.get("pull_request", {}).get("base", {}).get("sha")
        if base:
            return str(base)
    for candidate in ("origin/main", "main"):
        if git("rev-parse", "--verify", candidate) is not None:
            return candidate
    return None


def is_code_file(relative_path: str) -> bool:
    return Path(relative_path).suffix.lower() in CODE_SUFFIXES


def changed_code_files(base: str) -> list[str]:
    """The code files this branch touches, including the ones it adds.

    A file nobody touched has the same debt on both sides, so it cancels out
    and measuring it again would cost a full pass over the tree for nothing.
    """
    names: set[str] = set()
    listing = git("diff", "--name-only", "-z", base)
    for name in (listing or "").split("\0"):
        if name and is_code_file(name):
            names.add(name)
    untracked = git("ls-files", "--others", "--exclude-standard", "-z")
    for name in (untracked or "").split("\0"):
        if name and is_code_file(name):
            names.add(name)
    return sorted(names)


def offending_lines(text: str) -> list[tuple[int, str, str]]:
    """The comment lines that carry a reference, as (line, kind, matched)."""
    found: list[tuple[int, str, str]] = []
    for number, comment in comment_scan.comment_lines(text):
        for kind, pattern in FORBIDDEN:
            match = pattern.search(comment)
            if match:
                found.append((number, kind, match.group(0)))
                break
    return found


def working_tree_source(relative_path: str) -> str | None:
    try:
        return (REPO_ROOT / relative_path).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return None


def source_at(base: str, relative_path: str) -> str | None:
    return git("show", f"{base}:{relative_path}")


def debt_of(text: str | None) -> int:
    if text is None:
        return 0
    return len(offending_lines(text))


def setup() -> None:
    """Measure the branch against its base before the checks run."""
    global BASE, TOUCHED, HEAD_DEBT, BASE_DEBT
    BASE = resolve_base()
    if BASE is None:
        TOUCHED = []
        HEAD_DEBT = {}
        BASE_DEBT = {}
        return
    TOUCHED = changed_code_files(BASE)
    HEAD_DEBT = {
        path: debt_of(working_tree_source(path))
        for path in TOUCHED
    }
    BASE_DEBT = {
        path: debt_of(source_at(BASE, path))
        for path in TOUCHED
    }


BASE: str | None = None
TOUCHED: list[str] = []
HEAD_DEBT: dict[str, int] = {}
BASE_DEBT: dict[str, int] = {}


def test_the_base_commit_has_to_be_resolvable() -> None:
    """A ratchet that cannot look is not a ratchet."""
    check(
        BASE is not None,
        "the base commit could not be resolved, so the debt cannot be compared. "
        "This comparison is fail-closed on purpose.",
    )


def test_the_touched_files_do_not_carry_more_debt_than_they_inherited() -> None:
    """The debt may only shrink on the files this branch touches."""
    if BASE is None:
        return
    growth = sum(HEAD_DEBT.values()) - sum(BASE_DEBT.values())
    added = [
        "  %s: %d -> %d" % (path, BASE_DEBT.get(path, 0), HEAD_DEBT[path])
        for path in sorted(HEAD_DEBT)
        if HEAD_DEBT[path] > BASE_DEBT.get(path, 0)
    ]
    check(
        growth <= APPROVED_STANDARD_GROWTH,
        "the offending comment lines grew by %d. A comment explains what the "
        "code does; it does not cite the task, the decision, the pull request "
        "or the date. Write the explanation instead:\n%s"
        % (growth, "\n".join(added)),
    )


def test_the_scanner_sees_a_reference_inside_a_comment() -> None:
    """The measurement has to notice what it is looking for."""
    found = offending_lines("// H12: why this branch exists.\n")
    check(
        len(found) == 1,
        "a comment that names a task has to be counted, and it was not",
    )
    found = offending_lines("//! ADR-024 decides this.\n")
    check(
        len(found) == 1,
        "a comment that names a decision has to be counted, and it was not",
    )
    found = offending_lines("// the chain paginates over the file.\n")
    check(
        not found,
        "a comment that explains the code has to pass, and it did not",
    )


def test_the_scanner_does_not_read_code_as_a_comment() -> None:
    """A reference inside a string is not a reference in a comment."""
    found = offending_lines('let note = "// H12";\n')
    check(
        not found,
        "a string that happens to look like a comment was counted as one",
    )
    found = offending_lines("let ratio = 3.5; // the ratio is 3.5\n")
    check(
        not found,
        "a plain number in a comment was counted as a task identifier",
    )


def test_the_measurement_and_the_scanner_agree_on_the_whole_tree() -> None:
    """The shared scanner is the one the other ratchet uses."""
    sample = 'let x = 1; // H12 cites the task\n/* block\n   ADR-024 too\n*/\n'
    stripped = comment_scan.strip_rust_comments(sample)
    check(
        "H12" not in stripped and "ADR-024" not in stripped,
        "the shared scanner did not blank the comments out, so the two "
        "ratchets would be measuring different things",
    )
    found = offending_lines(sample)
    check(
        len(found) == 2,
        "the shared scanner did not find the two references in the sample",
    )


def test_the_guard_is_run_by_the_merge_authority() -> None:
    """The merge gate has to run this contract, or it is a wish."""
    authority = REPO_ROOT / "scripts/ci/pipeline_authority.py"
    check(authority.is_file(), "scripts/ci/pipeline_authority.py is missing")
    runner = REPO_ROOT / "scripts/ci/run-all-contracts.sh"
    check(runner.is_file(), "scripts/ci/run-all-contracts.sh is missing")
    if not runner.is_file():
        return
    text = runner.read_text(encoding="utf-8")
    check(
        "test_" in text and ".py" in text,
        "the contract runner does not discover python contracts by name, so "
        "this contract would never run in the merge gate",
    )


def main() -> int:
    setup()
    tests = [
        (name, value)
        for name, value in sorted(globals().items())
        if name.startswith("test_") and callable(value)
    ]
    for name, test in tests:
        test()
    if failures:
        for message in failures:
            print("FAIL " + message)
        print("comment standard: %d finding(s)" % len(failures))
        return 1
    print(
        "comment standard: %d offending comment line(s) in %d touched file(s)"
        % (sum(HEAD_DEBT.values()), len(TOUCHED))
    )
    print("comment standard: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
