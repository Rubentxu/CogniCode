#!/usr/bin/env python3
"""Ratchets over the production sources in `crates/*/src`: file size and
public-type homonyms.

Both compare HEAD with the PR's base commit, so there is no number in this
file to raise and nothing to edit when the measured value shrinks: the base
catches up on merge and the next PR measures against the smaller value. Both
fail closed: if the base cannot be resolved they fail instead of passing
vacuously.

Production code is the code before the first column-0 `#[cfg(test)]`. Files
named `*_tests.rs` or `tests.rs` are test code and are skipped; files under
`tests/` are not measured at all.

## File size

A production source file may not exceed `LIMIT` lines, and one that already
does may not grow. A file that was at or under `LIMIT` at the base (or did not
exist) may not cross it. This mirrors `test_cr06_ratchet.py` and deliberately
does NOT re-implement the CR-06 layering rule: that rule has one owner, and a
second copy would be a second source of truth.

## Public-type homonyms

A name declared more than once as `pub struct|enum|trait` must not be declared
more times than at the base. A name that was unique at the base (or absent) may
not become a homonym. Only unrestricted `pub` items count; `pub(crate)` and
friends are internal. This does not decide which duplicate wins; it only stops
the count growing.

Run:
    python3 scripts/ci/test_ratchets.py
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
LIMIT = 2000
CFG_TEST = re.compile(r"^#\[cfg\(test\)\]")
DECL = re.compile(r"^\s*pub (?:struct|enum|trait) ([A-Z][A-Za-z0-9_]*)")

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def git(*args: str) -> str | None:
    try:
        out = subprocess.run(
            ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, timeout=60
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout if out.returncode == 0 else None


def production_lines(text: str) -> int:
    """Lines before the first column-0 `#[cfg(test)]`; all lines if none."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    for number, line in enumerate(lines):
        if CFG_TEST.match(line):
            return number
    return len(lines)


def public_names(text: str) -> Counter:
    counts: Counter = Counter()
    for line in text.split("\n"):
        if CFG_TEST.match(line):
            break
        match = DECL.match(line)
        if match:
            counts[match.group(1)] += 1
    return counts


def resolve_base() -> str | None:
    explicit = os.environ.get("GITHUB_BASE_SHA", "").strip()
    if explicit:
        return explicit
    event_path = os.environ.get("GITHUB_EVENT_PATH", "").strip()
    if event_path and Path(event_path).is_file():
        try:
            event = json.loads(Path(event_path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            event = {}
        sha = (event.get("pull_request") or {}).get("base", {}).get("sha")
        if isinstance(sha, str) and sha.strip():
            return sha.strip()
    for candidate in ("origin/main", "main"):
        resolved = git("rev-parse", "--verify", f"{candidate}^{{commit}}")
        if resolved:
            return resolved.strip()
    return None


def measured_files() -> list[str]:
    """Tracked production sources: crates/<crate>/src/**/*.rs."""
    listing = git("ls-files", "crates/*/src/**/*.rs", "crates/*/src/*.rs")
    if listing is None:
        return []
    return sorted(
        {
            p
            for p in listing.splitlines()
            if p.endswith(".rs") and not (p.endswith("_tests.rs") or p.endswith("/tests.rs"))
        }
    )


def base_sources(base: str) -> list[str]:
    """Same filter as `measured_files`, over the tree at `base` (moves-safe)."""
    listing = git("ls-tree", "-r", "--name-only", base, "crates")
    if listing is None:
        return []
    return [
        p
        for p in listing.splitlines()
        if re.match(r"^crates/[^/]+/src/.+\.rs$", p)
        and not (p.endswith("_tests.rs") or p.endswith("/tests.rs"))
    ]


class Measure:
    base: str | None = None
    # file size
    head: dict[str, int] = {}
    oversize: dict[str, int] = {}
    base_sizes: dict[str, int | None] = {}
    # public-type homonyms
    names: Counter = Counter()
    base_names: Counter = Counter()
    files: int = 0


M = Measure()


def setup() -> None:
    M.base = resolve_base()
    for rel in measured_files():
        path = REPO_ROOT / rel
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        size = production_lines(text)
        M.head[rel] = size
        if size > LIMIT:
            M.oversize[rel] = size
        M.files += 1
        M.names += public_names(text)
    if M.base:
        for rel in M.oversize:
            old = git("show", f"{M.base}:{rel}")
            M.base_sizes[rel] = None if old is None else production_lines(old)
        for rel in base_sources(M.base):
            old = git("show", f"{M.base}:{rel}")
            if old is not None:
                M.base_names += public_names(old)


def test_the_base_is_resolvable() -> None:
    check(
        M.base is not None,
        "cannot resolve a base commit to compare against. Set GITHUB_BASE_SHA, or "
        "fetch origin/main. This fails instead of passing vacuously.",
    )


# --- file size ---------------------------------------------------------------


def test_file_size_counting_stops_at_the_first_cfg_test() -> None:
    check(production_lines("a\nb\n#[cfg(test)]\nmod t {}\n") == 2, "stops at cfg(test)")
    check(production_lines("a\nb\nc\n") == 3, "counts all lines without cfg(test)")
    check(production_lines("a\n    #[cfg(test)]\n") == 2, "indented cfg(test) is not a module boundary")
    check(production_lines("") == 0, "empty file is zero lines")


def test_file_size_scan_found_sources() -> None:
    check(len(M.head) > 100, f"only {len(M.head)} sources measured; the glob is vacuous")


def test_no_file_grew_past_its_base_or_crossed_the_limit() -> None:
    if not M.base:
        return
    for rel, size in sorted(M.oversize.items()):
        old = M.base_sizes.get(rel)
        if old is None or old <= LIMIT:
            check(
                False,
                f"{rel}: {size} production lines crosses the {LIMIT}-line limit "
                f"(base: {'new file' if old is None else old}). Split it.",
            )
        else:
            check(
                size <= old,
                f"{rel}: {size} production lines > {old} at the base "
                f"({M.base[:12]}). A file over {LIMIT} may only shrink.",
            )


# --- public-type homonyms ----------------------------------------------------


def test_homonym_declarations_are_counted() -> None:
    sample = "pub struct A;\npub(crate) struct B;\n    pub enum C {}\n#[cfg(test)]\npub struct D;\n"
    check(dict(public_names(sample)) == {"A": 1, "C": 1}, "counts pub only, stops at cfg(test)")


def test_homonym_scan_found_sources() -> None:
    check(M.files > 100, f"only {M.files} sources scanned; the glob is vacuous")


def test_no_public_type_name_became_more_ambiguous() -> None:
    if not M.base:
        return
    for name, count in sorted(M.names.items()):
        if count < 2:
            continue
        before = M.base_names.get(name, 0)
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

    homonyms = sum(1 for c in M.names.values() if c > 1)
    base = (M.base or "?")[:12]
    print(f"file-size ratchet: {len(M.oversize)} of {len(M.head)} sources over {LIMIT} production lines (base {base})")
    print(f"homonym ratchet: {homonyms} public names declared more than once (base {base})")
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1
    shrunk = [r for r, s in M.oversize.items() if (M.base_sizes.get(r) or 0) > s]
    for rel in shrunk:
        print(f"  shrank: {rel} {M.base_sizes[rel]} -> {M.oversize[rel]}")
    print("PASS — no production file crossed the limit or grew past its base size, and no public type name became more ambiguous")
    return 0


if __name__ == "__main__":
    sys.exit(main())
