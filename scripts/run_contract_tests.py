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
    python3 scripts/run_contract_tests.py --jobs 8 <script.py> [...]
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import traceback
from pathlib import Path
from types import ModuleType


def neutralise_git_configuration() -> None:
    """Give the contracts a git that does not carry the operator's rules.

    A contract has to answer a question about the repository, not about the
    machine that runs it. Several contracts build throwaway repositories and
    commit inside them; on a machine whose git configuration judges who is
    allowed to sign a commit, those commits are refused, the repository
    ends up with no history at all, and the contract reads "no commits"
    where there were two. The gate then fails on a tree that is fine, and
    the failure looks like a defect in the product.

    The preflight of the clean clone already does this for itself, with the
    same reason. Here the whole suite gets it, because a contract is not an
    entry point: it is a test, and a test has to give the same verdict
    everywhere.

    Both the global and the system configuration are neutralised: the
    global one is where the operator's rules live, and the system one is
    where a machine-wide policy would live.
    """
    os.environ["GIT_CONFIG_GLOBAL"] = "/dev/null"
    os.environ["GIT_CONFIG_SYSTEM"] = "/dev/null"


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


def recorded_findings(module: ModuleType) -> list:
    """Findings a contract recorded without raising anything.

    Measured 2026-10-03: 10 of the 12 contracts under `scripts/ci/`
    report by appending to a module-level `failures` list and turning it
    into an exit code in their own `main()`. The runner calls the test
    function and never `main()`, so before this existed every one of them
    passed unconditionally.

    That is not a reporting nit. The supply-chain contract is the thing
    that stops `cargo deny check advisories` from being deleted from the
    merge authority unnoticed, and running it through this runner reported
    `4 passed, 0 failed` and exit 0 on a tree where running the same file
    directly reported exit 1. A gate that cannot fail is worse than no
    gate, because it is reported as green.

    The list is read after each test so the finding is attributed to the
    test that recorded it, and a test that both raises and records is
    still counted once.
    """
    findings = getattr(module, "failures", None)
    return findings if isinstance(findings, list) else []


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

    # A contract may need to do work before its assertions are meaningful.
    # `test_cr06_ratchet.py` reads `M.head_entries`, which its own main()
    # populates with `M.resolve()`; run without that, three of its six
    # checks report "no entries were parsed" -- a false alarm about a
    # measurement that never happened. The runner has to reproduce what
    # main() does, so `setup()` is the hook for it, and a contract that
    # needs one has to say so by defining it.
    setup = getattr(module, "setup", None)
    if callable(setup):
        try:
            setup()
        except Exception:  # noqa: BLE001 - a broken setup is a failure
            detail = traceback.format_exc(limit=6).strip()
            return 0, 1, [f"  FAIL setup() in {path.name}\n{detail}"]

    for name in tests:
        func = getattr(module, name)
        before = len(recorded_findings(module))
        try:
            func()
            new = recorded_findings(module)[before:]
            if new:
                raise AssertionError(
                    f"recorded {len(new)} finding(s) instead of raising; "
                    "the runner reads them so a contract that reports by "
                    "accumulating still gates:\n  - "
                    + "\n  - ".join(str(item) for item in new)
                )
            passed += 1
            print(f"  PASS {name}")
        except Exception:  # noqa: BLE001 - a failing contract is data, not a crash
            detail = traceback.format_exc(limit=6).strip()
            failures.append(f"  FAIL {name}\n{detail}")
            print(f"  FAIL {name}")

    return passed, len(failures), failures


def parse_argv(argv: list[str]) -> tuple[int, list[str]]:
    """Separa `--jobs N` de la lista de ficheros. Devuelve (jobs, ficheros)."""
    jobs = 1
    files: list[str] = []
    index = 0
    while index < len(argv):
        item = argv[index]
        if item == "--jobs":
            if index + 1 >= len(argv):
                print("usage: --jobs necesita un número", file=sys.stderr)
                raise SystemExit(2)
            try:
                jobs = max(1, int(argv[index + 1]))
            except ValueError:
                print(f"usage: --jobs espera un entero, no {argv[index + 1]!r}", file=sys.stderr)
                raise SystemExit(2) from None
            index += 2
            continue
        files.append(item)
        index += 1
    return jobs, files


def run_files_parallel(jobs: int, files: list[str]) -> int:
    """Un proceso por contrato, hasta `jobs` a la vez.

    MEDIDO 2026-10-16. La etapa `contracts` del merge era 371,6 s de 556,8 s del
    gate (el 67%), y su trabajo es lanzar procesos (cargo, git, python) desde
    contratos que declaran ser independientes. El paralelismo aquí es **por
    fichero**: cada contrato se ejecuta en su propio proceso, con el mismo runner
    y el mismo aislamiento que tendría invocado a mano, así que no comparte estado
    con los demás. Un fichero que falla sigue fallando, y el padre devuelve 1 si
    alguno lo hizo: lo que cambia es cuándo se paga el tiempo, no qué se comprueba.

    La salida se imprime **en el orden de entrada**, no según terminan: un log de
    gate que cambia de orden entre corridas es un log que nadie puede diffear.
    """
    from concurrent.futures import ThreadPoolExecutor

    results: dict[str, subprocess.CompletedProcess[str]] = {}

    def _run(raw: str) -> tuple[str, subprocess.CompletedProcess[str]]:
        done = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), raw],
            capture_output=True,
            text=True,
            check=False,
        )
        return raw, done

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        for raw, done in pool.map(_run, files):
            results[raw] = done

    failed = 0
    for raw in files:
        done = results[raw]
        print(done.stdout, end="")
        if done.stderr:
            print(done.stderr, end="", file=sys.stderr)
        if done.returncode != 0:
            print(f"  !! {Path(raw).name} salió con {done.returncode}", file=sys.stderr)
            failed += 1
    return 1 if failed else 0


def main(argv: list[str]) -> int:
    jobs, argv = parse_argv(argv)
    if not argv:
        print("usage: run_contract_tests.py [--jobs N] <script.py> [...]", file=sys.stderr)
        return 2

    # Before anything else: the contracts must not inherit the git rules of
    # the machine that runs them. The parallel path spawns one process per
    # contract and those inherit this environment, so both paths are covered
    # by setting it here.
    neutralise_git_configuration()

    if jobs > 1 and len(argv) > 1:
        return run_files_parallel(jobs, argv)

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
