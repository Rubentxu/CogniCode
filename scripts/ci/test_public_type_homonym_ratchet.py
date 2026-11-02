#!/usr/bin/env python3
"""Ratchet: the number of public types sharing one name may not grow (TASK-29).

Measured 2026-11-02 on main: 90 names are declared as `pub struct|enum|trait`
more than once under `crates/*/src` (`Finding` x3, `FindingSeverity` x3,
`SymbolKind` x3, ...). Two types with one name are two sources of truth waiting
to drift, and nothing stopped a fourth.

What it asserts:

1. A name declared more than once at HEAD must not be declared more times than
   at the PR's base commit. A name that was unique at the base (or absent) may
   not become a homonym.
2. Only unrestricted `pub` items count; `pub(crate)` and friends are internal.
   Declarations after the first column-0 `#[cfg(test)]` are test code and are
   not counted, and neither are `*_tests.rs` / `tests.rs` files.

Like `test_file_size_ratchet.py`, the comparison is against the base commit, so
there is no number here to raise. This does not decide which duplicate wins
(that is the open TASK-22 question); it only stops the count growing.

Run:
    python3 scripts/ci/test_public_type_homonym_ratchet.py
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import test_file_size_ratchet as base_lib  # noqa: E402  (git, resolve_base, REPO_ROOT)

DECL = re.compile(r"^\s*pub (?:struct|enum|trait) ([A-Z][A-Za-z0-9_]*)")
CFG_TEST = re.compile(r"^#\[cfg\(test\)\]")

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def public_names(text: str) -> Counter:
    counts: Counter = Counter()
    for line in text.split("\n"):
        if CFG_TEST.match(line):
            break
        match = DECL.match(line)
        if match:
            counts[match.group(1)] += 1
    return counts


class Measure:
    base: str | None = None
    head: Counter = Counter()
    base_counts: Counter = Counter()
    files: int = 0


M = Measure()


def base_sources(base: str) -> list[str]:
    """Same filter as `measured_files`, over the tree at `base` (moves-safe)."""
    listing = base_lib.git("ls-tree", "-r", "--name-only", base, "crates")
    if listing is None:
        return []
    return [
        p
        for p in listing.splitlines()
        if re.match(r"^crates/[^/]+/src/.+\.rs$", p)
        and not (p.endswith("_tests.rs") or p.endswith("/tests.rs"))
    ]


def setup() -> None:
    M.base = base_lib.resolve_base()
    for rel in base_lib.measured_files():
        path = base_lib.REPO_ROOT / rel
        if path.is_file():
            M.files += 1
            M.head += public_names(path.read_text(encoding="utf-8", errors="replace"))
    if M.base:
        for rel in base_sources(M.base):
            old = base_lib.git("show", f"{M.base}:{rel}")
            if old is not None:
                M.base_counts += public_names(old)


def test_declarations_are_counted() -> None:
    sample = "pub struct A;\npub(crate) struct B;\n    pub enum C {}\n#[cfg(test)]\npub struct D;\n"
    check(dict(public_names(sample)) == {"A": 1, "C": 1}, "counts pub only, stops at cfg(test)")


def test_the_scan_found_sources() -> None:
    check(M.files > 100, f"only {M.files} sources scanned; the glob is vacuous")


def test_the_base_is_resolvable() -> None:
    check(
        M.base is not None,
        "cannot resolve a base commit to compare against. Set GITHUB_BASE_SHA, or "
        "fetch origin/main. This fails instead of passing vacuously.",
    )


def test_no_public_type_name_became_more_ambiguous() -> None:
    if not M.base:
        return
    for name, count in sorted(M.head.items()):
        if count < 2:
            continue
        before = M.base_counts.get(name, 0)
        check(
            count <= max(before, 1),
            f"`{name}` is declared {count} times as a public type "
            f"({before} at the base {M.base[:12]}). Reuse the existing one or rename it.",
        )


def main() -> int:
    setup()
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    homonyms = sum(1 for c in M.head.values() if c > 1)
    print(f"homonym ratchet: {homonyms} public names declared more than once (base {(M.base or '?')[:12]})")
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    print("PASS — no public type name became more ambiguous than at the base")
    return 0


if __name__ == "__main__":
    sys.exit(main())
