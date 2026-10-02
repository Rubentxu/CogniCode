#!/usr/bin/env python3
"""The bridge that lets a PipelineK verdict satisfy the required `merge-gate` check.

`main` requires a status context named `merge-gate` with `strict: true`. That
check has come from `.github/workflows/pr-ci.yml`, and the cutover deletes that
file — on the day it is deleted, the pull request that deletes it can no longer
produce its own required check and is blocked forever.

`scripts/ci/publish-merge-gate.sh` closes that: it runs the merge authority and
reports what it said. These contracts exist because a bridge that is subtly
wrong is worse than no bridge, since the wrong bridge does not fail — it
publishes a context nobody is waiting for, or publishes a success that was
never earned, and the second one is the shape of a bypass.

What is pinned here, and why each one:

- The context name is the one branch protection actually requires. Read from
  the API when it can be reached; the pinned constant is the fallback, and the
  two agreeing is the assertion. A typo here is invisible everywhere except in
  a pull request that never becomes mergeable.
- The script reports the gate's exit code, not a decision of its own. Asserted
  by structure — the publish reads the code the lane returned — and by a
  mutation: a variant that publishes `success` regardless goes red.
- The script actually runs the merge authority. A bridge that published a
  verdict nobody computed would satisfy every check in this file.
- The run database is inside the repository, so the verdict is auditable after
  the fact rather than only assertable at the moment of posting.

Run:
    python3 scripts/ci/test_publish_merge_gate_status.py
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_REL = "scripts/ci/publish-merge-gate.sh"
SCRIPT = REPO_ROOT / SCRIPT_REL
AUTHORITY = "merge-gate.pipeline.kts"

# What `main` requires, as of 2026-10-02 (commit 07f989c9 introduced the rule).
# Kept as a constant so the check works without network, and cross-checked
# against the API when the API is reachable.
REQUIRED_CONTEXT = "merge-gate"
REQUIRED_STRICT = True

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def script_text() -> str:
    return SCRIPT.read_text(encoding="utf-8")


def test_the_bridge_exists_and_is_runnable() -> None:
    """A bridge that is not there is not a bypass, it is a stuck pull request."""
    check(
        SCRIPT.is_file(),
        f"{SCRIPT_REL} is missing. Without it, the pull request that deletes "
        f"pr-ci.yml cannot produce the `{REQUIRED_CONTEXT}` check that `main` "
        f"requires, and is blocked with no way forward.",
    )
    if SCRIPT.is_file():
        check(
            os.access(SCRIPT, os.X_OK),
            f"{SCRIPT_REL} is not executable. It has a shebang and is meant to "
            f"be run directly, not read and re-implemented by the caller.",
        )


def test_the_context_name_is_the_one_branch_protection_requires() -> None:
    """Wrong context name: the check passes and nothing waits for it.

    This is the one failure the bridge cannot detect at runtime, because a
    status under a name nobody requires is indistinguishable from a success.
    So it is pinned here, and cross-checked against the live branch protection
    rule when the network is there.
    """
    text = script_text()
    declared = re.search(r'MERGE_GATE_CONTEXT="([^"]+)"', text)
    check(
        declared is not None,
        f"{SCRIPT_REL} does not declare MERGE_GATE_CONTEXT, so the context it "
        f"posts under is not stated anywhere in the file that publishes it",
    )
    if declared:
        check(
            declared.group(1) == REQUIRED_CONTEXT,
            f"{SCRIPT_REL} publishes under `{declared.group(1)}` but `main` "
            f"requires `{REQUIRED_CONTEXT}`. Every check would pass and nothing "
            f"would be waiting for it.",
        )

    # Cross-check against the rule itself, when we can ask.
    remote = subprocess.run(
        ["git", "remote", "get-url", "origin"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    m = re.search(r"github\.com[:/]([^/]+/[^/.]+)", remote)
    if not m:
        return
    probe = subprocess.run(
        ["gh", "api", f"repos/{m.group(1)}/branches/main/protection"],
        capture_output=True,
        text=True,
        check=False,
    )
    if probe.returncode != 0:
        return
    try:
        protection = json.loads(probe.stdout).get("required_status_checks") or {}
    except json.JSONDecodeError:
        return
    check(
        bool(protection),
        f"`main` has no required status checks, so this contract's premise is "
        f"wrong: nothing is waiting for `{REQUIRED_CONTEXT}`. Either the branch "
        f"protection was removed, or the contract should be retired with it.",
    )
    contexts = protection.get("contexts") or []
    check(
        REQUIRED_CONTEXT in contexts,
        f"`main` requires {contexts}, not `{REQUIRED_CONTEXT}`. The bridge "
        f"publishes a context the branch does not wait for.",
    )
    check(
        protection.get("strict") == REQUIRED_STRICT,
        f"`main`'s required checks are strict={protection.get('strict')}, "
        f"expected strict={REQUIRED_STRICT}. The bridge assumes a branch that "
        f"has to re-run the gate after the base moves; without strict, a stale "
        f"success on an old base would satisfy the rule.",
    )


def test_the_bridge_reports_the_gate_and_does_not_decide() -> None:
    """The published state is the lane's exit code, not the script's opinion."""
    text = script_text()
    check(
        "pipelinek run" in text,
        f"{SCRIPT_REL} does not run the merge authority. A bridge that reports "
        f"a verdict nobody computed would satisfy this contract and every "
        f"branch-protection rule on `main`.",
    )
    check(
        AUTHORITY in text,
        f"{SCRIPT_REL} never names {AUTHORITY}. Whichever pipeline it runs has "
        f"to be the one a merge is gated on, not one picked for the occasion.",
    )
    # The published state has to be a variable that the lane's exit code
    # assigns, never a literal. Two assertions that cannot be satisfied by a
    # bridge that decided for itself, and — the part that matters — one that
    # fails if someone hard-codes `state=success` above the branch.
    check(
        '-f "state=$state"' in text,
        f"{SCRIPT_REL} publishes a state that is not the variable it computed. "
        f"The verdict has to reach the API through `state=$state`, or the "
        f"branch below is decoration.",
    )
    for literal in ('state="success"', 'state="failure"'):
        check(
            f"    {literal}" in text or f"        {literal}" in text,
            f"{SCRIPT_REL} never assigns `{literal.strip()}`. Both outcomes "
            f"have to be spelled, or one of them is unreachable and a red gate "
            f"would be reported as anything the author chose.",
        )
    # Both assignments sit inside a test of the lane's own exit code.
    for literal, guard in (
        ('state="success"', '[ "$lane_rc" -eq 0 ]'),
        ('state="failure"', ''),
    ):
        idx = text.find(literal)
        if idx < 0:
            continue
        window = text[max(0, idx - 400):idx]
        if guard:
            check(
                guard in window,
                f"`{literal.strip()}` is not guarded by `{guard}`, so a success "
                f"can be published without the lane having returned 0",
            )
        else:
            check(
                "else" in window,
                f"`{literal.strip()}` is not in the else branch, so it is "
                f"assigned on the same path as a success",
            )
    check(
        'lane_rc=$?' in text,
        f"{SCRIPT_REL} does not capture the lane's exit code, so the state it "
        f"publishes cannot be the lane's verdict",
    )


def test_the_verdict_is_auditable_after_the_fact() -> None:
    """A posted status is an assertion; the run record is the evidence."""
    text = script_text()
    check(
        "--db" in text,
        f"{SCRIPT_REL} runs the pipeline without a database, so the run leaves "
        f"no record. A status whose only evidence is the moment it was posted "
        f"is worth nothing when someone asks whether the gate really ran.",
    )
    check(
        ".pipelinek" in text,
        f"{SCRIPT_REL} points the run database somewhere outside the "
        f"repository, so the record is lost with the temporary directory.",
    )
    check(
        "description=" in text,
        f"{SCRIPT_REL} publishes a state with no description, so the status "
        f"list cannot say which pipeline and which database produced it",
    )


def test_a_published_success_cannot_be_produced_without_running_the_gate() -> None:
    """The mutation: a bridge that reports success on a failed lane.

    Real behaviour is checked by running the script with a stand-in `pipelinek`
    on PATH that fails, and with `gh` replaced by a recorder. Nothing is posted:
    the recorder writes what it was asked to post, and this reads it.
    """
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        recorded = Path(tmp) / "posted.txt"

        # A stand-in pipelinek that fails, whatever the script asks for.
        (bindir / "pipelinek").write_text(
            "#!/usr/bin/env bash\n"
            'if [ "$1" = "run" ]; then echo "the gate says no" >&2; exit 7; fi\n'
            "exit 0\n",
            encoding="utf-8",
        )
        # A stand-in gh that records instead of posting.
        (bindir / "gh").write_text(
            "#!/usr/bin/env bash\n"
            'printf "%s\\n" "$*" > ' + str(recorded) + "\n"
            "exit 0\n",
            encoding="utf-8",
        )
        for name in ("pipelinek", "gh"):
            os.chmod(bindir / name, 0o755)

        env = dict(os.environ)
        env["PATH"] = f"{bindir}:{env['PATH']}"

        proc = subprocess.run(
            ["bash", str(SCRIPT)],
            capture_output=True,
            text=True,
            env=env,
            cwd=REPO_ROOT,
            check=False,
        )

        check(
            proc.returncode == 7,
            f"the bridge exited {proc.returncode} after a lane that exited 7. "
            f"It must report the gate's verdict, and the verdict here is red.",
        )
        check(
            recorded.is_file(),
            "nothing was posted. A bridge that reports nothing when the gate "
            "fails leaves the pull request blocked with no way to see why.",
        )
        if recorded.is_file():
            posted = recorded.read_text(encoding="utf-8")
            check(
                "state=failure" in posted,
                f"the bridge posted a state that is not failure after a red "
                f"gate: {posted.strip()[:200]}",
            )
            check(
                f"context={REQUIRED_CONTEXT}" in posted,
                f"the posted status does not carry the required context: "
                f"{posted.strip()[:200]}",
            )


def main() -> int:
    check(SCRIPT.is_file(), f"{SCRIPT_REL} is missing")
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
        "PASS — the bridge runs the merge authority, publishes under the "
        "context `main` requires, and cannot report a success the gate did not "
        "give"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
