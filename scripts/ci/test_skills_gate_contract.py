#!/usr/bin/env python3
"""Contracts for the CP5 skill gate: the validators exist, they have teeth,
and something required actually runs them.

`scripts/validate_skills.py` and `scripts/verify-skills.sh` are the only
things standing between a skill that teaches a tool name and an agent
that calls it and gets a JSON-RPC error. That is the CP5 gate in the
action register: "agent puede descubrir y usar CogniCode sin tool-name
guessing", and it is the acceptance criterion of A-033..A-036.

Neither one was invoked by any workflow. Measured 2026-10-02 with
`grep -rn` over `.github/workflows/`: zero hits for either. Both
passed, and passing is exactly the problem — a gate that no merge runs
is not a gate, and this repo has already paid for that three times
(N+50 selector, N+51 `--lib` blind spot, N+57 anchor scope) plus once
more in the CP axis when A-013's suite turned out to be green without
ever executing. `crates/cognicode-cli/tests/portable_skill_bundle.rs`
does not close the gap: its 8 tests exercise the validator CLI against
synthetic bundles, which proves the mechanism works when handed a
bundle, not that the six skills the repo publishes are valid.

So the pin needs three separate properties, and each has its own way of
rotting silently:

1. The step exists. A substring search for `validate_skills.py` would
   be satisfied by the comment explaining why it is absent.
2. The step is in a job `merge-gate` needs. A-013's lesson: a suite
   reachable only from `ci.yml`, which is `workflow_dispatch` and so
   never blocks a merge, is dead code wearing a green exit code. This
   is asserted by resolving the `needs:` list, not by trusting that
   "check" is the right job.
3. The validator is not vacuous. A validator neutered to `exit 0`, or
   one pointed at a catalog that resolves to nothing, satisfies every
   other assertion here. The floor is asserted against the real
   `skills/` tree: a non-zero count of skills and of validated MCP
   tool references. Measured 2026-10-02: 6 skills, 60 tool references.

The exit-2 contract for a missing catalog is pinned too. It is the
defect fixed alongside the catalog source, and it is the one most
likely to be reintroduced by someone tidying `load_catalog` back to
"return an empty set when the file is not there", which reads as
graceful degradation and is neither: it makes every tool reference in
every skill fail against an empty catalog while printing a warning
that says the check was skipped.

Run:
    python3 scripts/ci/test_skills_gate_contract.py
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = REPO_ROOT / ".github" / "workflows" / "pr-ci.yml"
VALIDATOR = REPO_ROOT / "scripts" / "validate_skills.py"
VERIFIER = REPO_ROOT / "scripts" / "verify-skills.sh"
SKILLS_DIR = REPO_ROOT / "skills"
CATALOG = REPO_ROOT / "product" / "tools.json"
PROFILES = REPO_ROOT / "product" / "profiles.json"

# Floors, not exact counts. The point is to catch the gate checking
# nothing, not to fail every time a skill is edited. Measured
# 2026-10-02 on the real tree: 6 skill directories, 60 validated MCP
# tool references (3 in `cognicode`, 5 in `cognicode-agent-hardness`,
# 52 in `cognicode-mcp`; `cognicode-developer` and `cognicode-pr-review`
# contribute none, and `cognicode-recommended` is a SkillSet).
MIN_SKILLS = 5
MIN_TOOL_REFS = 30

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def workflow_job_blocks(text: str) -> dict[str, str]:
    """Map job id -> that job's YAML text.

    Job keys sit at exactly two spaces of indentation under `jobs:`.
    Splitting on that is enough for a workflow whose job ids are all
    simple identifiers, and it avoids depending on a YAML parser the
    runner does not have.
    """
    blocks: dict[str, str] = {}
    starts: list[tuple[str, int]] = []
    for match in re.finditer(r"^  ([A-Za-z0-9_-]+):\s*$", text, re.MULTILINE):
        starts.append((match.group(1), match.start()))
    for index, (job_id, start) in enumerate(starts):
        end = starts[index + 1][1] if index + 1 < len(starts) else len(text)
        blocks[job_id] = text[start:end]
    return blocks


def merge_gate_needs(text: str) -> list[str]:
    """The job ids `merge-gate` declares in `needs:`."""
    blocks = workflow_job_blocks(text)
    gate = blocks.get("merge-gate", "")
    match = re.search(r"^\s+needs:\s*\[(.*?)\]", gate, re.MULTILINE)
    if not match:
        return []
    return [item.strip() for item in match.group(1).split(",") if item.strip()]


def run_validator(root: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(root / "scripts" / "validate_skills.py")],
        capture_output=True,
        text=True,
        check=False,
    )


def stage_tree(tmp: Path, product: dict[str, dict] | None) -> Path:
    """Build a `<root>/{scripts,skills,product}` tree the validator accepts.

    The validator derives every path from its own location, so staging a
    copy under a temporary root is the only way to exercise the
    catalog-absent and catalog-empty branches without touching the
    repository's real `product/`.

    `product` maps a filename under `product/` to the document to write.
    Pass None to stage no `product/` directory at all. Pass a subset to
    stage a document missing its companion, which is how the
    profiles-absent branch gets exercised without the catalog-absent one
    firing first.
    """
    root = tmp
    (root / "scripts").mkdir(parents=True, exist_ok=True)
    shutil.copy(VALIDATOR, root / "scripts" / "validate_skills.py")
    shutil.copytree(SKILLS_DIR, root / "skills")
    if product is not None:
        (root / "product").mkdir(parents=True, exist_ok=True)
        for name, document in product.items():
            (root / "product" / name).write_text(
                json.dumps(document), encoding="utf-8"
            )
    return root


def published() -> dict[str, dict]:
    """The real `product/` documents, parsed."""
    return {
        "tools.json": json.loads(CATALOG.read_text(encoding="utf-8")),
        "profiles.json": json.loads(PROFILES.read_text(encoding="utf-8")),
    }


def test_the_validators_are_pinned_in_a_job_merge_gate_needs() -> None:
    """The step must exist, and it must be somewhere a merge can be stopped.

    Asserted against the resolved `needs:` list rather than against the
    name of the job it happens to sit in today, because the failure
    being guarded against is a step quietly moving to a job that runs
    on a schedule.
    """
    text = WORKFLOW.read_text(encoding="utf-8")
    blocks = workflow_job_blocks(text)
    required = set(merge_gate_needs(text))
    check(bool(required), "could not resolve merge-gate's needs: list from pr-ci.yml")

    for script in ("scripts/validate_skills.py", "scripts/verify-skills.sh"):
        hosting = [
            job_id
            for job_id, body in blocks.items()
            if script in body and f"run:" in body
        ]
        check(
            bool(hosting),
            f"no job in pr-ci.yml runs {script}; the CP5 skill gate is not wired "
            "into any merge path",
        )
        outside = [job for job in hosting if job not in required]
        check(
            not outside,
            f"{script} runs in {outside}, which merge-gate does not need "
            f"(needs: {sorted(required)}). A gate in a job that no merge waits "
            "on is dead code with a green exit code — the A-013 failure",
        )


def test_the_skill_gate_is_not_vacuous_over_the_real_tree() -> None:
    """A floor on the real tree, so a neutered validator fails this contract.

    Every other test here checks that the gate is wired. This one
    checks that it is still looking at something. A validator patched
    to return 0 immediately, or pointed at a catalog that resolves to
    no tools, passes the wiring assertions and is caught here.

    This is the same shape as the non-vacuity floor in
    `backtick_provenance_contract`: a synthetic fixture proves the
    scanner works when handed a path, not that the sweep finds one.
    """
    check(VALIDATOR.is_file(), f"validator not found: {VALIDATOR}")
    check(SKILLS_DIR.is_dir(), f"skills directory not found: {SKILLS_DIR}")
    check(CATALOG.is_file(), f"published catalog not found: {CATALOG}")

    proc = run_validator(REPO_ROOT)
    check(
        proc.returncode == 0,
        f"validate_skills.py must pass on the real tree; exit {proc.returncode}\n"
        f"{proc.stdout}\n{proc.stderr}",
    )

    skills = sorted(d.name for d in SKILLS_DIR.iterdir() if d.is_dir())
    check(
        len(skills) >= MIN_SKILLS,
        f"only {len(skills)} skill(s) on disk ({skills}); this contract's floor is "
        f"{MIN_SKILLS}, so a sweep that finds almost nothing would still pass",
    )

    validated = sum(
        int(m) for m in re.findall(r"(\d+) MCP tool reference\(s\) validated", proc.stdout)
    )
    check(
        validated >= MIN_TOOL_REFS,
        f"the gate validated {validated} MCP tool reference(s) over the real tree; "
        f"the floor is {MIN_TOOL_REFS}. A gate that resolves no tool names is "
        "indistinguishable from a gate that found nothing to check",
    )


def test_the_gate_still_detects_a_tool_name_that_does_not_exist() -> None:
    """The floor proves the gate ran; this proves it would have complained.

    Run against a real published catalog with one skill rewritten to
    cite a tool that is not in it, in a staged copy so the repository
    is untouched.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = stage_tree(Path(tmp), published())
        target = root / "skills" / "cognicode" / "SKILL.md"
        target.write_text(
            target.read_text(encoding="utf-8")
            + "\nUse the `graph_query_tier2` tool for tiered graph queries.\n",
            encoding="utf-8",
        )
        proc = run_validator(root)

    check(
        proc.returncode != 0,
        "a skill citing a tool absent from the published catalog must fail the "
        f"gate; it exited {proc.returncode}. This is the whole purpose of the "
        "CP5 gate and the assertion that would notice if it stopped",
    )
    check(
        "graph_query_tier2" in proc.stdout,
        "the failure must name the offending tool, not just count errors; "
        f"stdout was:\n{proc.stdout}",
    )


def test_a_missing_catalog_is_an_explicit_failure_not_a_skipped_check() -> None:
    """Absent catalog -> exit 2, and never a claim that it was skipped.

    The behaviour this pins replaced a `load_catalog` that returned an
    empty set and printed "tool-ref validation skipped". Nothing was
    skipped: the branch still ran and produced 12 errors naming tools
    that exist. A warning that says the opposite of what the code did
    is worse than none, because it tells the reader the gate is off
    when the gate is misfiring.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = stage_tree(Path(tmp), product=None)
        proc = run_validator(root)

    check(
        proc.returncode == 2,
        "a missing published catalog must be a harness error (exit 2), not a "
        f"pass and not a validation failure; got exit {proc.returncode}\n"
        f"{proc.stdout}\n{proc.stderr}",
    )
    combined = proc.stdout + proc.stderr
    check(
        "skipped" not in combined,
        "the message must not claim the check was skipped; the gate did not run, "
        f"which is a different thing. Output was:\n{combined}",
    )


def test_a_missing_profiles_document_is_also_an_explicit_failure() -> None:
    """The same rule for the document the gate needs to know profile names.

    Without `product/profiles.json` the gate cannot tell `` `reviewer` ``
    (a profile) from `` `graph_query_tier2` `` (a tool), so every
    backticked profile name in every skill becomes a reported missing
    tool. Same shape as the catalog: an absent input is a harness error
    the gate could not run, never a silent degradation.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = stage_tree(Path(tmp), {"tools.json": published()["tools.json"]})
        proc = run_validator(root)

    check(
        proc.returncode == 2,
        "a missing published profiles document must be a harness error (exit 2); "
        f"got exit {proc.returncode}\n{proc.stdout}\n{proc.stderr}",
    )
    check(
        "profiles.json" in proc.stdout + proc.stderr,
        "the message must name the document it could not find; output was:\n"
        f"{proc.stdout}\n{proc.stderr}",
    )


def test_a_profile_name_is_not_reported_as_a_missing_tool() -> None:
    """A legitimate profile mention must not fail the gate.

    The gate had a denylist of backticked words that are not tools, and
    the published profile ids were not on it. They survived only
    because the skills happened to write them comma-separated, which
    trips the "near a comma means it is a parameter" heuristic. Measured
    before the fix, all three of these were reported as missing MCP
    tools: the comma-separated list, the same list without commas, and
    the word `reviewer` in an ordinary sentence.

    This is the reason the ids are read from `product/profiles.json`
    rather than hardcoded. Hardcoding them would have fixed today's four
    and left the next profile to be a false positive.
    """
    profiles = json.loads(PROFILES.read_text(encoding="utf-8"))
    ids = [profile["id"] for profile in profiles["profiles"]]
    check(bool(ids), "no profile ids in product/profiles.json; the test is vacuous")

    body = "MCP is available. Use `build_graph` and `find_usages` to navigate.\n"
    cases = {
        "comma separated": "Profiles: " + ", ".join(f"`{i}`" for i in ids) + " exist.\n",
        "space separated": "Profiles " + " ".join(f"`{i}`" for i in ids) + " exist.\n",
        "in a sentence": f"Pick the `{ids[0]}` profile for read-only work.\n",
    }
    with tempfile.TemporaryDirectory() as tmp:
        root = stage_tree(Path(tmp), published())
        target = root / "skills" / "cognicode" / "SKILL.md"
        pristine = target.read_text(encoding="utf-8")
        results = {}
        for label, extra in cases.items():
            # Rewritten from `pristine` each time, so the three phrasings
            # are independent rather than accumulating into one text.
            target.write_text(pristine + "\n" + body + extra, encoding="utf-8")
            results[label] = run_validator(root)

    for label, proc in results.items():
        check(
            proc.returncode == 0,
            f"naming a published profile ({label}) must not fail the skill gate; "
            f"exit {proc.returncode}\n{proc.stdout}\n{proc.stderr}",
        )


def test_an_empty_catalog_does_not_pass_vacuously() -> None:
    """A catalog with no tools is a claim, and the gate must reject it.

    Distinct from the missing-catalog case on purpose. Missing means
    the gate could not run; empty means it ran and the published
    document says the product exposes nothing. If that document is
    ever wrong, the skills still teach tool names, and a gate that
    shrugged at an empty catalog would wave it through.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = stage_tree(
            Path(tmp),
            {
                "tools.json": {"schema_version": "cognicode.tools/v1", "tools": []},
                "profiles.json": published()["profiles.json"],
            },
        )
        proc = run_validator(root)

    check(
        proc.returncode != 0,
        "a published catalog with zero tools must fail the skill gate, not pass "
        f"it vacuously; got exit {proc.returncode}\n{proc.stdout}",
    )


def main() -> int:
    check(VALIDATOR.is_file(), f"validator not found: {VALIDATOR}")
    check(VERIFIER.is_file(), f"verifier not found: {VERIFIER}")

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
        "PASS — skill gate is wired into a required job, non-vacuous over the "
        "real tree, and still detects a tool name that does not exist"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
