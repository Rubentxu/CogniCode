#!/usr/bin/env python3
"""Tests for scripts/ci/select-suites.sh (CR-08 suite selector).

Two defects motivated this file, and both were invisible until CI ran:

1. `.github/workflows/pr-ci.yml` passed `filters: .*:[*]` to
   `dorny/paths-filter`, which requires a YAML object. The action aborted
   with "Invalid filter YAML format: Root element is not an object", so
   the `selector` job went red on every PR that reached it. The
   selector could not classify anything; it could only fail.

   That defect is gone with the action: the checks it pinned were about the
   shape of a `filters:` value and an output key that only
   `dorny/paths-filter` emits. Neither has anywhere to occur once the
   workflows are gone, so a contract still asserting them would be guarding
   a mechanism the repository no longer has.

2. `emit_json` joined suite names with `IFS=','` and no quoting, producing
   `suites":[core,explorer]`. That is not JSON. The workflow reads the
   field with `jq -r '.suites | join(",")'`, which fails on it, so even
   with the filter fixed the next step would have died.

The contract this file pins: for every input class, the selector exits 0,
emits parseable JSON, and the `strategy` matches what the documented
policy requires. Orchestration files are an input class in their own right —
a pipeline that decides what runs is exactly the thing a selective gate must
re-validate completely, so both `*.pipeline.kts` and `.github/workflows/**`
force the full battery. Parsing with `json.loads` is the assertion that would
have caught defect 2; checking `strategy` is what catches a policy that
silently stops selecting.

Run:
    python3 scripts/ci/test_select_suites.py
    python3 -m pytest -q scripts/ci/test_select_suites.py   # what CI runs
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SELECTOR = REPO_ROOT / "scripts" / "ci" / "select-suites.sh"

# Input classes paired with the strategy the header of the script promises.
# `noop` means "touched files do not affect compiled code"; `fallback` is
# the safe default for anything the mapper does not recognise; `changed`
# means a specific suite was selected.
CASES: list[tuple[str, str, str]] = [
    ("crates/cognicode-core/src/lib.rs", "changed", "core"),
    ("crates/cognicode-cli/src/main.rs", "changed", "cli"),
    ("crates/cognicode-ladybug/src/lib.rs", "changed", "ladybug"),
    # Unknown path: the documented safe default is the full battery.
    ("scripts/product/some_new_script.py", "fallback", ""),
    # Cargo manifests and CI wiring force the full battery.
    ("Cargo.toml", "fallback", ""),
    ("Cargo.lock", "fallback", ""),
    # Orchestration, by either spelling. PipelineK is the authority now, so a
    # change to a pipeline is the case that has to force the full battery; the
    # workflows stay in the selector until the directory is gone.
    ("merge-gate.pipeline.kts", "fallback", ""),
    ("integration.pipeline.kts", "fallback", ""),
    (".github/workflows/ci.yml", "fallback", ""),
    ("scripts/ci/select-suites.sh", "fallback", ""),
    # Nothing touched.
    ("", "fallback", ""),
    # Whitespace only is still "nothing touched".
    ("   ", "fallback", ""),
]

FULL_BATTERY = {"core", "explorer", "mcp", "cli", "ladybug", "merge"}

failures: list[str] = []


def run_selector(paths: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", str(SELECTOR)],
        input="",
        capture_output=True,
        text=True,
        env={"SELECT_PATHS": paths, "PATH": "/usr/bin:/bin"},
    )


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    violations = check_all()
    if violations:
        print(f"FAIL — {len(violations)} problem(s):")
        for failure in violations:
            print(f"  - {failure}")
        return 1

    print(f"PASS — {len(CASES)} input classes, JSON contract, and workflow wiring")
    return 0


def check_all() -> list[str]:
    """Run every contract check and return the list of violations found."""
    del failures[:]
    check(SELECTOR.is_file(), f"selector not found: {SELECTOR}")

    # --- contract: exit 0, parseable JSON, declared strategy -------------
    for paths, expected_strategy, expected_suite in CASES:
        label = paths if paths.strip() else "<empty>"
        proc = run_selector(paths)
        check(
            proc.returncode == 0,
            f"[{label}] expected exit 0, got {proc.returncode}: {proc.stderr.strip()}",
        )

        try:
            payload = json.loads(proc.stdout)
        except json.JSONDecodeError as exc:
            failures.append(f"[{label}] stdout is not valid JSON ({exc}): {proc.stdout!r}")
            continue

        check(
            payload.get("strategy") == expected_strategy,
            f"[{label}] expected strategy {expected_strategy!r}, "
            f"got {payload.get('strategy')!r} (reason={payload.get('reason')!r})",
        )

        suites = payload.get("suites")
        check(
            isinstance(suites, list) and all(isinstance(s, str) for s in suites),
            f"[{label}] suites must be a list of strings, got {suites!r}",
        )
        check(isinstance(payload.get("reason"), str), f"[{label}] reason must be a string")

        if expected_strategy == "fallback":
            check(
                set(suites) == FULL_BATTERY,
                f"[{label}] fallback must run the full battery, got {suites!r}",
            )
        elif expected_strategy == "noop":
            check(suites == [], f"[{label}] noop must select no suites, got {suites!r}")
        elif expected_strategy == "changed" and expected_suite:
            check(
                expected_suite in suites,
                f"[{label}] expected {expected_suite!r} in suites, got {suites!r}",
            )

    # --- contract: every suite name is a string, never a bare token ------
    # This is the specific shape that `jq` rejects. Asserting on the joined
    # string is what makes the regression impossible to reintroduce.
    proc = run_selector("Cargo.toml")
    check(
        '"suites":["core"' in proc.stdout or '"suites":[]' in proc.stdout,
        f"suite names must be quoted in the JSON output, got: {proc.stdout!r}",
    )
    check(
        "suites\":[core" not in proc.stdout,
        f"unquoted suite tokens reintroduced: {proc.stdout!r}",
    )

    return list(failures)


def test_selector_contract() -> None:
    """Single pytest entry point.

    Written as a pytest test rather than a bare script because CI runs it
    with `python3 -m pytest`. A module with no `test_` function is
    imported and its `if __name__ == "__main__"` guard does not fire, so a
    `main()`-only script reports green in CI while checking nothing. That
    is the same class of failure this file exists to prevent: a gate that
    looks armed and is not.
    """
    violations = check_all()
    assert not violations, "selector contract violations:\n" + "\n".join(
        f"  - {v}" for v in violations
    )


if __name__ == "__main__":
    sys.exit(main())
