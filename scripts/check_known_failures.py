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

Exit codes: 0 = matches baseline, 1 = drift, 2 = harness error.
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


def run_tests(package: str, target: str) -> str:
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
    return proc.stdout + "\n" + proc.stderr


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


def write_baseline(path: Path, package: str, target: str, tests: list[str]) -> None:
    today = _dt.date.today().isoformat()
    out = [
        "# Known-failure baseline — machine-readable",
        "#",
        "# Regenerate with: python3 scripts/check_known_failures.py --update",
        "#",
        "# Contract: a run must match this set EXACTLY. New failures or",
        "# unexpectedly-fixed tests fail the check so regressions cannot hide.",
        "#",
        "# Scope: environment-scoped; do not copy across runners with a",
        "# different workspace root.",
        "",
        f"package: {package}",
        f"target: {target}",
        f"generated: {today}",
        "entries:",
    ]
    for test in tests:
        out += [
            f"  - test: {test}",
            "    category: environment_dependency",
            f"    first_seen: {today}",
            "    reason: environment/cwd dependency (review before removing)",
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
    output = run_tests(package, target)
    observed = observed_failures(output)

    if args.update:
        write_baseline(baseline_path, package, target, sorted(observed))
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
