#!/usr/bin/env python3
"""Contract: every subprocess call in the CI contracts carries a deadline.

A call without `timeout` can wait forever: a remote that never answers, a
binary that hangs, a sub-suite that spins. The contract run then stalls without
saying which call stalled (AGENTS.md, hard rule 7).

The scan is syntactic (`ast`), so it needs no process and no network.

Run: python3 scripts/run_contract_tests.py scripts/ci/test_subprocess_deadlines.py
"""

from __future__ import annotations

import ast
from pathlib import Path

CI_DIR = Path(__file__).resolve().parent
ROOT = CI_DIR.parents[1]
CALLS = {"run", "check_output", "check_call", "call"}


def calls_without_deadline(source: str, filename: str = "<src>") -> list[int]:
    """Lines of `subprocess.<call>(...)` that do not pass `timeout=`."""
    lines: list[int] = []
    for node in ast.walk(ast.parse(source, filename)):
        if not isinstance(node, ast.Call):
            continue
        func = node.func
        if not (
            isinstance(func, ast.Attribute)
            and func.attr in CALLS
            and isinstance(func.value, ast.Name)
            and func.value.id == "subprocess"
        ):
            continue
        if not any(kw.arg == "timeout" for kw in node.keywords):
            lines.append(node.lineno)
    return sorted(lines)


def test_the_scan_sees_a_call_without_a_deadline() -> None:
    source = "import subprocess\nsubprocess.run(['x'])\nsubprocess.run(['y'], timeout=5)\n"
    found = calls_without_deadline(source)
    assert found == [2], f"el escaneo debe marcar solo la linea 2, marco {found}"


def test_every_ci_subprocess_call_has_a_deadline() -> None:
    files = sorted(CI_DIR.glob("*.py"))
    assert files, "no se encontro ningun fichero en scripts/ci"
    offenders = [
        f"{path.relative_to(ROOT)}:{line}"
        for path in files
        for line in calls_without_deadline(path.read_text(encoding="utf-8"), str(path))
    ]
    assert not offenders, (
        f"{len(offenders)} llamadas subprocess sin timeout: " + ", ".join(offenders)
    )
