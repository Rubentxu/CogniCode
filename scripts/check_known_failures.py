#!/usr/bin/env python3
"""check_known_failures.py — guard against known-failure noise.

Runs a test target, compares its FAILED set against
``scripts/known_failures.yaml``, and fails when the set changes:

  * a test fails that is NOT in the baseline  -> NEW REGRESSION  (exit 1)
  * a baseline test PASSES                    -> UNEXPECTEDLY FIXED (exit 1)

A clean run means the observed failure set is *exactly* the baseline, so a
new regression can never hide inside the "usual red".

Usage::

    python3 scripts/check_known_failures.py               # check
    python3 scripts/check_known_failures.py --update      # regenerate baseline
    python3 scripts/check_known_failures.py --package cognicode-core --target lib

Exit codes: 0 = matches baseline, 1 = drift, 2 = harness error (the suite did
not run: compile failure / missing binary). ``--update`` is refused on a
harness error so a broken build cannot wipe the baseline.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BASELINE = REPO_ROOT / "scripts" / "known_failures.yaml"
FAILED_RE = re.compile(r"^test (\S+) \.\.\. FAILED$", re.MULTILINE)


def run_tests(package: str, target: str) -> tuple[int, str]:
    cmd = ["cargo", "test", "-p", package]
    if target == "lib":
        cmd.append("--lib")
    elif target:
        cmd += ["--test", target]
    proc = subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    # cargo writes test results to stdout; keep both to be safe.
    return proc.returncode, proc.stdout + "\n" + proc.stderr


def is_harness_failure(output: str) -> bool:
    """True when the suite never ran (compile error, missing binary, panic in
    the harness). Such a run must never be treated as "zero failures"."""
    if "error: could not compile" in output or "error: could not find" in output:
        return True
    if re.search(r"^error\[E\d+\]", output, re.MULTILINE):
        return True
    if re.search(r"^error: ", output, re.MULTILINE) and "test result:" not in output:
        return True
    if "test result:" not in output:
        return True
    return False


def observed_failures(output: str) -> set[str]:
    return set(FAILED_RE.findall(output))


def load_baseline(path: Path) -> dict:
    """Parse the tiny YAML by hand (no PyYAML dependency)."""
    if not path.exists():
        return {"package": "cognicode-core", "target": "lib", "entries": []}
    package, target = "cognicode-core", "lib"
    entries: list[dict] = []
    current: dict | None = None
    for raw in path.read_text().splitlines():
        line = raw.rstrip()
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped.startswith("package:"):
            package = stripped.split(":", 1)[1].strip()
        elif stripped.startswith("target:"):
            target = stripped.split(":", 1)[1].strip()
        elif stripped.startswith("- test:"):
            current = {"test": stripped.split(":", 1)[1].strip()}
            entries.append(current)
        elif current is not None and ":" in stripped:
            key, value = stripped.split(":", 1)
            current[key.strip()] = value.strip()
    return {"package": package, "target": target, "entries": entries}


def write_baseline(
    path: Path,
    package: str,
    target: str,
    tests: list[str],
    previous: dict[str, dict] | None = None,
) -> None:
    """Rewrite the baseline, preserving category/first_seen/reason for entries
    that already existed (so --update does not erase provenance)."""
    previous = previous or {}
    today = _dt.date.today().isoformat()
    out = [
        "# Known-failure baseline — cognicode-core lib tests",
        "#",
        "# Machine-readable list of tests that fail in this environment for",
        "# reasons OUTSIDE the code under review (see `reason`). Maintained so a",
        "# new regression is never lost in the noise:",
        "#",
        "#   python3 scripts/check_known_failures.py",
        "#",
        "# Contract: a run must match this set EXACTLY. The checker fails if",
        "#   - a test fails that is NOT in this baseline (new regression), or",
        "#   - a baseline test PASSES (unexpectedly fixed; regenerate the baseline).",
        "#",
        "# Scope: environment-scoped; do not copy across CI runners with a",
        "# different workspace root.",
        "",
        f"package: {package}",
        f"target: {target}",
        f"generated: {today}",
        "entries:",
    ]
    for test in tests:
        prior = previous.get(test)
        if prior:
            category = prior.get("category", "environment_dependency")
            first_seen = prior.get("first_seen", today)
            reason = prior.get("reason", "environment/cwd dependency (review before removing)")
        else:
            category = "environment_dependency"
            first_seen = today
            reason = "environment/cwd dependency (review before removing)"
        out += [
            f"  - test: {test}",
            f"    category: {category}",
            f"    first_seen: {first_seen}",
            f"    reason: {reason}",
        ]
    path.write_text("\n".join(out) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", default=None)
    parser.add_argument("--target", default=None)
    parser.add_argument("--baseline", default=str(DEFAULT_BASELINE))
    parser.add_argument("--update", action="store_true")
    args = parser.parse_args()

    baseline_path = Path(args.baseline)
    baseline = load_baseline(baseline_path)
    package = args.package or baseline.get("package") or "cognicode-core"
    target = args.target or baseline.get("target") or "lib"

    print(f"[known-failures] running `cargo test -p {package}` (target={target}) …")
    returncode, output = run_tests(package, target)

    if is_harness_failure(output):
        print(
            "[known-failures] HARNESS ERROR: the test suite did not run "
            "(compile error or missing binary); refusing to compare or update.",
            file=sys.stderr,
        )
        tail = "\n".join(output.splitlines()[-15:])
        print(tail, file=sys.stderr)
        return 2

    observed = observed_failures(output)

    if args.update:
        previous = {e["test"]: e for e in baseline["entries"]}
        write_baseline(baseline_path, package, target, sorted(observed), previous)
        print(f"[known-failures] baseline updated: {len(observed)} entries -> {baseline_path}")
        return 0

    known = {e["test"] for e in baseline["entries"]}
    new = sorted(observed - known)
    missing = sorted(known - observed)

    if not new and not missing:
        print(f"[known-failures] OK: observed failure set matches baseline ({len(known)} entries)")
        return 0

    print("[known-failures] DRIFT detected", file=sys.stderr)
    if new:
        print(f"\n  NEW REGRESSIONS ({len(new)}):", file=sys.stderr)
        for t in new:
            print(f"    + {t}", file=sys.stderr)
    if missing:
        print(
            f"\n  UNEXPECTEDLY FIXED ({len(missing)}) — remove from the baseline:",
            file=sys.stderr,
        )
        for t in missing:
            print(f"    - {t}", file=sys.stderr)
    print(
        "\nIf these are genuinely expected, regenerate with --update and review the diff.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
