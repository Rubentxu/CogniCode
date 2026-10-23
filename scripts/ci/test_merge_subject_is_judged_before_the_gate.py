#!/usr/bin/env python3
"""The header a squash merge would create is judged before the merge exists.

The gap
-------

A squash merge creates exactly one commit, and its header is the pull
request's title, verbatim. That commit is created by the forge, on the
forge's side, so it never passes through the hooks this machine runs on the
commits it makes itself. The house policy therefore gets its first look at
that header only afterwards, when the machine tries to reference the new
commit and the policy refuses the reference.

At that point the merge is public, the branch is protected against rewriting
its history, and the repository carries a commit that its own policy will
not let a clone point at. The mistake is measured in minutes of gate time
after the fact, and paid for in a history that cannot be repaired.

So the header is judged before the merge, by the authority that judges every
other commit. This contract pins that the judgement happens, that it happens
*before* the expensive part, and that it is the house policy doing the
judging rather than a number copied into this repository.

What is pinned here, and why each one:

- The check exists, is runnable, and refuses what the policy refuses.
- The check asks the house policy instead of measuring on its own. A copy of
  the limit would be a second truth, and the two would drift until one of
  them admitted exactly what the other forbids.
- The header that gets judged is the pull request's title, because that is
  the header the merge would create.
- A title that does not close the pull request it belongs to is refused,
  even when the policy admits it.
- The publisher of the merge gate consults the check before it starts the
  gate: a gate that proves something already known to be wrong costs the
  minutes it spends proving it.
- The check fails closed. Not being able to judge is not the same as having
  nothing to judge.

Run:
    python3 scripts/ci/test_merge_subject_is_judged_before_the_gate.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECK_REL = "scripts/ci/check-merge-subject.sh"
CHECK = REPO_ROOT / CHECK_REL
PUBLISHER_REL = "scripts/ci/publish-merge-gate.sh"
PUBLISHER = REPO_ROOT / PUBLISHER_REL

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def write_fake(directory: Path, name: str, body: str) -> None:
    path = directory / name
    path.write_text(body, encoding="utf-8")
    os.chmod(path, 0o755)


def authority_that_allows(record: Path) -> str:
    """A stand-in for the policy engine that admits every header."""
    return (
        "#!/usr/bin/env bash\n"
        f'printf "%s\\n" "$*" >> {record}\n'
        'echo "DECISIÓN: ALLOW"\n'
        "exit 0\n"
    )


def authority_that_refuses(record: Path) -> str:
    """A stand-in for the policy engine that refuses every header."""
    return (
        "#!/usr/bin/env bash\n"
        f'printf "%s\\n" "$*" >> {record}\n'
        'echo "DECISIÓN: DENY — Cabecera demasiado larga" >&2\n'
        "exit 1\n"
    )


def forge_that_answers_with(title: str) -> str:
    """A stand-in for the forge that answers with a pull request title.

    The first field is the pull request number and the second one its title,
    separated by a tab, which is the shape the check reads.
    """
    return (
        "#!/usr/bin/env bash\n"
        "case \" $* \" in\n"
        f'  *" --method POST "*) exit 0;;\n'
        "esac\n"
        f'printf "7\\t{title}\\n"\n'
    )


def forge_that_answers_with_nothing() -> str:
    """A stand-in for the forge that knows of no pull request."""
    return "#!/usr/bin/env bash\nexit 0\n"


def run(script: Path, arguments: list[str], bindir: Path | None = None) -> subprocess.CompletedProcess[str]:
    environment = dict(os.environ)
    if bindir is not None:
        environment["PATH"] = f"{bindir}:{environment['PATH']}"
    return subprocess.run(
        ["bash", str(script), *arguments],
        capture_output=True,
        text=True,
        env=environment,
        cwd=REPO_ROOT,
        check=False,
    )


def test_the_check_exists_and_is_runnable() -> None:
    """A check that is not there is a merge nobody looks at."""
    check(
        CHECK.is_file(),
        f"{CHECK_REL} is missing. Without it the header of a squash merge is "
        f"judged for the first time when the machine tries to reference the "
        f"commit, which is after the merge is public and permanent.",
    )
    if CHECK.is_file():
        check(
            os.access(CHECK, os.X_OK),
            f"{CHECK_REL} is not executable. It has a shebang and is meant to "
            f"be run directly, not read and re-implemented by the caller.",
        )


def test_a_header_the_policy_refuses_is_refused() -> None:
    """The behaviour: the verdict of the policy is the verdict of the check."""
    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        record = Path(tmp) / "judged.txt"
        write_fake(bindir, "git-control", authority_that_refuses(record))

        proc = run(
            CHECK,
            ["--title", "fix(ci): una cabecera que la casa no admite (#7)"],
            bindir,
        )
        check(
            proc.returncode != 0,
            f"the check admitted a header the policy refused (exit "
            f"{proc.returncode}). A check that cannot refuse is a check that "
            f"does not judge.",
        )


def test_the_check_judges_with_the_policy_and_not_with_a_number_of_its_own() -> None:
    """The mutation: the limit is not written in this repository.

    The policy engine is replaced by a stand-in that admits everything. If the
    check measured the header on its own, a very long title would still be
    refused, and the copy of the limit would be the truth instead of the
    policy.
    """
    long_title = "fix(core): " + ("una cabecera interminable " * 8) + "(#7)"

    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        record = Path(tmp) / "judged.txt"
        write_fake(bindir, "git-control", authority_that_allows(record))

        proc = run(CHECK, ["--title", long_title], bindir)
        check(
            proc.returncode == 0,
            f"the check refused a header of {len(long_title)} characters that "
            f"the policy admitted (exit {proc.returncode}). The check is "
            f"measuring the header with a limit of its own instead of asking "
            f"the policy, and that copy of the limit is a second truth.",
        )
        check(
            record.is_file(),
            "the policy was never asked. A check that decides on its own is "
            "not judging with the house policy, whatever it says.",
        )
        if record.is_file():
            judged = record.read_text(encoding="utf-8")
            check(
                long_title in judged,
                f"the policy was asked about something else:\n"
                f"    {judged.strip()[:300]}",
            )

    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        record = Path(tmp) / "judged.txt"
        write_fake(bindir, "git-control", authority_that_refuses(record))

        proc = run(CHECK, ["--title", "fix(core): corto (#7)"], bindir)
        check(
            proc.returncode != 0,
            f"the check admitted a header the policy refused (exit "
            f"{proc.returncode}). The policy is not deciding here; something "
            f"else is.",
        )


def test_the_header_judged_is_the_title_of_the_pull_request() -> None:
    """The header a squash merge creates is the title, verbatim."""
    title = "fix(core): el grafo distingue la declaracion del simbolo (#7)"
    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        record = Path(tmp) / "judged.txt"
        write_fake(bindir, "git-control", authority_that_allows(record))
        write_fake(bindir, "gh", forge_that_answers_with(title))

        proc = run(CHECK, ["--pr", "7", "--repo", "owner/name"], bindir)
        check(
            proc.returncode == 0,
            f"the check refused a title the policy admits (exit "
            f"{proc.returncode}): {proc.stderr.strip()[:300]}",
        )
        if record.is_file():
            judged = record.read_text(encoding="utf-8")
            check(
                title in judged,
                f"the policy was not asked about the title of the pull "
                f"request:\n    {judged.strip()[:300]}",
            )


def test_a_title_has_to_close_the_pull_request_it_belongs_to() -> None:
    """The rule that belongs to this repository and not to the policy.

    Every commit in this history says which pull request it closes. A title
    without that would be admitted by the policy and would land a commit that
    does not say where it came from.
    """
    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        record = Path(tmp) / "judged.txt"
        write_fake(bindir, "git-control", authority_that_allows(record))
        write_fake(bindir, "gh", forge_that_answers_with("fix(core): sin numero"))

        proc = run(CHECK, ["--pr", "7", "--repo", "owner/name"], bindir)
        check(
            proc.returncode != 0,
            f"the check admitted a title that does not close the pull request "
            f"it belongs to (exit {proc.returncode}). The commit it would "
            f"create would not say where it came from.",
        )


def test_the_check_fails_closed_when_it_cannot_judge() -> None:
    """Not being able to judge is not the same as having nothing to judge."""
    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        write_fake(bindir, "gh", forge_that_answers_with_nothing())

        environment = dict(os.environ)
        # A path without the policy engine on it: the check has to notice
        # that it cannot ask anybody.
        environment["PATH"] = "/usr/bin:/bin"
        proc = subprocess.run(
            ["bash", str(CHECK), "--title", "fix(core): algo (#7)"],
            capture_output=True,
            text=True,
            env=environment,
            cwd=REPO_ROOT,
            check=False,
        )
        check(
            proc.returncode != 0,
            f"the check admitted a header it could not judge (exit "
            f"{proc.returncode}). A check that cannot reach its authority has "
            f"to refuse, because refusing is the only answer it can stand "
            f"behind.",
        )


def test_the_gate_does_not_start_when_the_header_is_refused() -> None:
    """The ordering: the cheap judgement comes before the expensive gate.

    The gate takes minutes and proves that the tree is sound. When the
    header is already known to be wrong, those minutes buy nothing: the merge
    cannot happen anyway, and the status that would say so must not be
    published, because a green gate next to a header the policy refuses is
    the state this whole change exists to prevent.
    """
    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        record = Path(tmp) / "judged.txt"
        started = Path(tmp) / "gate-started.txt"
        posted = Path(tmp) / "posted.txt"

        write_fake(bindir, "git-control", authority_that_refuses(record))
        write_fake(
            bindir,
            "pipelinek",
            "#!/usr/bin/env bash\n"
            f'printf "started\\n" > {started}\n'
            "exit 0\n",
        )
        write_fake(
            bindir,
            "gh",
            "#!/usr/bin/env bash\n"
            'case "$*" in\n'
            f'  *"--method POST"*) printf "%s\\n" "$*" > {posted};;\n'
            "esac\n"
            'printf "7\\tfix(ci): una cabecera que la casa no admite (#7)\\n"\n',
        )

        proc = run(PUBLISHER, ["HEAD", "owner/name"], bindir)
        check(
            proc.returncode != 0,
            f"the publisher reported success (exit {proc.returncode}) for a "
            f"pull request whose title the policy refuses.",
        )
        check(
            not started.exists(),
            "the gate started even though the header was already refused. The "
            "minutes it spends prove the tree is sound, and the merge cannot "
            "happen anyway.",
        )
        check(
            not posted.exists(),
            "a status was published for a pull request whose header the policy "
            "refuses. A green gate next to a header that cannot land is the "
            "state this change exists to prevent.",
        )


def test_the_publisher_judges_the_header_before_it_starts_the_gate() -> None:
    """The wiring, read from the file that does the wiring."""
    if not PUBLISHER.is_file():
        check(False, f"{PUBLISHER_REL} is missing, so nothing publishes the gate.")
        return
    text = PUBLISHER.read_text(encoding="utf-8")
    check(
        "check-merge-subject.sh" in text,
        f"{PUBLISHER_REL} never consults {CHECK_REL}, so the header of a squash "
        f"merge is still judged for the first time when the machine tries to "
        f"reference the commit.",
    )
    judgement = text.find("check-merge-subject.sh")
    gate = text.find("pipelinek run")
    if judgement >= 0 and gate >= 0:
        check(
            judgement < gate,
            f"{PUBLISHER_REL} starts the gate before it judges the header. The "
            f"judgement has to come first: it is free, and it decides whether "
            f"the gate is worth its minutes.",
        )


def main() -> int:
    check(CHECK.is_file(), f"{CHECK_REL} is missing")
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
        "PASS — the header a squash merge would create is judged by the house "
        "policy before the gate starts, and a refusal stops the merge"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
