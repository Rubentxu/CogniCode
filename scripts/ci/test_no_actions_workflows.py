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

## The scope is git's index, not the filesystem

Worth stating on its own because it was the second defect in this file, found
on 2026-10-03 against the commit that introduced it.

The first version walked the working tree and excluded directories by name.
It reported two action manifests in a repository that ships none, both of them
inside `sandbox/repos/` — gitignored checkouts of the third-party projects the
sandbox lanes analyse. `sandbox` was not on the list. The tree was correct; the
list was incomplete, and a list that has to be extended every time somebody
vendors a new tree is a list that will be wrong again.

Git already knows the answer to the question being asked, which is not "what
files are on this disk" but "what does this repository ship". An untracked file
cannot become an Actions surface: it is not pushed, and GitHub never parses it.
The third-party checkouts are the strongest argument for the index rather than
against it — they are large, they are expected to contain workflows, and they
are not CogniCode's CI.

The consequence of getting this wrong is worth recording, because it is worse
than a red gate. Those checkouts do not exist on a CI runner, so the contract
was green in CI and red on any machine that had run a sandbox lane: a gate
whose verdict tracked the machine rather than the tree, failing for reasons
unrelated to the change under test and green on the very commits it was
supposed to inspect.

Historical material is not preserved in the tree. Git history is the archive,
and a checked-in copy of a retired orchestrator is a file someone will read as
current. The reasoning that mattered is in
`docs/adr/ADR-CI-ORCHESTRATOR-CUTOVER.md`.

Run:
    python3 scripts/ci/test_no_actions_workflows.py
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import pipeline_authority as PA  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]

WORKFLOW_SUFFIXES = (".yml", ".yaml")
ACTION_MANIFESTS = ("action.yml", "action.yaml")

# The one place GitHub reads workflows from. A YAML file elsewhere in the tree
# is not an Actions surface: GitHub does not parse it.
WORKFLOW_DIR_PARTS = (".github", "workflows")

# The runner that executes this file, and the glob it discovers by. Resolved by
# reading each link rather than assumed — see
# `test_the_invariant_is_executed_by_the_merge_authority`.
CONTRACT_RUNNER = "scripts/ci/run-all-contracts.sh"
CONTRACT_GLOB = "scripts/ci/test_*.py"
CONTRACT_ENTRYPOINT = "scripts/run_contract_tests.py"

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


def tracked_paths(root: Path) -> list[Path] | None:
    """The files `root` ships, according to git. None when git cannot answer.

    `None` is not the same answer as an empty list. A repository whose index
    cannot be read has not been shown to contain zero Actions surfaces; it has
    been shown nothing at all, and a contract that reports "0" for that is the
    silent-green failure this repository keeps refusing to ship.
    """
    try:
        done = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            capture_output=True,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if done.returncode != 0:
        return None
    names = done.stdout.decode("utf-8", "replace").split("\0")
    return [Path(name) for name in names if name]


def is_workflow(relative: Path) -> bool:
    """Whether GitHub would execute this file as a workflow."""
    return (
        relative.suffix in WORKFLOW_SUFFIXES
        and relative.parts[: len(WORKFLOW_DIR_PARTS)] == WORKFLOW_DIR_PARTS
    )


def is_action_manifest(relative: Path) -> bool:
    if relative.name not in ACTION_MANIFESTS:
        return False
    # A record of what was, not a surface that runs. Listed rather than
    # pattern-matched on a directory prefix, so adding a new archive has to be
    # a deliberate edit here.
    return not relative.as_posix().startswith(ARCHIVE_PREFIXES)


def actions_surfaces(root: Path) -> tuple[list[Path], list[Path]] | None:
    """(workflows, action manifests) this repository ships, or None if unknown.

    The scope is git's index, not the filesystem. That is the whole difference
    between this contract and the one it replaced, and it was not a stylistic
    choice.

    Measured 2026-10-03, on the commit that closed the cutover: this file
    reported two action manifests in a tree with zero of them —
    `sandbox/repos/elixir/…/release_pre_built/action.yml` and
    `sandbox/repos/rust-analyzer/…/github-release/action.yml`. Both are inside
    `sandbox/repos/`, which is gitignored and holds third-party checkouts of
    the projects the sandbox lanes analyse. GitHub never parsed them and never
    will. The previous version walked the filesystem and excluded directories
    by name, and `sandbox` was not on that list.

    The failure mode that produced it is worse than a red gate. In CI the
    offending checkouts do not exist, so the gate is green there; on a
    developer machine that has run a sandbox lane it is red. A gate whose
    verdict depends on the machine rather than on the tree is not a gate, and
    the denylist would have needed one more name for every vendored tree anyone
    ever adds. Git already knows which files ship.
    """
    tracked = tracked_paths(root)
    if tracked is None:
        return None
    workflows = sorted(p for p in tracked if is_workflow(p))
    manifests = sorted(p for p in tracked if is_action_manifest(p))
    return workflows, manifests


def workflow_files() -> list[Path] | None:
    surfaces = actions_surfaces(REPO_ROOT)
    return None if surfaces is None else surfaces[0]


def action_manifests() -> list[Path] | None:
    surfaces = actions_surfaces(REPO_ROOT)
    return None if surfaces is None else surfaces[1]


def test_no_github_actions_workflow_exists() -> None:
    """The whole of the migration's end state, in one assertion."""
    workflows = workflow_files()
    assert workflows is not None, (
        "the tracked file list could not be read, so this contract has not "
        "established that there are zero workflows. An unreadable repository "
        "is not a clean one."
    )
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
    assert manifests is not None, (
        "the tracked file list could not be read, so this contract has not "
        "established that there are zero action manifests. An unreadable "
        "repository is not a clean one."
    )
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


def _git(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-C", str(root), *args], capture_output=True, timeout=60
    )


def make_repo(tmp: Path, files: dict[str, str], *, commit: bool = True) -> Path:
    """A throwaway git repository holding `files`.

    A synthetic fixture, so these tests can ask what the scan does with a given
    tree. The three assertions above deliberately run against the real
    repository instead, because a scanner that finds nothing when handed
    nothing has proved nothing about the tree it is supposed to be gating.
    """
    subprocess.run(
        ["git", "init", "-q", str(tmp)], check=True, capture_output=True, timeout=60
    )
    for name, body in files.items():
        path = tmp / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(body, encoding="utf-8")
    _git(tmp, "add", "-A")
    if commit:
        _git(
            tmp,
            "-c",
            "user.email=contract@example.invalid",
            "-c",
            "user.name=contract",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "fixture",
        )
    return tmp


def test_a_tracked_action_manifest_is_reported() -> None:
    """The scan has teeth: a real manifest in a real index is found.

    Without this, the whole contract could be satisfied by a function that
    returns an empty list for every input, which is the shape that let the
    false positive through in the first place.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(
            Path(raw),
            {"README.md": "x", ".github/actions/build/action.yml": "name: build\n"},
        )
        surfaces = actions_surfaces(root)
        assert surfaces is not None, "a repository git can read returned no answer"
        workflows, manifests = surfaces
        assert not workflows, f"a bare action manifest was read as a workflow: {workflows}"
        assert [str(p) for p in manifests] == [".github/actions/build/action.yml"], (
            "a committed action manifest was not reported, so the invariant "
            f"cannot fail: {manifests}"
        )


def test_a_tracked_workflow_is_reported() -> None:
    """The same teeth for the workflow half, at any depth GitHub would read."""
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(
            Path(raw),
            {".github/workflows/nested/ci.yml": "on: push\n", "README.md": "x"},
        )
        surfaces = actions_surfaces(root)
        assert surfaces is not None
        assert [str(p) for p in surfaces[0]] == [".github/workflows/nested/ci.yml"], (
            f"a committed workflow was not reported: {surfaces[0]}"
        )


def test_an_untracked_vendored_checkout_is_not_reported() -> None:
    """The regression: a third-party checkout is not an Actions surface.

    This is the case that made the gate red on a machine with `sandbox/repos/`
    populated and green in CI, from the same commit. It is asserted against a
    fixture because the real vendored trees are gitignored — a test that
    depended on them would pass on a clean checkout and prove nothing.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(
            Path(raw),
            {"README.md": "x"},
            commit=True,
        )
        vendored = root / "sandbox/repos/elixir/elixir/.github/workflows/release/action.yml"
        vendored.parent.mkdir(parents=True, exist_ok=True)
        vendored.write_text("name: release\n", encoding="utf-8")
        _git(root, "status", "--porcelain")  # the file exists and is untracked
        surfaces = actions_surfaces(root)
        assert surfaces is not None
        workflows, manifests = surfaces
        assert not workflows, (
            "an untracked third-party checkout was reported as a workflow of "
            f"this repository: {workflows}"
        )
        assert not manifests, (
            "an untracked third-party checkout was reported as an Actions "
            f"surface of this repository: {manifests}"
        )


def test_a_yaml_file_outside_the_workflow_directory_is_not_a_workflow() -> None:
    """GitHub parses one directory. A YAML fixture elsewhere is not a surface.

    This is the known-false-positive direction the opposite way: a crate
    fixture, a doc archive or a sandbox recipe that happens to be named `ci.yml`
    must not redden the gate, or the fix for the first false positive would be
    a gate nobody can run.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = make_repo(
            Path(raw),
            {
                "README.md": "x",
                "docs/adr/examples/ci.yml": "on: push\n",
                "crates/cognicode-core/tests/fixtures/ci.yaml": "on: push\n",
            },
        )
        surfaces = actions_surfaces(root)
        assert surfaces is not None
        assert not surfaces[0], (
            f"YAML outside .github/workflows was read as a workflow: {surfaces[0]}"
        )


def test_an_unreadable_repository_is_not_an_empty_answer() -> None:
    """Absent and empty are different, and the gate must not confuse them.

    If the tracked list cannot be read, the honest result is "unknown". A
    contract that answers "zero surfaces" for a repository it could not read
    has not passed; it has reported nothing, in the shape of a pass.
    """
    with tempfile.TemporaryDirectory() as raw:
        outside = Path(raw)
        (outside / "action.yml").write_text("name: x\n", encoding="utf-8")
        assert tracked_paths(outside) is None, (
            "a directory that is not a git repository answered with a file "
            "list; the fail-closed path is unreachable"
        )
        assert actions_surfaces(outside) is None, (
            "an unreadable tree produced an answer shaped like a clean one"
        )


def _executes_the_runner() -> bool:
    """Whether a step of the merge authority *runs* the contract runner.

    Not whether the runner's path appears in the lane. The distinction is not
    hypothetical: the `selector` stage names the same file inside a
    `PATHS="…"` assignment that feeds the suite selector a change filter, and
    that mention is not an execution of anything.

    This was the first version of this test's own wiring check, and a mutation
    found it: disabling the `contracts` stage left
    `is_in_merge_authority(CONTRACT_RUNNER)` true, because the selector stage
    still held the string. The check passed with the suite switched off, which
    is the exact failure this test exists to prevent — committed by the test
    that was supposed to prevent it.

    So the path has to be preceded by something that runs it.
    """
    path = REPO_ROOT / PA.MERGE_AUTHORITY
    if not path.is_file():
        return False
    text = PA.strip_line_comments(path.read_text(encoding="utf-8"))
    pattern = re.compile(
        r"(?:^|[\s;|&(])(?:ba|z|k|da)?sh\s+[\w./-]*"
        + re.escape(CONTRACT_RUNNER)
        + r"(?=$|[\s;|&)])",
        re.MULTILINE,
    )
    return any(pattern.search(body) for body in PA.steps(text))


def test_the_invariant_is_executed_by_the_merge_authority() -> None:
    """A gate that passes without being executed is a file.

    The chain is resolved link by link, not assumed: the merge authority runs
    the contract runner, the runner discovers `test_*.py` by glob, and this
    file is one of the files that glob matches. Any link can be broken by a
    later rename, and each is checked where it actually lives.

    `pipeline_authority` strips line comments before matching, so a comment
    naming the runner does not satisfy this either. That is the same trap that
    let the CP5 skills gate sit unexecuted: the capability has to be inside an
    `sh()` body, and — as the mutation above showed — inside a body that runs
    it rather than one that names it.
    """
    assert _executes_the_runner(), (
        f"{PA.MERGE_AUTHORITY} does not execute {CONTRACT_RUNNER} from a step, "
        "so nothing runs this contract on a pull request. The cutover's "
        "closing invariant has to be in the lane that gates a merge, not on "
        "disk next to it."
    )

    runner_path = REPO_ROOT / CONTRACT_RUNNER
    assert runner_path.is_file(), f"{CONTRACT_RUNNER} is missing"
    runner = runner_path.read_text(encoding="utf-8")
    assert CONTRACT_GLOB in runner, (
        f"{CONTRACT_RUNNER} no longer discovers contracts by the {CONTRACT_GLOB} "
        "glob. A hand-written list is how a contract ends up passing locally "
        "and running nowhere."
    )
    assert CONTRACT_ENTRYPOINT in runner, (
        f"{CONTRACT_RUNNER} no longer hands the discovered files to "
        f"{CONTRACT_ENTRYPOINT}, so discovering them would still run nothing"
    )

    discovered = {p.resolve() for p in REPO_ROOT.glob(CONTRACT_GLOB)}
    assert Path(__file__).resolve() in discovered, (
        f"{Path(__file__).name} does not match {CONTRACT_GLOB} any more, so the "
        "runner would never discover it. Rename deliberately, or the invariant "
        "stops being enforced without anything failing."
    )


def main() -> int:
    failures: list[str] = []
    for func in (
        test_no_github_actions_workflow_exists,
        test_no_composite_action_manifest_exists,
        test_every_lane_still_exists,
        test_a_tracked_action_manifest_is_reported,
        test_a_tracked_workflow_is_reported,
        test_an_untracked_vendored_checkout_is_not_reported,
        test_a_yaml_file_outside_the_workflow_directory_is_not_a_workflow,
        test_an_unreadable_repository_is_not_an_empty_answer,
        test_the_invariant_is_executed_by_the_merge_authority,
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
