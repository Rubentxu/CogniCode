#!/usr/bin/env python3
"""The contract of the recall measurement: `scripts/affected-tests-recall.py`.

The measurement compares the selection the product publishes against the
tests a complete run proves reach the change. Both sides can be given, and
that is what makes the comparison testable without a compiler in the loop:
the selection is a JSON document, and the run is a command that prints the
tests that fail.

What these checks prove:

    - the recall is computed as it says: of the tests the execution
      reaches, the ones the selection names;
    - a miss is REPORTED, not swallowed: a selection that names nothing
      says so, and says which test it failed to name;
    - the tree is left as it was: the file whose contract was broken is
      restored byte for byte;
    - a measurement that cannot be made is an error, not a number: a
      symbol that does not exist stops the run.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "affected-tests-recall.py"
GATE = REPO / "scripts" / "test-gate.sh"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


LIB = """pub fn watched() -> u32 {
    41
}
"""

TEST = """#[test]
fn the_test_that_watches() {
    assert_eq!(1, 1);
}
"""

# The runner the fixture uses. It fails the test that reaches the change
# only when the contract of the symbol is broken, which is what a real
# complete run does.
#
# The line it prints is the line nextest prints, with the counter of the
# progress between the time and the binary: the reader of the output has to
# understand the format that the real runner writes, not one that is easier
# to parse. And it prints it on STDERR, which is where the real runner
# writes what it did —its stdout stays empty—.
RUNNER = """#!/usr/bin/env python3
import os
import sys
from pathlib import Path

fixture = Path(os.environ["RECALL_FIXTURE"])
source = (fixture / "src" / "lib.rs").read_text(encoding="utf-8")
if "affected-tests-recall" in source:
    print("        FAIL [   0.005s] (   1/2) demo it::the_test_that_watches", file=sys.stderr)
sys.exit(100)
"""


def write_fixture(root: Path) -> Path:
    (root / "src").mkdir(parents=True)
    (root / "tests" / "it").mkdir(parents=True)
    (root / "Cargo.toml").write_text(
        '[package]\nname = "demo"\nversion = "0.1.0"\nedition = "2021"\n\n'
        "[workspace]\n",
        encoding="utf-8",
    )
    (root / "src" / "lib.rs").write_text(LIB, encoding="utf-8")
    (root / "tests" / "it" / "checks.rs").write_text(TEST, encoding="utf-8")
    runner = root / "runner.sh"
    runner.write_text(RUNNER, encoding="utf-8")
    runner.chmod(0o755)
    return runner


def measure(
    fixture: Path,
    runner: Path,
    selection: list[dict],
    symbol: str = "watched",
    report: Path | None = None,
) -> tuple[int, dict, str]:
    """Run the measurement over the fixture and return its verdict."""
    document = fixture / "selection.json"
    document.write_text(
        json.dumps({"kind": "impact", "affected_tests": selection}), encoding="utf-8"
    )
    command = [
        sys.executable,
        str(SCRIPT),
        "--package",
        "demo",
        "--symbol",
        symbol,
        "--manifest-path",
        str(fixture / "Cargo.toml"),
        "--runner",
        str(runner),
        "--selection",
        str(document),
        "--json",
    ]
    if report is not None:
        command += ["--report", str(report)]
    proc = subprocess.run(
        command,
        cwd=REPO,
        capture_output=True,
        text=True,
        env={**os.environ, "RECALL_FIXTURE": str(fixture)},
    )
    parsed: dict = {}
    if proc.stdout.strip():
        try:
            parsed = json.loads(proc.stdout)
        except json.JSONDecodeError:
            parsed = {}
    return proc.returncode, parsed, proc.stderr


def test_the_recall_is_the_share_of_the_reached_tests_the_selection_names() -> None:
    """The selection names the test the complete run proves reaches the
    change: the recall is the whole truth."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        code, report, _ = measure(
            fixture,
            runner,
            [{"name": "the_test_that_watches", "file": "tests/it/checks.rs", "line": 2}],
        )
        check(code == 0, f"the measurement failed: {code}")
        check(
            report.get("reached") == 1,
            f"the execution reaches one test: {report.get('reached')}",
        )
        check(
            report.get("recall") == 1.0,
            f"the selection names the test that reaches the change: {report.get('recall')}",
        )
        check(
            report.get("precision") == 1.0,
            f"nothing extra is selected: {report.get('precision')}",
        )


def test_a_selection_that_names_nothing_reports_the_miss() -> None:
    """The number is not the whole answer: the test the selection failed
    to name is listed, because a miss is either a defect of the selection
    or a limit of the oracle, and only a human can say which."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        code, report, _ = measure(fixture, runner, [])
        check(code == 0, f"the measurement failed: {code}")
        check(
            report.get("recall") == 0.0,
            f"an empty selection has no recall against the truth: {report.get('recall')}",
        )
        misses = report.get("misses") or []
        check(
            len(misses) == 1 and misses[0]["name"] == "the_test_that_watches",
            f"the missed test is named: {misses}",
        )


def test_the_tree_is_left_as_it_was() -> None:
    """The measurement breaks a contract on purpose, and it puts the tree
    back: a measurement that leaves the tree modified is worse than no
    measurement."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        source = fixture / "src" / "lib.rs"
        before = source.read_bytes()
        measure(
            fixture,
            runner,
            [{"name": "the_test_that_watches", "file": "tests/it/checks.rs", "line": 2}],
        )
        check(
            source.read_bytes() == before,
            "the file whose contract was broken is restored byte for byte",
        )


def test_a_symbol_that_does_not_exist_is_an_error_and_not_a_number() -> None:
    """The measurement that cannot be made says so. A zero would be a
    figure, and a figure is believed."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        code, report, stderr = measure(fixture, runner, [], symbol="not_there")
        check(code != 0, f"the measurement of a symbol that does not exist fails: {code}")
        check(
            "not_there" in stderr,
            f"the error names the symbol that was not found: {stderr!r}",
        )
        check(
            not report,
            f"no report is printed when the measurement could not be made: {report}",
        )


def test_the_measurement_writes_the_artefact_the_receipt_cites() -> None:
    """The figure is cited with the SHA of the tree it was measured on, and
    the report is a file: a number in a journal with nothing behind it is a
    claim, not a measurement."""
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        runner = write_fixture(fixture)
        artefact = fixture / "recall.json"
        code, _, _ = measure(
            fixture,
            runner,
            [{"name": "the_test_that_watches", "file": "tests/it/checks.rs", "line": 2}],
            report=artefact,
        )
        check(code == 0, f"the measurement failed: {code}")
        check(artefact.exists(), "the report is written where it was asked for")
        if artefact.exists():
            written = json.loads(artefact.read_text(encoding="utf-8"))
            check(
                written.get("recall") == 1.0,
                f"the report carries the figure that was measured: {written}",
            )
            check(
                bool(written.get("head")),
                f"the report says the tree it was measured on: {written}",
            )


def test_the_measurement_stays_out_of_the_gate() -> None:
    """The measurement is asked for on purpose. A gate that runs it would
    pay its price on every batch, and the price is a complete run of the
    suite twice."""
    text = GATE.read_text(encoding="utf-8")
    check(
        "affected-tests-recall" not in text,
        "scripts/test-gate.sh does not run the recall measurement: it is asked for on purpose",
    )


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("PASS — la recall se mide, el fallo se nombra y el árbol queda como estaba")
    return 0


if __name__ == "__main__":
    sys.exit(main())
