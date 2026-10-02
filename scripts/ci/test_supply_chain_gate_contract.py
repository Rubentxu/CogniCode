#!/usr/bin/env python3
"""Contracts for the supply-chain gate: it must exist, it must be pinned, and
something a merge waits on must run it.

`cargo deny check advisories` and `cargo deny check licenses` are the only
things standing between a known-vulnerable or unvetted-licence dependency and
a published binary. They ran, and they were not on the merge path.

Measured 2026-10-02. `grep -rn 'cargo deny' .github/workflows/` returns hits in
exactly two files:

    release.yml          on: push, tags: v*
    release-validate.yml on: workflow_dispatch

Neither fires on a pull request. The only workflow with `on: pull_request`
toward main is `pr-ci.yml`, and it does not invoke cargo-deny at all. So a
dependency with a published advisory merges to main unnoticed, and the first
thing that looks at it is the next person who pushes a version tag — at which
point there is no cheap way back.

`release.yml` is even explicit about having noticed the shape of the problem
and fixing it one layer too low:

    Without this step, a new dependency with an unlisted licence would pass
    `release.yml` without detection; only `release-validate.yml` would catch
    it, and that workflow is `workflow_dispatch`-only.

That comment is correct and the fix is incomplete. Duplicating the gate into
`release.yml` closed "release.yml alone" and left the merge path open. This is
the A-013 failure — a gate reachable only from a `workflow_dispatch` workflow is
dead code wearing a green exit code — applied to the security outcome.

The two remaining properties are about the wiring rather than the finding, and
each rots independently:

1. The gate runs in a job `merge-gate` lists in `needs:`. Asserted by resolving
   the `needs:` list, not by naming the job, so that moving the step to a
   scheduled or tag-triggered job fails instead of still looking right.
2. The cargo-deny version is fixed. `cargo install cargo-deny --locked` pins
   cargo-deny's *dependencies* but floats the *tool* to whatever is newest that
   day, so the gate's behaviour changes without a commit. The version asserted
   here is the one the policy in `deny.toml` was written against.

Not asserted: that cargo-deny is installed the same way, or that it lives in
this job rather than another. Both are free to change as long as a merge waits
on the result.

Run:
    python3 scripts/ci/test_supply_chain_gate_contract.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "pr-ci.yml"

# The tool version this contract pins the merge path to. 0.20.2 is the
# version the current `deny.toml` policy was measured against on 2026-10-02,
# and the one whose behaviour (`unused-ignored-advisory = "deny"`, the
# `[advisories] ignore` list) the advisory-ignore backing contract assumes.
PINNED_CARGO_DENY = "0.20.2"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def strip_comments(text: str) -> str:
    """Drop whole-line YAML comments before any scanning.

    Without this, a comment that quotes a command in backticks is
    indistinguishable from a step that runs it — in both directions. A
    comment naming a missing gate would satisfy "the gate exists", and a
    comment explaining that the version must be pinned would trip "the
    version is pinned". Both happened while writing this file: the
    paragraph explaining why `--locked` is not enough contains
    `cargo install cargo-deny --locked` in backticks, and the contract
    failed on its own documentation.

    Only whole-line comments are removed. A trailing `# comment` on a line
    that also carries a command is left in place, because the line is
    still executed; `verify-skills.sh` and the cargo-deny steps both rely
    on that, and half the workflow's steps have trailing comments.
    """
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def workflow_job_blocks(text: str) -> dict[str, str]:
    """Map job id -> that job's YAML text.

    Job keys sit at exactly two spaces of indentation under `jobs:`. Splitting
    on that is enough for a workflow whose job ids are simple identifiers, and
    it avoids depending on a YAML parser the runner does not have. The skills
    gate contract parses the same file the same way; the two must agree about
    what a job block is, or one of them silently stops seeing jobs.
    """
    blocks: dict[str, str] = {}
    starts: list[tuple[str, int]] = []
    for match in re.finditer(r"^  ([A-Za-z0-9_-]+):\s*$", text, re.MULTILINE):
        starts.append((match.group(1), match.start()))
    for index, (job_id, start) in enumerate(starts):
        end = starts[index + 1][1] if index + 1 < len(starts) else len(text)
        blocks[job_id] = text[start:end]
    return blocks


def merge_gate_needs(text: str) -> set[str]:
    """The job ids `merge-gate` declares in `needs:`."""
    blocks = workflow_job_blocks(text)
    gate = blocks.get("merge-gate", "")
    match = re.search(r"^\s+needs:\s*\[(.*?)\]", gate, re.MULTILINE)
    if not match:
        return set()
    return {item.strip() for item in match.group(1).split(",") if item.strip()}


def test_the_supply_chain_gate_runs_in_a_job_merge_gate_needs() -> None:
    """The step must exist, and it must be somewhere a merge can be stopped.

    Asserted against the resolved `needs:` list rather than the name of the job
    it sits in today, because the failure being guarded against is a step
    quietly moving to a workflow that runs on a schedule or on a tag.
    """
    text = strip_comments(WORKFLOW.read_text(encoding="utf-8"))
    blocks = workflow_job_blocks(text)
    required = merge_gate_needs(text)
    check(bool(required), "could not resolve merge-gate's needs: list from pr-ci.yml")

    for axis in ("advisories", "licenses"):
        invocation = f"cargo deny check {axis}"
        hosting = [
            job_id
            for job_id, body in blocks.items()
            # `run:` in the body keeps a job whose *comment* mentions the
            # command from counting as a host, which is the same reason the
            # skills gate contract checks it.
            if invocation in body
            and re.search(r"^\s+run:", body, re.MULTILINE)
        ]
        check(
            bool(hosting),
            f"no job in pr-ci.yml runs `{invocation}`; the supply-chain gate is "
            "not on the merge path, so a dependency with a published advisory "
            "merges unnoticed and is first examined at tag time",
        )
        outside = [job for job in hosting if job not in required]
        check(
            not outside,
            f"`{invocation}` runs in {outside}, which merge-gate does not need "
            f"(needs: {sorted(required)}). A security gate in a job no merge "
            "waits on is the A-013 failure applied to the supply chain",
        )


def test_the_gate_covers_both_axes_the_policy_declares() -> None:
    """Advisories alone is half a supply-chain gate.

    `deny.toml` carries both an `[advisories]` section and a `[licenses]`
    allow-list, and the licence allow-list took real work to build (CP2-DEBT-04
    landed the shared `--check` semantics; the workspace `license` fields came
    in a separate commit). A wiring that runs only the advisory axis leaves
    that work unexercised on every merge while still looking like a security
    gate in the diff.
    """
    text = strip_comments(WORKFLOW.read_text(encoding="utf-8"))
    for axis in ("advisories", "licenses"):
        check(
            f"cargo deny check {axis}" in text,
            f"pr-ci.yml does not run the {axis} axis; deny.toml declares a policy "
            "for it that no merge currently exercises",
        )


def test_the_cargo_deny_version_is_fixed() -> None:
    """`--locked` pins cargo-deny's dependencies, not cargo-deny itself.

    `cargo install cargo-deny --locked` resolves cargo-deny's own Cargo.lock,
    so the tool still floats to whatever version is newest on the day it runs.
    A security gate whose behaviour can change without a commit is a gate whose
    behaviour nobody reviewed. The fix is one flag, and the version to pin is
    the one `deny.toml` was written against.
    """
    text = strip_comments(WORKFLOW.read_text(encoding="utf-8"))
    installs = re.findall(r"cargo install cargo-deny[^\n]*", text)
    check(
        bool(installs),
        "pr-ci.yml does not install cargo-deny at all; a gate that runs it "
        "would depend on whatever the runner image happens to carry, and the "
        "runner is not a declared input to this repository",
    )
    for command in installs:
        check(
            "--version" in command,
            f"cargo-deny is installed without a fixed version: `{command}`. "
            "--locked pins its dependencies but floats the tool, so the gate's "
            "behaviour can change with no commit in this repository",
        )
        check(
            PINNED_CARGO_DENY in command,
            f"cargo-deny is not pinned to the version this policy was measured "
            f"against ({PINNED_CARGO_DENY}): `{command}`",
        )


def main() -> int:
    check(WORKFLOW.is_file(), f"workflow not found: {WORKFLOW}")

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
        "PASS — the supply-chain gate runs in a job a merge waits on, covers "
        "both declared axes, and pins the tool version"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
