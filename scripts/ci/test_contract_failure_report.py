#!/usr/bin/env python3
"""Contract: a failing contract run leaves its full evidence on disk.

A contract that fails once in a hundred runs is only diagnosable if the first
failure keeps what the generator said (rc, stdout, stderr) and the conditions
of the machine (load, temp dir, free space). The report is written by the
parallel runner when at least one contract fails, to the file named by
`CONTRACTS_FAILURE_REPORT`; without the variable nothing is written.

Run: python3 scripts/run_contract_tests.py scripts/ci/test_contract_failure_report.py
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "scripts" / "run_contract_tests.py"


def load_runner():
    spec = importlib.util.spec_from_file_location("run_contract_tests_under_test", RUNNER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def write_contracts(directory: Path) -> tuple[Path, Path]:
    failing = directory / "test_fails.py"
    failing.write_text(
        "import sys\n"
        "def test_boom():\n"
        "    print('GENERATOR-STDOUT-MARK')\n"
        "    print('GENERATOR-STDERR-MARK', file=sys.stderr)\n"
        "    raise AssertionError('boom-detail')\n",
        encoding="utf-8",
    )
    passing = directory / "test_passes.py"
    passing.write_text("def test_ok():\n    assert True\n", encoding="utf-8")
    return failing, passing


def run_runner(files: list[Path], report: Path | None) -> subprocess.CompletedProcess[str]:
    env = {k: v for k, v in os.environ.items() if k != "CONTRACTS_FAILURE_REPORT"}
    if report is not None:
        env["CONTRACTS_FAILURE_REPORT"] = str(report)
    return subprocess.run(
        [sys.executable, str(RUNNER), "--jobs", "2", *map(str, files)],
        capture_output=True,
        text=True,
        env=env,
        timeout=60,
        check=False,
    )


def test_a_failing_contract_writes_the_report_with_output_and_environment() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        failing, passing = write_contracts(directory)
        report = directory / "out" / "report.txt"
        done = run_runner([failing, passing], report)
        assert done.returncode == 1, (done.returncode, done.stdout, done.stderr)
        assert report.exists(), f"no se escribio {report}: {done.stdout} {done.stderr}"
        text = report.read_text(encoding="utf-8")
        for mark in ("test_fails.py", "GENERATOR-STDOUT-MARK", "boom-detail", "loadavg", "tempdir", "free_bytes"):
            assert mark in text, f"falta {mark!r} en el informe:\n{text}"
        assert "test_passes.py" not in text.split("--- environment ---")[0], text


def test_the_stderr_of_the_failing_contract_is_kept() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        failing, _ = write_contracts(directory)
        report = directory / "report.txt"
        run_runner([failing, failing.with_name("test_passes.py")], report)
        assert "GENERATOR-STDERR-MARK" in report.read_text(encoding="utf-8")


def test_a_green_run_writes_no_report() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        _, passing = write_contracts(directory)
        other = directory / "test_also_passes.py"
        other.write_text("def test_ok2():\n    assert True\n", encoding="utf-8")
        report = directory / "report.txt"
        done = run_runner([passing, other], report)
        assert done.returncode == 0, (done.stdout, done.stderr)
        assert not report.exists(), "un run verde no debe dejar informe"


def test_without_the_variable_nothing_is_written() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        failing, passing = write_contracts(directory)
        before = set(directory.rglob("*"))
        done = run_runner([failing, passing], None)
        assert done.returncode == 1
        created = {p for p in directory.rglob("*") if p not in before and p.suffix == ".txt"}
        assert not created, f"se escribio un informe sin pedirlo: {created}"


def test_the_report_builder_is_pure_over_its_inputs() -> None:
    runner = load_runner()
    done = subprocess.CompletedProcess(["x"], 3, "OUT\n", "ERR\n")
    text = runner.build_failure_report(
        [("test_x.py", done)],
        {"loadavg": "1.0 2.0 3.0", "tempdir": "/tmp/t", "free_bytes": 42},
    )
    assert "test_x.py" in text and "rc=3" in text and "OUT" in text and "ERR" in text, text
    assert "loadavg: 1.0 2.0 3.0" in text and "free_bytes: 42" in text, text
