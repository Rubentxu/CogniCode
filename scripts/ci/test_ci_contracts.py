#!/usr/bin/env python3
"""Contracts for the CI and product test harness itself.

This suite exists because the harness failed twice in ways that were
invisible locally.

1. `scripts/product/test_oss_foundation.py` and `test_profiles.py`
   delegated their `__main__` to `pytest.main()`. They passed on the
   maintainer's machine and failed on the PR-CI runner with
   `ModuleNotFoundError: No module named 'pytest'`, because
   ubuntu-latest does not ship it. The contracts were fine; the harness
   was not portable, and nothing local could have caught that.

2. A GitHub Actions `run: |` block does not enable `set -e`. When the
   job iterated over five test files, a failure in an early file would
   not stop the loop and only the last file would set the exit code. With
   one file that is invisible.

So the harness is pinned here on properties:

- no test script under `scripts/` may import pytest, because that makes
  it unrunnable on the merge gate;
- the runner exits non-zero when any test fails, and continues running
  the rest so one failure does not mask others;
- a script that exposes no test is reported, not counted as passing;
- the suite passes in an interpreter that has no pytest at all.

The third of those used to be a per-file count, and that is worth
recording: the merge path was asserted by reading the orchestrator's
source — `set -euo pipefail` present, `run_contract_tests.py` invoked,
`pytest` never mentioned. All three were true only because the text said
so, and two of them describe a mechanism PipelineK does not have. The
properties behind them are now asserted against the things that actually
decide what runs: `test_cr06_ratchet.py` walks the chain from the merge
authority through `run-all-contracts.sh` to this glob, and the
`no_contract_test_imports_pytest` and `suite_runs_without_pytest_installed`
checks below cover the portability property without reading any
orchestrator.

Run:
    python3 scripts/ci/test_ci_contracts.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNNER = REPO_ROOT / "scripts" / "run_contract_tests.py"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def test_no_contract_test_imports_pytest() -> None:
    """A suite that needs pytest cannot run on the merge gate.

    Asserted over the whole `scripts/` tree, not just the two files that
    were caught, so the next suite that reaches for pytest is caught here
    rather than in CI.
    """
    offenders: list[str] = []
    for path in sorted((REPO_ROOT / "scripts").rglob("*.py")):
        if "__pycache__" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        for lineno, line in enumerate(text.splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith(("import pytest", "from pytest")):
                offenders.append(f"{path.relative_to(REPO_ROOT)}:{lineno}")
    check(
        not offenders,
        "these scripts import pytest, so they cannot run on the ubuntu-latest "
        f"runner: {offenders}",
    )


def test_runner_reports_failures_with_nonzero_exit() -> None:
    """A failing test must fail the runner, and must not hide the others."""
    with tempfile.TemporaryDirectory() as tmp:
        suite = Path(tmp) / "test_synthetic.py"
        suite.write_text(
            "def test_passes():\n"
            "    assert True\n"
            "\n"
            "def test_fails():\n"
            "    assert False, 'contract violated'\n"
            "\n"
            "def test_also_passes():\n"
            "    assert True\n",
            encoding="utf-8",
        )
        proc = subprocess.run(
            [sys.executable, str(RUNNER), str(suite)],
            capture_output=True,
            text=True,
            check=False,
        )

    check(
        proc.returncode != 0,
        f"runner must exit non-zero when a test fails, got {proc.returncode}",
    )
    check(
        "test_fails" in proc.stdout,
        "runner must name the failing test; stdout was:\n" + proc.stdout,
    )
    check(
        "test_also_passes" in proc.stdout,
        "runner must keep going after a failure so one does not hide the "
        "others; stdout was:\n" + proc.stdout,
    )
    check(
        "2 passed" in proc.stdout,
        f"runner must count the passing tests; stdout was:\n{proc.stdout}",
    )


def test_runner_rejects_a_script_with_no_tests() -> None:
    """A file that exposes no test must not be reported as passing."""
    with tempfile.TemporaryDirectory() as tmp:
        suite = Path(tmp) / "test_empty.py"
        suite.write_text("VALUE = 1\n", encoding="utf-8")
        proc = subprocess.run(
            [sys.executable, str(RUNNER), str(suite)],
            capture_output=True,
            text=True,
            check=False,
        )
    check(
        proc.returncode != 0,
        f"a script with no test_* function must not exit 0, got {proc.returncode}",
    )
    check(
        "no test_* function" in proc.stdout,
        f"the reason must be reported; stdout was:\n{proc.stdout}",
    )


def test_suite_runs_without_pytest_installed() -> None:
    """The harness must work in an interpreter that has no pytest.

    The local interpreter having pytest is exactly what hid this defect:
    `pytest.main()` worked on the maintainer's machine and raised
    `ModuleNotFoundError` on the runner. This runs the runner under
    `scripts/ci/no_pytest_env/`, a `sitecustomize.py` that raises
    `ImportError` on any `import pytest`, so the guarantee is exercised on
    every merge instead of being assumed.
    """
    import os

    env = dict(os.environ)
    env["PYTHONPATH"] = str(REPO_ROOT / "scripts" / "ci" / "no_pytest_env")

    # First: the simulated environment must actually block pytest. A
    # sitecustomize that silently fails to load would make this whole check
    # pass without proving anything.
    probe = subprocess.run(
        [sys.executable, "-c", "import pytest"],
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    check(
        probe.returncode != 0 and "No module named" in probe.stderr,
        "the simulated runner environment does not block pytest; the whole "
        f"guarantee would be vacuous. stderr was: {probe.stderr.strip()}",
    )

    # Second: the real suites must pass under it.
    proc = subprocess.run(
        [
            sys.executable,
            str(REPO_ROOT / "scripts" / "run_contract_tests.py"),
            str(REPO_ROOT / "scripts" / "ci" / "test_select_suites.py"),
        ],
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )
    check(
        proc.returncode == 0,
        f"the suite must pass with pytest unavailable; exit {proc.returncode}\n{proc.stdout}",
    )


def main() -> int:
    check(RUNNER.is_file(), f"runner not found: {RUNNER}")

    # Each check is independent and reports its own verdict, so a single
    # run tells us everything that is wrong, not just the first thing.
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print("PASS — harness is pytest-free, fails fast, and reports honestly")
    return 0


if __name__ == "__main__":
    sys.exit(main())
