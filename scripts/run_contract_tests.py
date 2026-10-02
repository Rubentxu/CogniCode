#!/usr/bin/env python3
"""Minimal test runner for the standalone contract tests.

The suites under `scripts/product/` and `scripts/ci/` are plain scripts
that expose `test_*` functions. They used to delegate to `pytest.main()`,
which meant they could only run where pytest happened to be installed.

That is not good enough for a merge gate. PR-CI runs on the
`ubuntu-latest` runner, which does not ship pytest, and a suite that only
runs on the maintainer's laptop is a suite nobody gates. The first attempt
at wiring these into CI failed for exactly that reason.

This runner discovers `test_*` callables in a module, runs each one, and
reports failures with the traceback. It deliberately does not reimplement
test discovery or fixtures: these are small, linear, dependency-free
contract checks, and pulling in a framework to run them would trade one
undeclared dependency for a heavier one.

Two properties matter and are asserted by `scripts/ci/test_ci_contracts.py`:

- a failing test produces a non-zero exit code, so `set -e` in CI stops
  the loop;
- a test that raises is reported, not swallowed, and the remaining tests
  still run so one failure does not hide the others.

Usage:
    python3 scripts/run_contract_tests.py <script.py> [<script.py> ...]
"""

from __future__ import annotations

import importlib.util
import sys
import traceback
from pathlib import Path
from types import ModuleType


def load_module(path: Path) -> ModuleType:
    """Import a script by path without requiring it to be on sys.path."""
    # A contract's own directory goes on sys.path so a suite can import a
    # sibling module. Several contracts share `scripts/ci/pipeline_authority.py`
    # rather than each re-deriving which orchestrator is the merge authority,
    # and that import has to work identically whether the suite is run by this
    # runner, by `./scripts/ci/run-all-contracts.sh`, or directly as
    # `python3 scripts/ci/<name>.py`. Only the middle one of those puts the
    # repository root on sys.path, so without this the suite passes on the
    # command line and fails in the gate.
    parent = str(path.resolve().parent)
    if parent not in sys.path:
        sys.path.insert(0, parent)
    spec = importlib.util.spec_from_file_location(path.stem, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    # The suites are scripts, not packages; registering them keeps
    # dataclasses/pickle in any transitive import working.
    sys.modules[path.stem] = module
    spec.loader.exec_module(module)
    return module


def run_one(path: Path) -> tuple[int, int, list[str]]:
    """Return (passed, failed, failure descriptions) for one script."""
    module = load_module(path)
    tests = sorted(
        (name for name in dir(module) if name.startswith("test_")),
        key=lambda name: getattr(module, name).__code__.co_firstlineno,
    )
    if not tests:
        return 0, 0, [f"{path.name}: exposes no test_* function"]

    passed = 0
    failures: list[str] = []
    for name in tests:
        func = getattr(module, name)
        try:
            func()
            passed += 1
            print(f"  PASS {name}")
        except Exception:  # noqa: BLE001 - a failing contract is data, not a crash
            detail = traceback.format_exc(limit=6).strip()
            failures.append(f"  FAIL {name}\n{detail}")
            print(f"  FAIL {name}")

    return passed, len(failures), failures


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: run_contract_tests.py <script.py> [...]", file=sys.stderr)
        return 2

    total_passed = 0
    total_failed = 0
    all_failures: list[str] = []

    for raw in argv:
        path = Path(raw)
        print(f"\n=== {path.name} ===")
        try:
            passed, failed, failures = run_one(path)
        except Exception:  # noqa: BLE001 - import-time failure is a failure
            print(traceback.format_exc(limit=6))
            total_failed += 1
            all_failures.append(f"{path.name}: failed to import\n{traceback.format_exc()}")
            continue
        total_passed += passed
        total_failed += failed
        all_failures.extend(failures)
        print(f"  -> {passed} passed, {failed} failed")

    print(f"\n{'=' * 60}")
    print(f"TOTAL: {total_passed} passed, {total_failed} failed")
    if all_failures:
        print("\nFailures:")
        for failure in all_failures:
            print(failure)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
