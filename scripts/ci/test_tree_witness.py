#!/usr/bin/env python3
"""Contract: the tree witness sees what a suite leaves in the working tree.

A test that writes into the repository is a test with shared state: it breaks
rule 1 of AGENTS.md and it dirties the `git status` of every other worktree.
The SBOM contract did exactly that: `cargo cyclonedx` writes an SBOM next to
the manifest of EVERY workspace member, so a run against the repository left
`crates/*/*.cdx.json` behind, and `.gitignore` grew an entry per accident
instead of the accident being stopped.

`scripts/ci/tree_witness.py` is the cheap check that stops it: it records
`git status` before the suites and compares it afterwards. This contract pins
what that check has to do, and where it has to run.

What it pins:

- the residue the SBOM contract used to leave is seen even though git ignores
  it (an ignored file is exactly the kind of residue that hides);
- an untracked file, a file inside an untracked directory that already
  existed, a modified tracked file and a deleted one are all seen;
- what was already there before is not residue;
- the output of the gate's own mutation step is not residue;
- a witness that cannot tell fails closed: no snapshot, an unreadable snapshot
  or a directory that is not a repository is an error, never a clean bill;
- both entry points run the witness, and neither one lets it be skipped by
  `exec`-ing the suite.

Run through `scripts/run_contract_tests.py` or directly.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
WITNESS = REPO / "scripts" / "ci" / "tree_witness.py"
CONTRACTS_RUNNER = REPO / "scripts" / "ci" / "run-all-contracts.sh"
GATE = REPO / "scripts" / "test-gate.sh"

# The residue the SBOM contract left in the tree, by name.
SBOM_RESIDUE = "crates/cognicode-cli/cogh-x86_64-unknown-linux-gnu.cdx.json"

# A git that does not carry the operator's rules: the fixture is a repository
# of its own, and the verdict must be the same on every machine.
ISOLATED_GIT = {
    **os.environ,
    "GIT_CONFIG_GLOBAL": "/dev/null",
    "GIT_CONFIG_SYSTEM": "/dev/null",
}

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def _git(repo: Path, *args: str) -> None:
    proc = subprocess.run(
        ["git", "-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", *args],
        cwd=repo,
        capture_output=True,
        text=True,
        timeout=120,
        env=ISOLATED_GIT,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)}: {proc.stderr.strip()}")


def _write(path: Path, text: str = "x\n") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _fixture(root: Path) -> None:
    """A repository shaped like the real one: crates, a tracked file to edit,
    and a `.gitignore` that hides the SBOMs and the mutation report."""
    _write(root / ".gitignore", "*.cdx.json\n/mutants.out/\n/mutants.out.old/\n__pycache__/\n")
    _write(root / "crates/cognicode-cli/src/lib.rs", "pub fn a() {}\n")
    _write(root / "crates/cognicode-cli/Cargo.toml", "[package]\nname = \"cli\"\n")
    _write(root / "scratch/keep.txt")
    _write(root / "scripts/ci/check.py", "print('ok')\n")
    _git(root, "init", "-q", "-b", "main")
    _git(root, "add", "-A")
    _git(root, "commit", "-q", "-m", "chore: base")


def _witness(root: Path, *args: str) -> tuple[int, str]:
    proc = subprocess.run(
        [sys.executable, str(WITNESS), *args, "--root", str(root)],
        capture_output=True,
        text=True,
        timeout=120,
        env=ISOLATED_GIT,
    )
    return proc.returncode, proc.stdout + proc.stderr


def _after_a_suite(suite) -> tuple[int, str]:
    """Snapshot a fresh fixture, let `suite(root)` leave what it leaves, compare."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "repo"
        root.mkdir()
        _fixture(root)
        snapshot = Path(tmp) / "before.json"
        rc, out = _witness(root, "snapshot", str(snapshot))
        check(rc == 0, f"the snapshot of a clean fixture must succeed; rc={rc}\n{out}")
        suite(root)
        return _witness(root, "compare", str(snapshot))


# --- the behaviour -------------------------------------------------------------


def test_the_witness_sees_the_sbom_residue_even_though_git_ignores_it() -> None:
    rc, out = _after_a_suite(lambda root: _write(root / SBOM_RESIDUE))
    check(rc == 1, f"an ignored .cdx.json left by a suite must fail the witness; rc={rc}\n{out}")
    check(SBOM_RESIDUE in out, f"the verdict must name the file it found; got:\n{out}")


def test_the_witness_sees_an_untracked_file() -> None:
    rc, out = _after_a_suite(lambda root: _write(root / "crates/cognicode-cli/stray.txt"))
    check(rc == 1, f"an untracked file must fail the witness; rc={rc}\n{out}")
    check("crates/cognicode-cli/stray.txt" in out, f"the verdict must name the file; got:\n{out}")


def test_the_witness_sees_a_file_inside_a_directory_that_was_already_untracked() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "repo"
        root.mkdir()
        _fixture(root)
        _write(root / "notes/one.txt")  # the directory exists before the snapshot...
        snapshot = Path(tmp) / "before.json"
        _witness(root, "snapshot", str(snapshot))
        _write(root / "notes/two.txt")  # ...and a new file lands in it
        rc, out = _witness(root, "compare", str(snapshot))
    check(rc == 1, f"a new file in an untracked directory must fail the witness; rc={rc}\n{out}")
    check("notes/two.txt" in out, f"the verdict must name the new file; got:\n{out}")


def test_the_witness_sees_a_modified_and_a_deleted_tracked_file() -> None:
    def suite(root: Path) -> None:
        _write(root / "crates/cognicode-cli/src/lib.rs", "pub fn b() {}\n")
        (root / "scratch/keep.txt").unlink()

    rc, out = _after_a_suite(suite)
    check(rc == 1, f"a changed tracked file must fail the witness; rc={rc}\n{out}")
    check("crates/cognicode-cli/src/lib.rs" in out, f"the edit must be named; got:\n{out}")
    check("scratch/keep.txt" in out, f"the deletion must be named; got:\n{out}")


def test_what_was_already_in_the_tree_is_not_residue() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "repo"
        root.mkdir()
        _fixture(root)
        # The operator's own work in progress, present before the suites run.
        _write(root / "crates/cognicode-cli/src/lib.rs", "pub fn edited() {}\n")
        _write(root / "draft.txt")
        _write(root / SBOM_RESIDUE)
        snapshot = Path(tmp) / "before.json"
        _witness(root, "snapshot", str(snapshot))
        rc, out = _witness(root, "compare", str(snapshot))
        check(rc == 0, f"a tree that did not change must pass; rc={rc}\n{out}")


def test_the_mutation_report_of_the_gate_is_not_residue() -> None:
    def suite(root: Path) -> None:
        _write(root / "mutants.out/outcomes.json")
        _write(root / "mutants.out.old/outcomes.json")

    rc, out = _after_a_suite(suite)
    check(rc == 0, f"the report of the gate's own mutation step is not residue; rc={rc}\n{out}")


def test_python_bytecode_is_not_residue() -> None:
    rc, out = _after_a_suite(lambda root: _write(root / "scripts/ci/__pycache__/x.pyc"))
    check(rc == 0, f"the bytecode the contracts leave is not residue; rc={rc}\n{out}")


def test_the_allowance_is_for_the_report_and_not_for_a_lookalike() -> None:
    rc, out = _after_a_suite(lambda root: _write(root / "crates/mutants.out/stray.txt"))
    check(rc == 1, f"only the report at the root is allowed, not any path with its name; rc={rc}\n{out}")


def test_a_witness_that_cannot_tell_fails_closed() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp) / "repo"
        root.mkdir()
        _fixture(root)

        rc, out = _witness(root, "compare", str(Path(tmp) / "never-taken.json"))
        check(rc == 2, f"a missing snapshot is an error, not a clean bill; rc={rc}\n{out}")

        broken = Path(tmp) / "broken.json"
        broken.write_text("{not json", encoding="utf-8")
        rc, out = _witness(root, "compare", str(broken))
        check(rc == 2, f"an unreadable snapshot is an error; rc={rc}\n{out}")

        not_a_repo = Path(tmp) / "plain"
        not_a_repo.mkdir()
        rc, out = _witness(not_a_repo, "snapshot", str(Path(tmp) / "x.json"))
        check(rc == 2, f"a directory that is not a repository is an error; rc={rc}\n{out}")


# --- where it runs -------------------------------------------------------------


def _position(text: str, pattern: str) -> int:
    match = re.search(pattern, text, flags=re.MULTILINE)
    return match.start() if match else -1


def test_the_contract_suite_runs_inside_the_witness() -> None:
    text = CONTRACTS_RUNNER.read_text(encoding="utf-8")
    snapshot = _position(text, r"tree_witness\.py\s+snapshot")
    suite = _position(text, r"python3\s+scripts/run_contract_tests\.py")
    compare = _position(text, r"tree_witness\.py\s+compare")
    check(snapshot != -1, "run-all-contracts.sh must take the snapshot")
    check(compare != -1, "run-all-contracts.sh must compare against it")
    check(
        -1 < snapshot < suite < compare,
        "run-all-contracts.sh must snapshot, run the suite, then compare, in that order",
    )
    check(
        not re.search(r"^\s*exec\s+python3\s+scripts/run_contract_tests\.py", text, flags=re.MULTILINE),
        "run-all-contracts.sh must not `exec` the suite: nothing would run after it",
    )


def test_the_gate_runs_inside_the_witness() -> None:
    text = GATE.read_text(encoding="utf-8")
    snapshot = _position(text, r"tree_witness\.py\s+snapshot")
    compare = _position(text, r"tree_witness\.py\s+compare")
    first_step = _position(text, r'^\s*run_step\s+"')
    verdict = _position(text, r"=== test-gate rc=")
    check(snapshot != -1, "test-gate.sh must take the snapshot")
    check(compare != -1, "test-gate.sh must compare against it")
    check(
        -1 < snapshot < first_step and first_step < compare < verdict,
        "test-gate.sh must snapshot before its first step and compare after its last, "
        "before it prints the verdict",
    )


def main() -> int:
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            fn()
    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(
        "PASS — el chivato ve lo que una suite deja en el árbol, no acusa lo que ya estaba "
        "ni el informe de mutantes, falla cerrado y corre en los dos puntos de entrada"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
