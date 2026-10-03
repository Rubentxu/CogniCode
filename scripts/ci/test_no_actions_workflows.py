#!/usr/bin/env python3
"""No GitHub Actions workflow exists in this repository. Zero.

This file replaces `test_ci_orchestrator_gap.py`, and the replacement is the
point.

The gap checker was a *migration* instrument. It compared the commands Actions
ran against the commands PipelineK ran, so that no gate could be lost quietly
while the two orchestrators overlapped. Its job had a stated end: when Actions
reached zero, keeping it would have meant keeping a dependency on the system
being deleted, and — worse — keeping a rule that the Actions side must have
gates, which is the only way a zero-Actions repository can be accused of
having gates it does not have.

So the instrument retires and leaves one invariant behind.

## Why zero files and not zero triggers

Six workflows were deleted. Two of them would have run on their own: `pr-ci.yml`
on `pull_request` and `release.yml` on a `v*` tag. The other four carried only
`workflow_dispatch` and could not run without a human asking.

Checking for "no automatically-triggered workflow" would pass with those four
still in the tree. That is a weaker end state than the one this cutover set out
to reach, and it is weaker in a way that costs something concrete: a file that
GitHub still parses, still lists in the Actions tab, and that the next person
can add a `push:` trigger to in one line. A manual-only workflow is a
cocked gun, not an absence.

The invariant is therefore "no workflow file", stated plainly, and it is
harder to misread than a trigger-classification rule would be.

## What is checked

- `.github/workflows/` holds no `*.yml` or `*.yaml`, at any depth — GitHub
  executes workflow files in that directory and its subdirectories, and a
  nested one counts. A YAML file elsewhere in the tree is not an Actions
  surface: GitHub does not read it, and counting it would make the contract
  fire on a documentation archive or a crate fixture.
- No `action.yml` / `action.yaml` anywhere in the tree, since a composite
  action is an Actions surface with no workflow to host it.
- The six lanes still exist. The migration is finished, not reverted: a
  repository with no Actions *and* no orchestrator has not reached the target
  state, it has lost CI. This is the half of the contract that would catch the
  cutover being undone rather than completed.
- The lanes are not a ratchet against each other. The old file compared
  coverage against a base commit; that comparison had meaning only while two
  orchestrators existed to compare. Its successor asserts the end state, and
  per-lane capability is asserted by the wiring contracts that name a lane —
  `test_supply_chain_gate_contract.py`, `test_skills_gate_contract.py`,
  `test_cr06_ratchet.py`, `test_pipeline_stage_bodies.py` and
  `test_release_candidate_layout.py`.

Historical material is not preserved in the tree. Git history is the archive,
and a checked-in copy of a retired orchestrator is a file someone will read as
current. The reasoning that mattered is in
`docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`.

Run:
    python3 scripts/ci/test_no_actions_workflows.py
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
WORKFLOW_SUFFIXES = (".yml", ".yaml")
ACTION_MANIFESTS = ("action.yml", "action.yaml")

# Directories that are not part of the product: build output, the untracked
# scratch area, and vendored trees. A workflow in any of them is not a
# workflow this repository ships.
SKIP_DIRS = {".git", "target", "node_modules", "odd", ".pipelinek"}

# The lanes the cutover produced. Named so the failure says which one is
# missing rather than "no orchestrator found".
LANES = (
    "merge-gate.pipeline.kts",
    "integration.pipeline.kts",
    "product-fast.pipeline.kts",
    "release-candidate.pipeline.kts",
    "release.pipeline.kts",
    "certification.pipeline.kts",
)

# Files whose contents are a record of what was, not a statement of what is.
# Listed rather than pattern-matched on a directory prefix so adding a new
# archive has to be a deliberate edit here.
ARCHIVE_PREFIXES = ("docs/", "evidence/", "openspec/changes/archive/")


def workflow_files() -> list[Path]:
    """Every workflow file GitHub would execute from this repository.

    Confined to `.github/workflows/`, because that is the only place GitHub
    reads workflows from. A YAML file elsewhere in the tree is not an Actions
    surface, and treating it as one would make the contract fire on a
    `docs/…/workflows/` archive or a crate fixture — a linter with a known
    false positive is worse than no linter.

    An earlier version of this scan was phrased as "a workflow anywhere else
    in the tree is also a workflow" and did not implement that, which a
    mutation caught: a workflow dropped in an unrelated directory passed. The
    claim was wrong, not the code. It is now stated as what it checks.
    """
    found: list[Path] = []
    root = WORKFLOW_DIR
    if not root.is_dir():
        return found
    for path in sorted(root.rglob("*")):
        if path.is_file() and path.suffix in WORKFLOW_SUFFIXES:
            found.append(path.relative_to(REPO_ROOT))
    return found


def action_manifests() -> list[Path]:
    found: list[Path] = []
    for path in REPO_ROOT.rglob("*"):
        if not path.is_file():
            continue
        relative = path.relative_to(REPO_ROOT)
        if any(part in SKIP_DIRS for part in relative.parts):
            continue
        if relative.as_posix().startswith(ARCHIVE_PREFIXES):
            continue
        if relative.name in ACTION_MANIFESTS:
            found.append(relative)
    return sorted(found)


def test_no_github_actions_workflow_exists() -> None:
    """The whole of the migration's end state, in one assertion."""
    workflows = workflow_files()
    assert not workflows, (
        f"{len(workflows)} GitHub Actions workflow(s) in the tree:\n"
        + "\n".join(f"    {p}" for p in workflows)
        + "\n\n  PipelineK is the execution authority (see "
        "docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md). A workflow here would be a "
        "second orchestrator whose behaviour nobody would run and everybody "
        "would believe. If a new capability needs a lane, add a stage to the "
        "lane that owns the responsibility — do not add a workflow."
    )


def test_no_composite_action_manifest_exists() -> None:
    """A composite action is an Actions surface with no workflow to host it."""
    manifests = action_manifests()
    assert not manifests, (
        f"{len(manifests)} action manifest(s) in the tree:\n"
        + "\n".join(f"    {p}" for p in manifests)
        + "\n\n  A composite action only runs inside a workflow, and there are "
        "no workflows. This file is a surface with nothing that can execute it."
    )


def test_every_lane_still_exists() -> None:
    """The cutover finished, not undone.

    Zero Actions with zero orchestrators is not the target state; it is a
    repository that has lost CI. This is the half of the contract that fails
    when the migration is rolled back rather than completed, and it is the only
    reason the first two are safe to assert: they are the negative of this.
    """
    missing = [lane for lane in LANES if not (REPO_ROOT / lane).is_file()]
    assert not missing, (
        f"missing lane(s): {', '.join(missing)}\n"
        f"  The repository has no GitHub Actions workflows, so these are the "
        f"only thing that decides what runs. Restore the lane, or — if the "
        f"responsibility it owned moved — move it deliberately and update the "
        f"list in this file, which is the only record of what the cutover was "
        f"supposed to leave behind."
    )


def main() -> int:
    failures: list[str] = []
    for func in (
        test_no_github_actions_workflow_exists,
        test_no_composite_action_manifest_exists,
        test_every_lane_still_exists,
    ):
        try:
            func()
        except AssertionError as exc:
            failures.append(str(exc))

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(failure)
        return 1

    print(
        f"PASS — 0 GitHub Actions workflows, 0 action manifests, "
        f"{len(LANES)} PipelineK lanes present."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
