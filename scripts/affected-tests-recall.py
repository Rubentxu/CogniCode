#!/usr/bin/env python3
"""The recall of the published test selection, measured against a complete run.

WHAT THIS ANSWERS
-----------------
The product publishes, next to every impact answer, the list of tests that
reach the changed symbol (`affected_tests`). That list is what a developer is
told to run before shipping a change. This script measures how much of the
truth that list carries.

The two sides are measured against each other, and each side has to come
from somewhere that does not depend on the other:

    SELECTION  what the product publishes. It is read from the product
               itself: `cognicode --format json graph impact <symbol>
               <dir>` and the `affected_tests` of that document.

    EXECUTION  what the whole suite says. The package's ENTIRE test suite
               is run twice: once as it is, and once with the symbol's
               contract broken -- its body panics before it does anything.
               Every test that fails only in the second run reaches the
               change. The difference between the two runs is the truth;
               the tests that were already failing are subtracted, so a
               suite that was red before says nothing here.

    RECALL     of the selection against the execution: of the tests that
               the complete run proves reach the change, how many did the
               product name.

WHY THE ORACLE IS A BROKEN CONTRACT AND NOT A CHANGED VALUE
------------------------------------------------------------
A mutation that changes a returned value only fails the tests that ASSERT
on that value, so it under-reports: a test that calls the function and
ignores the result does reach the change and would not be counted. A
mutation that panics fails every caller, asserted or not, so it measures
what the question is actually about: which tests reach the change.

THE LIMITS ARE PART OF THE ANSWER
----------------------------------
The oracle sees the calls the runtime makes. A test that reaches the
change through a path the graph does not carry (a dynamic call, a macro,
a trait object) is counted as a miss, and this report says so instead of
hiding it. Read the misses before the number.

This is a MEASUREMENT, not a gate. It is asked for on purpose, it lives
outside the suite and outside the gate, and the figure it prints is
recorded in the journal next to the SHA of the tree it was measured on.

Usage:
    scripts/affected-tests-recall.py --package cognicode-core --symbol impact_of
    scripts/affected-tests-recall.py --package p --symbol s --json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]

# A failing test, as nextest prints it:
#     FAIL [   0.005s] ( 471/3606) cognicode-core application::services::x::tests::y
#
# The counter between the time and the binary is the progress of the run,
# and it is optional: it is there when the runner prints the progress and
# gone when it does not.
FAIL_LINE = re.compile(
    r"^\s*(?:TRY\s+\d+\s+)?FAIL\s+\[\s*[\d.]+s\]\s*(?:\(\s*\d+/\d+\)\s*)?(\S+)\s+(\S+)\s*$"
)

# The declaration of a function: `fn name`, `pub fn name`, `async fn name`.
# The name is followed by `(` or `<` so that `fn name_of` does not match
# `fn name`.
def declaration_pattern(symbol: str) -> re.Pattern[str]:
    return re.compile(r"\bfn\s+" + re.escape(symbol) + r"\s*[<(]")


class MeasurementError(Exception):
    """The measurement could not be made.

    A measurement that cannot be made is not a measurement with a bad
    number; it is no number at all, and it is reported as an error.
    """


# ---------------------------------------------------------------------------
# The package and the symbol.
# ---------------------------------------------------------------------------


def package_dir_of(package: str, manifest_path: Path | None) -> Path:
    """The directory of the cargo package, as cargo says it."""
    cmd = ["cargo", "metadata", "--no-deps", "--format-version", "1"]
    if manifest_path is not None:
        cmd += ["--manifest-path", str(manifest_path)]
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if proc.returncode != 0:
        raise MeasurementError(f"cargo metadata failed: {proc.stderr.strip()}")

    for entry in json.loads(proc.stdout).get("packages", []):
        if entry.get("name") == package:
            return Path(entry["manifest_path"]).parent.resolve()
    known = sorted(
        p.get("name", "?")
        for p in json.loads(proc.stdout).get("packages", [])
    )
    raise MeasurementError(
        f"the package {package!r} is not in this workspace: {', '.join(known)}"
    )


def source_files_of(package_dir: Path) -> list[Path]:
    """The Rust sources of a package, production code first."""
    sources = sorted((package_dir / "src").rglob("*.rs")) if (package_dir / "src").is_dir() else []
    tests = sorted((package_dir / "tests").rglob("*.rs")) if (package_dir / "tests").is_dir() else []
    return sources + tests


def locate_symbol(
    package_dir: Path, symbol: str, explicit_file: str | None
) -> tuple[Path, int]:
    """The file that declares `symbol`, and where its body opens.

    The declaration is found by its own name, in the production code first:
    the contract that is broken has to be the contract of the product, not
    the contract of a test.
    """
    pattern = declaration_pattern(symbol)
    candidates: list[Path] = []

    for path in source_files_of(package_dir):
        if explicit_file is not None and str(path) != str((package_dir / explicit_file).resolve()):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        if declares(text, pattern):
            candidates.append(path)

    if not candidates:
        where = f" in {explicit_file}" if explicit_file else ""
        raise MeasurementError(f"no declaration of {symbol!r} found{where} in {package_dir}")

    files = set(candidates)
    if len(files) > 1:
        listed = ", ".join(sorted(str(p.relative_to(package_dir)) for p in files))
        raise MeasurementError(
            f"{symbol!r} is declared in several files ({listed}): name the one with --file"
        )

    path = candidates[0]
    text = path.read_text(encoding="utf-8", errors="replace")
    match = pattern.search(text)
    if match is None:
        raise MeasurementError(f"the declaration of {symbol!r} disappeared from {path}")
    return path, body_open_brace(text, match.start(), symbol)


def declares(text: str, pattern: re.Pattern[str]) -> bool:
    """Whether the text declares the function the pattern looks for.

    A declaration inside a comment is not a declaration: the comment
    says what the code once did, and the mutation would break nothing.
    """
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//") or stripped.startswith("*"):
            continue
        if pattern.search(line):
            return True
    return False


def body_open_brace(text: str, fn_at: int, symbol: str) -> int:
    """Where the body of the declaration that starts at `fn_at` opens.

    The scan counts the parentheses and the brackets, so a brace that
    belongs to a closure in an argument is not mistaken for the body.
    """
    depth = 0
    index = fn_at
    while index < len(text):
        char = text[index]
        if char in "([":
            depth += 1
        elif char in ")]":
            depth -= 1
        elif char == "{":
            if depth == 0:
                return index
            depth += 1
        elif char == "}":
            if depth == 0:
                raise MeasurementError(f"the body of {symbol!r} does not open")
            depth -= 1
        index += 1
    raise MeasurementError(f"the body of {symbol!r} does not open")


def mutation_of(symbol: str) -> str:
    """The text that breaks the contract of a symbol.

    The panic goes first, before anything the body does, so every caller
    fails: a test that reaches the change is a test that fails, whether it
    asserts on the result or not.
    """
    return f'\n    panic!("affected-tests-recall: the contract of {symbol} is broken");'


# ---------------------------------------------------------------------------
# The selection: what the product publishes.
# ---------------------------------------------------------------------------


def read_selection(args: argparse.Namespace, package_dir: Path) -> list[dict]:
    """The `affected_tests` the product publishes for the symbol."""
    if args.selection is not None:
        document = json.loads(Path(args.selection).read_text(encoding="utf-8"))
    else:
        cmd = [
            "cargo",
            "run",
            "--quiet",
            "-p",
            "cognicode-cli",
            "--bin",
            "cognicode",
            "--",
            "--format",
            "json",
            "graph",
            "impact",
            args.symbol,
            str(package_dir),
        ]
        proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
        if proc.returncode != 0:
            raise MeasurementError(
                f"the product could not answer the impact of {args.symbol!r}: "
                f"{proc.stderr.strip()[-800:]}"
            )
        try:
            document = json.loads(proc.stdout)
        except json.JSONDecodeError as error:
            raise MeasurementError(
                f"the product did not answer in JSON ({error}); "
                f"stdout was: {proc.stdout[:400]!r}"
            ) from error

    tests = document.get("affected_tests")
    if tests is None:
        raise MeasurementError(
            "the answer of the product carries no `affected_tests`: "
            "the selection cannot be measured"
        )
    return tests


def relative_to_package(path: str, package_dir: Path) -> str:
    """The path of a file, as the package sees it."""
    candidate = Path(path)
    if candidate.is_absolute():
        try:
            return str(candidate.resolve().relative_to(package_dir))
        except ValueError:
            return str(candidate)
    text = str(candidate).replace("\\", "/")
    prefix = f"{package_dir.name}/"
    if text.startswith(prefix):
        return text[len(prefix) :]
    return text


# ---------------------------------------------------------------------------
# The execution: what the whole suite says.
# ---------------------------------------------------------------------------


def runner_command(args: argparse.Namespace) -> list[str]:
    """The command that runs the suite of the package."""
    if args.runner:
        return [args.runner]
    return ["cargo", "nextest", "run"]


def run_suite(args: argparse.Namespace, package: str) -> set[str]:
    """The tests that fail in a complete run of the package's suite.

    The runner writes what it did to STDERR —the same channel cargo uses—,
    and its stdout stays empty. Both are read, because a runner that
    writes its verdict somewhere else would then be invisible, and a
    measurement that sees no failures would report a recall of nothing
    instead of saying it could not look.
    """
    cmd = runner_command(args) + ["-p", package, "--no-fail-fast", "--color", "never"]
    if args.runner_args:
        cmd += args.runner_args
    proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
    if "could not compile" in proc.stderr or "error[E" in proc.stderr:
        raise MeasurementError(f"the suite did not build: {proc.stderr.strip()[-800:]}")
    lines = proc.stdout.splitlines() + proc.stderr.splitlines()
    return {
        match.group(2) for match in (FAIL_LINE.match(line) for line in lines) if match
    }


def file_of_tests(package_dir: Path) -> dict[str, str]:
    """The file each test name is declared in, as the package sees it.

    The name is the last segment of the test id nextest prints, and this
    is where that name is declared. A name declared in two files keeps the
    first one: the report says which file it used.
    """
    where: dict[str, str] = {}
    for path in source_files_of(package_dir):
        relative = str(path.relative_to(package_dir))
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in re.finditer(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", text):
            where.setdefault(match.group(1), relative)
    return where


def reached_by_mutation(
    args: argparse.Namespace, package: str, package_dir: Path, path: Path, body_at: int
) -> set[str]:
    """The tests that reach the change, measured by breaking its contract.

    The file is restored byte for byte, and the restoration is verified:
    a measurement that leaves the tree modified is worse than no
    measurement at all.
    """
    original = path.read_bytes()
    digest = hashlib.sha256(original).hexdigest()
    text = original.decode("utf-8", errors="replace")
    mutated = text[: body_at + 1] + mutation_of(args.symbol) + text[body_at + 1 :]

    baseline = run_suite(args, package)
    try:
        path.write_text(mutated, encoding="utf-8")
        broken = run_suite(args, package)
    finally:
        path.write_bytes(original)
        restored = path.read_bytes()
        if hashlib.sha256(restored).hexdigest() != digest:
            raise MeasurementError(
                f"the file {path} was not restored byte for byte: the tree is modified"
            )

    return broken - baseline


# ---------------------------------------------------------------------------
# The comparison.
# ---------------------------------------------------------------------------


def key_of_test(name: str, file: str | None) -> str:
    return f"{file}::{name}" if file else name


def compare(
    selection: list[dict],
    reached: set[str],
    where: dict[str, str],
    package_dir: Path,
) -> dict:
    """The recall and the precision of the selection, and what is missing."""
    selected: dict[str, str] = {}
    for entry in selection:
        name = entry.get("name", "")
        file = relative_to_package(entry.get("file", ""), package_dir)
        selected[name] = file

    reached_by_name: dict[str, str] = {}
    for test_id in sorted(reached):
        name = test_id.rsplit("::", 1)[-1]
        reached_by_name[name] = where.get(name, "")

    matched = [name for name in reached_by_name if name in selected]
    misses = [
        {"name": name, "file": reached_by_name[name]}
        for name in sorted(reached_by_name)
        if name not in selected
    ]
    extras = [
        {"name": name, "file": selected[name]}
        for name in sorted(selected)
        if name not in reached_by_name
    ]

    recall = len(matched) / len(reached_by_name) if reached_by_name else None
    precision = len(matched) / len(selected) if selected else None

    return {
        "selected": len(selected),
        "reached": len(reached_by_name),
        "matched": len(matched),
        "recall": recall,
        "precision": precision,
        "misses": misses,
        "extras": extras,
        "selected_files": sorted({file for file in selected.values()}),
        "reached_files": sorted({file for file in reached_by_name.values() if file}),
        "keys": {
            "selected": sorted(key_of_test(n, selected[n]) for n in selected),
            "reached": sorted(
                key_of_test(n, reached_by_name[n]) for n in reached_by_name
            ),
        },
    }


# ---------------------------------------------------------------------------
# The report.
# ---------------------------------------------------------------------------


def head_sha() -> str:
    proc = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, capture_output=True, text=True
    )
    return proc.stdout.strip() if proc.returncode == 0 else "unknown"


def tree_is_dirty() -> bool:
    """Whether the tracked tree is modified.

    The untracked files do not count: a file that nobody committed is not a
    change to the code that was measured, and a measurement that calls a
    dirty tree dirty because of an untracked file says nothing useful.
    """
    proc = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=no"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    return bool(proc.stdout.strip())


def emit(report: dict, args: argparse.Namespace) -> None:
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
        return

    print(f"symbol            {report['symbol']}")
    print(f"package           {report['package']} ({report['package_dir']})")
    print(f"measured on       {report['head']}"
          f"{' (tree modified)' if report['dirty'] else ''}")
    print(f"selected          {report['selected']} test(s)")
    print(f"reached           {report['reached']} test(s)")
    print(f"matched           {report['matched']} test(s)")
    if report["recall"] is None:
        print("recall            n/a — no test reaches the change")
    else:
        print(f"recall            {report['recall'] * 100:.1f}%")
    if report["precision"] is None:
        print("precision         n/a — the selection is empty")
    else:
        print(f"precision         {report['precision'] * 100:.1f}%")

    if report["misses"]:
        print("")
        print("missed (the execution reaches them, the selection does not name them):")
        for miss in report["misses"]:
            print(f"  - {miss['name']}  ({miss['file']})")
    if report["extras"]:
        print("")
        print("extra (the selection names them, the execution does not reach them):")
        for extra in report["extras"]:
            print(f"  - {extra['name']}  ({extra['file']})")

    print("")
    print(
        "The oracle is a broken contract: it sees the calls the runtime makes. "
        "A test that reaches the change through a path the graph does not carry "
        "is counted as a miss."
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Measure the recall of the published test selection against a complete run."
    )
    parser.add_argument("--package", required=True, help="the cargo package to measure")
    parser.add_argument("--symbol", required=True, help="the symbol whose selection is measured")
    parser.add_argument(
        "--file", default=None, help="the file that declares the symbol, when it is ambiguous"
    )
    parser.add_argument(
        "--manifest-path", default=None, help="the manifest of the workspace to measure"
    )
    parser.add_argument(
        "--runner",
        default=None,
        help="the command that runs the suite (default: cargo nextest run)",
    )
    parser.add_argument(
        "--runner-args", nargs="*", default=[], help="extra arguments for the runner"
    )
    parser.add_argument(
        "--selection",
        default=None,
        help="a JSON document with the selection, instead of asking the product",
    )
    parser.add_argument("--json", action="store_true", help="the report as JSON")
    parser.add_argument(
        "--report",
        default=None,
        help="a file to write the JSON report to, as the artefact the figure cites",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])

    try:
        package_dir = package_dir_of(args.package, args.manifest_path)
        path, body_at = locate_symbol(package_dir, args.symbol, args.file)
        selection = read_selection(args, package_dir)
        reached = reached_by_mutation(
            args, args.package, package_dir, path, body_at
        )
        where = file_of_tests(package_dir)
        comparison = compare(selection, reached, where, package_dir)
    except MeasurementError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    report = {
        "symbol": args.symbol,
        "package": args.package,
        "package_dir": str(package_dir),
        "mutated_file": str(path.relative_to(package_dir)),
        "head": head_sha(),
        "dirty": tree_is_dirty(),
        **comparison,
    }
    if args.report is not None:
        Path(args.report).write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    emit(report, args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
