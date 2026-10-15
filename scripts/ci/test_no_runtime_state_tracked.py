#!/usr/bin/env python3
"""No PipelineK run state is tracked in git.

Measured 2026-10-15. A run launched with `--control-root "$PWD"` instead of the
default `<workspace>/.pipelinek` writes its control state into the working
tree: `leases/`, `last-run/`, `output-plane/`, `retry-control/`,
`wait-until-control/`. A `cargo fmt --all` commit in this repository's history
swept 32 of those files into version control, and they have been tracked ever
since.

Nothing notices. `git status` shows the *new* ones as untracked noise and the
tracked ones as modified on every run, and a reviewer reads that churn as
someone else's WIP. Meanwhile two of the tracked files are `*.lock` files —
a lock whose purpose is to vanish with the process holding it, preserved in a
release tarball — and the `output-plane/streams/` set is per-run stdout, which
is exactly the kind of content that should never be published.

The oracle is `git ls-files`, because that is what names the content of a
release tarball. A directory can be absent from the working tree and still be
tracked; only the index knows.

Run:
    python3 scripts/ci/test_no_runtime_state_tracked.py
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# Written by the runtime into the control root. `.pipelinek/` is already
# ignored and is the default location; these five are what appears when the
# control root is pointed at the repository instead.
CONTROL_ROOT_DIRS = (
    "leases",
    "last-run",
    "output-plane",
    "retry-control",
    "wait-until-control",
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def tracked_files() -> list[str]:
    proc = subprocess.run(
        ["git", "ls-files", "-z", "--", *CONTROL_ROOT_DIRS],
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
        check=False,
    )
    if proc.returncode != 0:
        failures.append(
            f"git ls-files failed: {proc.stderr.strip()[:200]}. This contract "
            f"cannot tell what is tracked, so it must not report a pass."
        )
        return []
    return [name for name in proc.stdout.split("\0") if name]


def test_no_control_root_state_is_tracked() -> None:
    tracked = tracked_files()
    if tracked:
        failures.append(
            f"{len(tracked)} PipelineK control-root file(s) are tracked and "
            f"would ship in a release tarball:\n    "
            + "\n    ".join(tracked[:12])
            + ("\n    ..." if len(tracked) > 12 else "")
            + "\nThey are per-run scratch. Untrack them with "
            "`git rm -r --cached` and keep them out with .gitignore; the "
            "control root belongs at `.pipelinek`, not at the repository root."
        )


def test_the_control_root_state_is_ignored() -> None:
    """Untracked is not enough: the next run recreates them."""
    ignored = [
        d
        for d in CONTROL_ROOT_DIRS
        if subprocess.run(
            ["git", "check-ignore", "-q", f"{d}/probe"],
            cwd=REPO_ROOT,
            check=False,
        ).returncode
        == 0
    ]
    missing = [d for d in CONTROL_ROOT_DIRS if d not in ignored]
    check(
        not missing,
        f"{missing} are not in .gitignore. Untracking them fixes this commit; "
        f"without an ignore rule the next pipeline run puts them straight back "
        f"into `git status` as untracked noise.",
    )


def main() -> int:
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

    print(
        "PASS — no PipelineK control-root state is tracked, and all five "
        "directories are ignored so a run cannot put it back"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())