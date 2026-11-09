#!/usr/bin/env python3
"""Test-side `RuntimePorts` literals must not list every field.

`RuntimePorts` grows with the optional features (`multimodal` adds
`federation_store` and `ingest_commit_port`). A test that lists the fields one
by one compiles with the default features and then fails with E0063 under
`--all-features`, which nobody builds with `--all-targets` by default. The
struct derives `Default`, so a test literal names only the ports it exercises
and ends with `..Default::default()`.

The production wiring in `bootstrap_ladybug` is exempt on purpose: it lists
every port so that a new port cannot be left unwired without a compile error.

Run: `python3 -I scripts/ci/test_runtime_ports_initializers.py`
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME = REPO_ROOT / "crates" / "cognicode-runtime"
LITERAL = re.compile(r"RuntimePorts\s*\{")


def _literal_bodies(text: str) -> list[tuple[int, str]]:
    """Return (line, body) for each `RuntimePorts { ... }` struct literal."""
    found = []
    for match in LITERAL.finditer(text):
        before = text[: match.start()].rstrip()
        if before.endswith("struct"):
            continue
        depth, i = 1, match.end()
        while i < len(text) and depth:
            depth += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
        found.append((text.count("\n", 0, match.start()) + 1, text[match.end() : i - 1]))
    return found


def _test_literals() -> list[tuple[str, int, str]]:
    out = []
    for path in sorted((RUNTIME / "tests").glob("*.rs")):
        for line, body in _literal_bodies(path.read_text()):
            out.append((str(path.relative_to(REPO_ROOT)), line, body))
    lib = RUNTIME / "src" / "lib.rs"
    text = lib.read_text()
    tests_start = text.index("#[cfg(test)]")
    offset = text.count("\n", 0, tests_start)
    for line, body in _literal_bodies(text[tests_start:]):
        out.append((str(lib.relative_to(REPO_ROOT)), line + offset, body))
    return out


def test_the_scan_sees_the_literals_it_governs() -> None:
    literals = _test_literals()
    assert len(literals) >= 3, f"expected the known test literals, found {len(literals)}"


def test_test_literals_end_with_default() -> None:
    bad = [
        f"{path}:{line}"
        for path, line, body in _test_literals()
        if "..Default::default()" not in body
    ]
    assert not bad, (
        "RuntimePorts literals in tests must end with `..Default::default()` so a "
        "feature-gated field does not break `--all-features --all-targets` (E0063): "
        + ", ".join(bad)
    )


if __name__ == "__main__":
    failed = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"PASS {name}")
            except AssertionError as err:
                failed += 1
                print(f"FAIL {name}: {err}")
    sys.exit(1 if failed else 0)
