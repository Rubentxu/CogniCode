#!/usr/bin/env python3
"""The Explorer browser suite waits on facts, not on clocks.

Why this exists
---------------
The Explorer E2E suite used to answer "is the application ready yet?" with
`page.waitForTimeout(1500)` — fifty-nine times, across twenty-four spec
files. A clock is a guess about how long the application takes, and it is
guessed wrong twice: it is too short when the machine is loaded, and it is
too long every single time, because the wait does not end when the thing
happens. It ends when the number expires.

The fix was not "make the number bigger". The application already knew the
answer — the global shortcut was registered — and the suite had no way to
ask. So the application now publishes that fact (an attribute on `<html>`
that is present exactly while the listener is registered) and the suite
waits for the fact.

What this guard asserts
-----------------------
No file under `apps/explorer-ui/e2e/` may call `waitForTimeout`. Every
waits in that suite has to name the condition it is waiting for.

The alternative to this guard is a review convention. A review convention
is checked by whoever happens to read the diff, and it fails in the
direction that matters: the next spec that needs to wait for something
new copies the nearest clock it can find, and the suite slowly returns to
the shape it had.

What it does not do
-------------------
It does not run the browser suite. No pipeline in this repository installs
or drives a browser or a dev server, so the specs stay as reference
material and are not presented as a guarantee. This guard asserts a
property of the source, which is what can be asserted here.

Run:
    python3 scripts/ci/test_explorer_e2e_waits_on_conditions.py
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# The tree this guard is about. Every TypeScript file in it is checked: the
# specs are where the waits were, and a helper is where the next one would
# arrive.
E2E_TREE = REPO_ROOT / "apps" / "explorer-ui" / "e2e"

# The forbidden call. `waitForTimeout` in Playwright always takes a fixed
# number of milliseconds, so every occurrence of it is a clock wait — there
# is no variant of it that means "wait for a condition".
FORBIDDEN = "waitForTimeout"

# The runner that discovers the contracts, and the glob it discovers them by.
RUNNER = "scripts/ci/run-all-contracts.sh"
DISCOVERY_GLOB = "scripts/ci/test_*.py"
SELF_NAME = "test_explorer_e2e_waits_on_conditions.py"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    """Record a finding. A check that fails is data, not an exception."""
    if not condition:
        failures.append(message)


def typescript_files() -> list[Path]:
    """Every TypeScript file of the Explorer browser suite."""
    if not E2E_TREE.is_dir():
        return []
    return sorted(path for path in E2E_TREE.rglob("*.ts") if path.is_file())


def specs() -> list[Path]:
    """Every spec of the Explorer browser suite."""
    return sorted(path for path in E2E_TREE.rglob("*.spec.ts") if path.is_file())


def test_the_explorer_suite_has_specs() -> None:
    """A guard over an empty tree would pass without checking anything."""
    found = specs()
    check(
        found,
        f"{E2E_TREE.relative_to(REPO_ROOT)} holds no spec file. The tree this "
        f"guard is about is gone or moved, and the guard is now measuring "
        f"nothing",
    )


def test_no_explorer_spec_waits_on_a_clock() -> None:
    """No spec may answer "is the application ready?" with a clock."""
    offenders: list[str] = []
    for path in typescript_files():
        text = path.read_text(encoding="utf-8")
        for number, line in enumerate(text.splitlines(), start=1):
            if FORBIDDEN not in line:
                continue
            relative = path.relative_to(REPO_ROOT)
            offenders.append(f"{relative}:{number}: {line.strip()}")
    check(
        not offenders,
        "the Explorer browser suite waits on a clock again. Each of these "
        "waits has to name the condition it waits for instead:\n  "
        + "\n  ".join(offenders),
    )


def test_the_signal_the_suite_waits_for_is_owned_by_one_module() -> None:
    """The attribute the suite reads has exactly one owner."""
    owner = REPO_ROOT / "apps" / "explorer-ui" / "src" / "components" / "Spotter" / "globalShortcut.ts"
    check(
        owner.is_file(),
        f"{owner.relative_to(REPO_ROOT)} is missing. The Explorer suite "
        f"waits for a signal that the application publishes; without the "
        f"module that publishes it, the wait has no subject",
    )


def test_the_guard_is_run_by_the_merge_authority() -> None:
    """A guard nothing runs is correct, tested, and invisible."""
    sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
    try:
        import pipeline_authority as authority
    except ImportError:
        check(False, "scripts/ci/pipeline_authority.py could not be imported")
        return

    hosts = authority.invoked_by(RUNNER)
    check(
        authority.MERGE_AUTHORITY in hosts,
        f"{RUNNER} is not invoked by {authority.MERGE_AUTHORITY} "
        f"(found in: {sorted(hosts) or 'no pipeline at all'}). A guard that "
        f"nothing runs is the CP5 failure again: correct, tested, and invisible",
    )

    matched = sorted(REPO_ROOT.glob(DISCOVERY_GLOB))
    check(
        SELF_NAME in [p.name for p in matched],
        f"{SELF_NAME} no longer matches {DISCOVERY_GLOB}, so the runner never "
        f"discovers it. Renaming or moving this file without touching the glob "
        f"silently stops it from running",
    )

    runner_text = (REPO_ROOT / RUNNER).read_text(encoding="utf-8")
    check(
        "test_*.py" in runner_text,
        f"{RUNNER} no longer discovers contracts by glob. It runs whatever it "
        f"is handed, which puts the contract list back into a second place",
    )


def main() -> int:
    for name, function in sorted(globals().items()):
        if name.startswith("test_") and callable(function):
            try:
                function()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    for finding in failures:
        print(f"  FAIL {finding}")
    if failures:
        print(f"explorer e2e waits on conditions: {len(failures)} finding(s)")
        return 1
    print("explorer e2e waits on conditions: none")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
