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

import re
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


def test_runner_honours_the_accumulating_failures_pattern() -> None:
    """The pattern ten of the twelve CI contracts actually use.

    `test_runner_reports_failures_with_nonzero_exit` above proves the
    runner catches a test that *raises*. That is the half that already
    worked. These contracts do not raise: they append to a module-level
    `failures` list and only `main()` turns that into an exit code. The
    runner calls the test function and never calls `main()`, so every one
    of them passed unconditionally.

    The consequence is not a cosmetic reporting gap. This contract is
    what stops `merge-gate.pipeline.kts` from losing its
    `cargo deny check advisories` stage, and it could not report that loss
    if it happened. Measured 2026-10-03: run through the runner,
    `test_supply_chain_gate_contract.py` reported 4 passed / 0 failed
    and exit 0, while running the same file directly reported exit 1 on
    the same tree, in the same second.
    """
    with tempfile.TemporaryDirectory() as tmp:
        suite = Path(tmp) / "test_accumulating.py"
        # The exact shape of the real contracts: a `check` helper that
        # appends, a `test_` function that calls it, and a `main` that is
        # never invoked by the runner.
        suite.write_text(
            "failures = []\n"
            "\n"
            "\n"
            "def check(condition, message):\n"
            "    if not condition:\n"
            "        failures.append(message)\n"
            "\n"
            "\n"
            "def test_violated():\n"
            "    check(False, 'a dependency with an advisory merged unnoticed')\n"
            "\n"
            "\n"
            "def test_satisfied():\n"
            "    check(True, 'never recorded')\n"
            "\n"
            "\n"
            "def main():\n"
            "    return 1 if failures else 0\n",
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
        "a contract that records a violation in its `failures` list must fail "
        f"the runner, got exit {proc.returncode}. The runner calls each test "
        "function but never the module's main(), so a contract that reports "
        "by accumulating rather than raising passes unconditionally. stdout "
        f"was:\n{proc.stdout}",
    )
    check(
        "a dependency with an advisory merged unnoticed" in (proc.stdout + proc.stderr),
        "the runner must print the contract's own finding, not just a test "
        "name; a maintainer reading the gate has to see what was violated. "
        f"stdout was:\n{proc.stdout}",
    )
    check(
        "test_satisfied" in proc.stdout,
        "the runner must still report the tests that passed alongside a "
        f"failure; stdout was:\n{proc.stdout}",
    )


def test_runner_calls_a_contract_setup_before_its_checks() -> None:
    """A contract that must measure before it asserts has to be able to.

    `test_cr06_ratchet.py` reads state its own `main()` populates first.
    Run without that, three of its six checks reported that the CR-06
    allowlist contained no entries -- describing a measurement that was
    never taken as a corrupt inventory. The runner cannot know which
    contracts need that, so it calls an optional module-level `setup()`
    and a contract that needs one defines it. Without this guard the next
    contract to grow a setup phase goes back to failing silently, which
    is the defect the two checks above exist to prevent.
    """
    with tempfile.TemporaryDirectory() as tmp:
        suite = Path(tmp) / "test_needing_setup.py"
        suite.write_text(
            "state = []\n"
            "\n"
            "\n"
            "def setup():\n"
            "    state.append('measured')\n"
            "\n"
            "\n"
            "def test_setup_ran():\n"
            "    assert state == ['measured'], (\n"
            "        'setup() was not called before the checks: '\n"
            "        f'state={state}'\n"
            "    )\n",
            encoding="utf-8",
        )
        ran = subprocess.run(
            [sys.executable, str(RUNNER), str(suite)],
            capture_output=True,
            text=True,
            check=False,
        )

        # And the same fixture with a setup that fails must not be reported
        # as a clean bill either.
        broken = Path(tmp) / "test_broken_setup.py"
        broken.write_text(
            "def setup():\n"
            "    raise RuntimeError('git is unavailable, cannot measure')\n"
            "\n"
            "\n"
            "def test_never_runs():\n"
            "    assert True\n",
            encoding="utf-8",
        )
        failed = subprocess.run(
            [sys.executable, str(RUNNER), str(broken)],
            capture_output=True,
            text=True,
            check=False,
        )

    check(
        ran.returncode == 0 and "PASS test_setup_ran" in ran.stdout,
        "the runner must call a contract's setup() before its checks; stdout "
        f"was:\n{ran.stdout}",
    )
    check(
        failed.returncode != 0,
        "a setup() that raises must fail the runner, not be swallowed into a "
        f"green result; got exit {failed.returncode}. stdout was:\n{failed.stdout}",
    )
    check(
        "git is unavailable" in (failed.stdout + failed.stderr),
        "the runner must print why setup() failed; the operator sees the gate, "
        f"not only its verdict. stdout was:\n{failed.stdout}",
    )


def test_every_ci_contract_is_reachable_by_the_runner() -> None:
    """Anti-vacuity for the property above, measured over the real tree.

    The check above proves the runner handles the accumulating shape. It
    does not prove any real contract uses that shape, which would leave a
    fix that passes its own fixture and governs nothing.

    Measured 2026-10-03 over `scripts/ci/test_*.py`: 10 of 12 record
    findings by appending to a module-level `failures` list, and 6 of
    those contain no `assert` anywhere -- every assertion they have lives
    in the accumulating helper, so nothing in them can ever raise. The two
    that do raise (`test_pipeline_stage_bodies`,
    `test_serial_env_contract`) are the only two the runner could fail
    before this fix.

    This asserts the direction, not the exact count: a contract that
    accumulates is only meaningful if some runner can see it, and the
    runner is the one the merge gate invokes. Every contract is allowed
    to mix the two styles, so the assertion is that at least one exists
    and that each one's own `main` turns its list into a non-zero exit --
    which is what makes running it by hand a real option rather than a
    thing that only works on a maintainer's machine.
    """
    accumulating: list[str] = []
    silent: list[str] = []
    for path in sorted((REPO_ROOT / "scripts" / "ci").glob("test_*.py")):
        text = path.read_text(encoding="utf-8")
        if "failures.append(" not in text:
            continue
        accumulating.append(path.name)
        if not re.search(r"def main\(.*?\).*?return\s+1", text, re.DOTALL):
            silent.append(path.name)
    check(
        len(accumulating) > 0,
        "no contract under scripts/ci records findings by appending to a "
        "`failures` list. If the style was migrated, this contract should be "
        "deleted rather than left asserting a property nothing uses -- but do "
        "not relax it to `>= 0`, which is the vacuous form",
    )
    check(
        not silent,
        "these contracts accumulate findings but their main() never returns a "
        "non-zero exit, so running them by hand would report success too: "
        f"{silent}",
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
