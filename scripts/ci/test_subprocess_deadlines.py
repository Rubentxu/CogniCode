#!/usr/bin/env python3
"""Contract: every subprocess call in the CI contracts and measure tools carries a deadline.

A call without `timeout` can wait forever: a remote that never answers, a
binary that hangs, a sub-suite that spins. The contract run then stalls without
saying which call stalled (AGENTS.md, hard rule 7).

The scan is syntactic (`ast`), so it needs no process and no network.

Run: python3 scripts/run_contract_tests.py scripts/ci/test_subprocess_deadlines.py
"""

from __future__ import annotations

import ast
import importlib.util
import shutil
import subprocess
import sys
from pathlib import Path

CI_DIR = Path(__file__).resolve().parent
ROOT = CI_DIR.parents[1]
SCANNED = (CI_DIR, ROOT / "scripts" / "measure")
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


def test_every_ci_and_measure_subprocess_call_has_a_deadline() -> None:
    files = sorted(path for directory in SCANNED for path in directory.glob("*.py"))
    assert any(p.parent.name == "measure" for p in files), "scripts/measure no se recorre"
    offenders = [
        f"{path.relative_to(ROOT)}:{line}"
        for path in files
        for line in calls_without_deadline(path.read_text(encoding="utf-8"), str(path))
    ]
    assert not offenders, (
        f"{len(offenders)} llamadas subprocess sin timeout: " + ", ".join(offenders)
    )


def _harness():
    path = ROOT / "scripts" / "measure" / "harness_ab.py"
    spec = importlib.util.spec_from_file_location("harness_ab_deadline", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["harness_ab_deadline"] = module
    spec.loader.exec_module(module)
    return module


def _hanging_run(seen: list[float]):
    def hang(command, *args, **kwargs):
        timeout = kwargs.get("timeout")
        assert timeout, f"llamada sin plazo: {command[:2]}"
        seen.append(timeout)
        raise subprocess.TimeoutExpired(command, timeout)

    return hang


def test_a_measured_run_that_outlives_its_deadline_is_a_timeout_row() -> None:
    harness = _harness()
    real_run, real_which, real_time = subprocess.run, shutil.which, harness.TIME
    seen: list[float] = []
    subprocess.run = _hanging_run(seen)  # type: ignore[assignment]
    shutil.which = lambda name, *a, **k: "/usr/bin/" + name  # type: ignore[assignment]
    harness.TIME = sys.executable
    try:
        result = harness.isolated_run(["x"], timeout_s=10, memory_max="1G")
    finally:
        subprocess.run, shutil.which, harness.TIME = real_run, real_which, real_time
    assert result["exit_code"] == 124 and result["cut"] == "timeout", result
    assert max(seen) > 10, f"el plazo exterior debe dejar margen al interior: {seen}"


def test_helpers_that_hang_degrade_instead_of_hanging() -> None:
    harness = _harness()
    real_run = subprocess.run
    seen: list[float] = []
    subprocess.run = _hanging_run(seen)  # type: ignore[assignment]
    try:
        version = harness.binary_version(Path("/nonexistent/bin"))
        binary = harness.default_binary()
    finally:
        subprocess.run = real_run  # type: ignore[assignment]
    assert version == "", f"sin respuesta no hay version: {version!r}"
    assert binary.name == "cognicode-mcp" and binary.parent.name == "release", binary
