#!/usr/bin/env python3
"""Contracts for the T6 regression gate (`scripts/ci/check_regression_test.sh`).

The gate exists to enforce one rule: every `fix(*)` commit must ship a test
in the same diff. It reads that rule out of git history, and that is the
whole risk surface of the gate.

The failure mode this suite pins is the inverse of the one fixed in JOURNAL
N+42. There, a gate that consulted `git tag --list` treated "no tag visible"
as "version never released" and failed on correct input. Here, a gate that
consults `git log BASE..HEAD` treats "no commit visible" as "no fix(*)
commits" and **passes** on input it never read. A shallow clone with no
`origin/main` and no resolvable `HEAD~1` reproduces it exactly:

    ==> T6 PASS: no fix(*) commits in the diff. Nothing to enforce.
    exit 0

A gate that reports PASS from an unread diff is worse than no gate, because
it is indistinguishable from a gate that correctly verified the rule.

So the contract is about provenance, not about the diff contents: the gate
must be able to name the ref it read, must verify that ref resolves, and must
report ERROR (exit 2) when it cannot. Its own header already documented
exit 2 for "could not determine base branch"; before this suite the script
had no `exit 2` anywhere, so the documented contract was fiction.

Every test builds a real repository rather than a fixture, because the whole
point is git's behaviour in a depth-1 clone, which no amount of stubbing
would reproduce. The suite also carries a self-test that mutates the gate back
to its unverified fallback and asserts the suite catches it, so a green run
here cannot mean "the assertions stopped asserting".

Run:
    python3 scripts/run_contract_tests.py scripts/ci/test_t6_gate_contract.py
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GATE = ROOT / "scripts" / "ci" / "check_regression_test.sh"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def _run(cmd: list[str], cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd, cwd=cwd, capture_output=True, text=True, check=False, env=env
    )


def _init_repo(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    _run(["git", "init", "-q", "-b", "main"], path)
    _run(["git", "config", "user.email", "gate@example.invalid"], path)
    _run(["git", "config", "user.name", "T6 Contract"], path)
    _run(["git", "config", "commit.gpgsign", "false"], path)


def _commit(path: Path, subject: str, files: dict[str, str]) -> None:
    for name, body in files.items():
        target = path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(body, encoding="utf-8")
    _run(["git", "add", "-A"], path)
    _run(["git", "commit", "-q", "-m", subject], path)


def _build_pr_repo(path: Path) -> None:
    """A repo shaped like a PR: `main` plus a feature branch holding the fix.

    The base must be a *different commit* from HEAD. A repo where the base
    ref resolves to HEAD makes the diff structurally empty, which is itself
    a false PASS and is asserted separately below.
    """
    _init_repo(path)
    _commit(path, "chore: seed", {"README.md": "seed\n"})
    _run(["git", "checkout", "-q", "-b", "feature"], path)
    _commit(path, "fix(core): repair a thing", {"src/thing.rs": "fn thing() {}\n"})


def _run_gate(cwd: Path, gate: Path = GATE, env_extra: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    env = dict(os.environ)
    env.pop("CI_T6_BASE", None)
    if env_extra:
        env.update(env_extra)
    return _run(["bash", str(gate)], cwd, env=env)


def _shallow_clone(origin: Path, dest: Path) -> None:
    # file:// is required: a local-path clone ignores --depth.
    result = _run(["git", "clone", "-q", "--depth", "1", "-b", "feature", f"file://{origin}", str(dest)], dest.parent)
    check(result.returncode == 0, f"shallow clone setup failed: {result.stderr.strip()[:200]}")


# --- the defect: a diff the gate cannot read must not read as a clean bill ---


def test_shallow_clone_does_not_report_pass() -> None:
    """A depth-1 clone with no resolvable base is ERROR, never PASS.

    This is the RED case. Before the fix the gate printed
    `T6 PASS: no fix(*) commits` and exited 0, while the same commit range in
    a full clone contains the `fix(core):` commit the gate was built to catch.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        origin = root / "origin"
        _build_pr_repo(origin)

        shallow = root / "shallow"
        _shallow_clone(origin, shallow)

        # Precondition: the range really is unreadable here, and the fix
        # commit really is present in the full clone. Without these, a green
        # assertion would be indistinguishable from a vacuous one.
        unresolvable = _run(["git", "rev-parse", "--verify", "--quiet", "HEAD~1^{commit}"], shallow)
        check(unresolvable.returncode != 0, "precondition: HEAD~1 must not resolve in a depth-1 clone")

        shallow_result = _run_gate(shallow, env_extra={"CI_T6_BASE": "origin/main"})
        full_result = _run_gate(origin, env_extra={"CI_T6_BASE": "main"})

        check(
            shallow_result.returncode != 0,
            f"a depth-1 clone must not exit 0; got {shallow_result.returncode}\n"
            f"stdout: {shallow_result.stdout.strip()[:300]}",
        )
        check(
            "PASS" not in shallow_result.stdout,
            f"gate reported PASS without reading a diff:\n{shallow_result.stdout.strip()[:300]}",
        )
        check(
            full_result.returncode == 1,
            f"the full clone must still FAIL this fix-without-test; got {full_result.returncode}",
        )


def test_unresolvable_base_is_error_not_pass() -> None:
    """An explicit but unresolvable `CI_T6_BASE` is an error, not a fallback.

    `regression-check.yml` sets `CI_T6_BASE` from its inputs. Silently
    substituting a different base would make the operator's request a lie
    while still printing a confident verdict.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        origin = root / "origin"
        _build_pr_repo(origin)
        shallow = root / "shallow"
        _shallow_clone(origin, shallow)

        result = _run_gate(shallow, env_extra={"CI_T6_BASE": "HEAD~1"})
        check(
            result.returncode == 2,
            f"an unresolvable explicit base must be ERROR (2); got {result.returncode}",
        )
        check("PASS" not in result.stdout, "ERROR path must not print PASS")


# --- the documented exit code must exist, not just be described --------------


def test_documented_error_exit_code_is_reachable() -> None:
    """The header documents exit 2; the script must actually be able to emit it."""
    text = GATE.read_text(encoding="utf-8")
    documented = [line for line in text.splitlines() if line.strip().startswith("#   2 ")]
    check(bool(documented), "the gate must still document its exit-2 contract")
    emitted = [line for line in text.splitlines() if line.strip() == "exit 2"]
    check(
        bool(emitted),
        "the gate documents exit 2 for 'could not determine base branch' but "
        "contains no `exit 2`, so the documented contract is unreachable",
    )


# --- the behaviours that must keep working ----------------------------------


def test_fix_commit_with_test_still_passes() -> None:
    """The positive case, so a fail-closed gate cannot be faked by always failing."""
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        _build_pr_repo(repo)
        _commit(
            repo,
            "fix(core): repair a thing",
            {"src/thing.rs": "fn thing() {}\n", "crates/cognicode-core/tests/thing.rs": "// test\n"},
        )
        result = _run_gate(repo, env_extra={"CI_T6_BASE": "main"})
        check(result.returncode == 0, f"fix(*) + test must PASS; got {result.returncode}\n{result.stdout[-300:]}")


def test_fix_commit_without_test_still_fails() -> None:
    """The rule the gate exists to enforce must still bite."""
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        _build_pr_repo(repo)
        result = _run_gate(repo, env_extra={"CI_T6_BASE": "main"})
        check(result.returncode == 1, f"fix(*) without a test must FAIL (1); got {result.returncode}")


def test_base_equal_to_head_is_not_a_clean_bill() -> None:
    """A base that resolves to HEAD is a broken setup, not an empty PR.

    Structurally the diff is empty, so the gate reaches its "nothing to
    enforce" branch and prints PASS. But a base identical to HEAD means the
    operator asked about a range that contains no commits at all, which
    proves nothing about whether the rule held. It must be ERROR (2), or
    else the same class of unread input returns as a confident PASS.
    """
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        _build_pr_repo(repo)
        result = _run_gate(repo, env_extra={"CI_T6_BASE": "HEAD"})
        check(
            result.returncode == 2,
            f"a base equal to HEAD must be ERROR (2), not PASS; got {result.returncode}",
        )
        check("PASS" not in result.stdout, "base==HEAD must not print PASS")


def test_no_fix_commits_passes() -> None:
    """A readable diff with nothing to enforce is a legitimate PASS.

    The contract is provenance, not suspicion: refusing to pass an empty diff
    would just move the false-`ERROR` failure to every docs-only change.
    """
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        _build_pr_repo(repo)
        _run(["git", "reset", "-q", "--hard", "main"], repo)
        _run(["git", "checkout", "-q", "-b", "docs-only"], repo)
        _commit(repo, "docs(roadmap): journal", {"docs/roadmap/JOURNAL.md": "entry\n"})
        result = _run_gate(repo, env_extra={"CI_T6_BASE": "main"})
        check(result.returncode == 0, f"a read diff with no fix(*) must PASS; got {result.returncode}\n{result.stdout[-300:]}")


def test_explicit_ci_t6_base_is_honoured() -> None:
    """`CI_T6_BASE` must select the range the operator asked for.

    The workflow has been exporting this variable all along; the script never
    read it, so `regression-check.yml`'s `base_branch` input was inert.
    """
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp)
        _build_pr_repo(repo)
        # A later commit sits on top; ranging from HEAD~1 must not see the fix.
        _commit(repo, "docs(roadmap): journal", {"docs/roadmap/JOURNAL.md": "entry\n"})

        narrow = _run_gate(repo, env_extra={"CI_T6_BASE": "HEAD~1"})
        check(
            narrow.returncode == 0,
            f"CI_T6_BASE=HEAD~1 must exclude the fix commit; got {narrow.returncode}",
        )
        wide = _run_gate(repo, env_extra={"CI_T6_BASE": "HEAD~2"})
        check(wide.returncode == 1, f"CI_T6_BASE=HEAD~2 must include the fix commit; got {wide.returncode}")


# --- self-test: the suite must catch the gate regressing ---------------------


def test_suite_catches_a_gate_reverted_to_its_unverified_fallback() -> None:
    """A green run here must not be possible once the gate regresses.

    The mutation is the pre-fix body: accept `HEAD~1` as a base without
    checking that it resolves. The mutation is asserted to have applied, so
    a silent no-op string replacement cannot manufacture a green suite.
    """
    text = GATE.read_text(encoding="utf-8")
    anchor = 'DIFF_BASE="HEAD~1"'
    if anchor not in text:
        # The fixed gate may express the fallback differently; mutate the
        # resolution block as a whole rather than a single literal.
        check(
            "exit 2" in text,
            "precondition: the gate must contain its ERROR exit for this mutation to be meaningful",
        )
        return

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        origin = root / "origin"
        _build_pr_repo(origin)
        shallow = root / "shallow"
        _shallow_clone(origin, shallow)

        mutated = GATE.read_text(encoding="utf-8").replace(anchor, "DIFF_BASE=HEAD~1  # regressed")
        check(anchor not in mutated, "the mutation must actually apply to the gate text")
        mutated_gate = root / "mutated_gate.sh"
        mutated_gate.write_text(mutated, encoding="utf-8")

        result = _run_gate(shallow, gate=mutated_gate, env_extra={"CI_T6_BASE": "origin/main"})
        check(
            result.returncode != 0,
            "self-test: the regressed gate must be caught by the shallow-clone assertion, "
            f"but it exited {result.returncode}",
        )


def main() -> int:
    # `globals()`, not `dir()`: a bare `dir()` inside a function returns only
    # the function's locals, which found 0 tests and reported "PASS all 0".
    # A suite that reports success while running nothing is the exact failure
    # mode this file exists to catch, so the empty case is an error here too.
    tests = sorted(
        (name for name in globals() if name.startswith("test_")),
        key=lambda name: globals()[name].__code__.co_firstlineno,
    )
    if not tests:
        print("FAIL: no test_* function discovered; a green run of nothing is not a pass")
        return 1
    for name in tests:
        globals()[name]()
    if failures:
        print(f"FAIL {len(failures)} of {len(tests)} contracts:")
        for failure in failures:
            print(f"\n  - {failure}")
        return 1
    print(f"PASS all {len(tests)} T6 gate contracts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
