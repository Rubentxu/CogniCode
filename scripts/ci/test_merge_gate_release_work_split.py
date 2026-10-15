#!/usr/bin/env python3
"""A merge gate proves a merge. A release lane proves a release. Neither does both.

MEDIDO 2026-10-15, con el operador de acuerdo.

The merge gate had grown to 73 `stage(` occurrences. Collapsing 65 executable
stages into 10 left a ~1900 s run, and two stages were 1170 s of it:

    build   ~700 s   four release-profile binaries
    docs    ~472 s   cargo doc --workspace --no-deps

That is 62% of the gate spent producing artefacts a merge does not publish.
Everything else it checks — fmt, clippy, 276 contracts, 39 test suites,
advisories, licences, LSP, selector, drift — fits in 730 s.

So the release work moved to the lane that actually cuts a release.

## What this costs, stated plainly

A merge no longer proves that the release binaries build. That is proved in
`release-candidate.pipeline.kts` instead, so a broken release build surfaces
when preparing to publish rather than when merging. One class of failure is
detected later. Nothing else changes.

`cognicode-control-plane` is a host binary. `release-candidate` cross-compiles
cogh, cognicode and cognicode-mcp per target; the control plane was built
*only* in the merge gate. Moving `build` wholesale would have left it built by
no lane at all, so this contract also pins that the release lane builds it.

## Why this contract exists

A contract that only checked "the gate is fast" would be satisfied by deleting
the work. It checks both directions: the gate does not do release work, and
the release lane does not lose it.

Run:
    python3 scripts/ci/test_merge_gate_release_work_split.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Sibling contract, same directory. Imported, not copied: a second
# implementation would be a second source of truth for the same rule.
from test_candidate_packaging import strip_comments

REPO_ROOT = Path(__file__).resolve().parents[2]
MERGE_GATE = REPO_ROOT / "merge-gate.pipeline.kts"
RELEASE_LANE = REPO_ROOT / "release-candidate.pipeline.kts"

# What a merge must not build, and what a release must.
RELEASE_ONLY_IN_GATE = (
    (r"cargo\s+build\s+--release", "a release-profile build"),
    (r"cargo\s+doc\s+--workspace", "the workspace documentation build"),
)
REQUIRED_IN_LANE = (
    ("host-build-and-verify", "the host binary build"),
    ("public-docs", "the zero-warning documentation build"),
    ("cognicode-control-plane", "the control-plane host binary"),
)

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def code_of(path: Path) -> str:
    """The pipeline with Kotlin comments removed.

    A comment does not execute. `release-candidate.pipeline.kts` names
    `stage("skill-bundles")` *inside a comment* — a post-mortem explaining why
    a `$` in that stage once broke the Kotlin compile. Scanning raw text for
    stage names reports it as a second `skill-bundles`, which is not what
    this contract is about.

    That is the third time this session a gate-reading contract has matched
    its own prose. It is why `strip_comments` is imported from
    `test_candidate_packaging.py` rather than re-derived: the rule already
    exists and has been paid for.
    """
    return strip_comments(path.read_text(encoding="utf-8"))


def test_the_merge_gate_does_not_publish() -> None:
    """The gate that gates a merge must not build what a release publishes."""
    text = code_of(MERGE_GATE)
    for pattern, what in RELEASE_ONLY_IN_GATE:
        hits = [
            line.strip()
            for line in text.splitlines()
            if re.search(pattern, line)
        ]
        check(
            not hits,
            f"{MERGE_GATE.name} still runs {what} ({pattern}). Measured at "
            f"62% of the gate's wall time spent on artefacts a merge does not "
            f"publish:\n    " + "\n    ".join(hits[:3]),
        )


def test_the_release_lane_actually_does_it() -> None:
    """Moving work out of a gate is not the same as deleting it."""
    text = code_of(RELEASE_LANE)
    for needle, what in REQUIRED_IN_LANE:
        check(
            needle in text,
            f"{RELEASE_LANE.name} does not mention {what} (`{needle}`). It was "
            f"removed from the merge gate; if it is not here, nothing builds or "
            f"checks it anywhere. `cognicode-control-plane` is a host binary and "
            f"the per-target stage cross-compiles only cogh, cognicode and "
            f"cognicode-mcp, so dropping this leaves it built by no lane.",
        )


def test_the_release_lane_still_has_its_own_build_stage() -> None:
    """The moved block must not have collided with an existing stage name.

    Measured 2026-10-15: the first attempt inserted a `stage("build")` into a
    file that already had one — `release-candidate.pipeline.kts` holds a
    `build` CONTAINER for the per-target cross builds. Two stages with one
    name in one file means a failure gets attributed to the wrong one, and
    nothing complains.
    """
    for path in (MERGE_GATE, RELEASE_LANE):
        names = re.findall(r'stage\("([^"]+)"\)', code_of(path))
        seen: set[str] = set()
        dupes: list[str] = []
        for name in names:
            if name in seen:
                dupes.append(name)
            seen.add(name)
        check(
            not dupes,
            f"{path.name} declares the stage name(s) {sorted(set(dupes))} more "
            f"than once. Two stages with one name in one pipeline is how a "
            f"failure gets attributed to the wrong stage.",
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
        "PASS — the merge gate does no release work, the release lane still does "
        "all of it, and no stage name is declared twice"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())