#!/usr/bin/env python3
"""A stale roadmap snapshot is refused before the gate spends its minutes.

The gap
-------

`docs/roadmap/CURRENT.md` must cite a commit near the point where a branch
met `main`; the version ratchet pins that. The ratchet takes well under a
second, but it only ran inside the `contracts` stage of the gate, after the
gate had linted the tree and compiled the `cognicode` binary. When `main`
moved past the snapshot, the first gate of the next change paid for all of
that and then failed on a one-line document.

The header of a squash merge already has the right treatment: it is judged by
the authority that owns the rule, before the expensive part. The snapshot gets
the same one. The judgement is the ratchet itself, run as it is, so the
window of drift is written in one place and cannot be copied here.

What is pinned here, and why each one:

- A refusal by the ratchet stops the publisher before the gate starts and
  before any status is posted: the minutes would prove a tree that cannot be
  accepted anyway.
- The publisher asks the ratchet and does not measure on its own, so the
  window has a single owner.
- The question is asked before the gate is started.

Run:
    python3 scripts/ci/test_snapshot_is_judged_before_the_gate.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PUBLISHER_REL = "scripts/ci/publish-merge-gate.sh"
PUBLISHER = REPO_ROOT / PUBLISHER_REL
RATCHET_REL = "scripts/ci/test_roadmap_version_ratchet.py"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def write_fake(directory: Path, name: str, body: str) -> None:
    path = directory / name
    path.write_text(body, encoding="utf-8")
    os.chmod(path, 0o755)


def python_that_refuses_the_ratchet() -> str:
    """A `python3` that fails only for the ratchet and runs everything else."""
    real = sys.executable
    return (
        "#!/usr/bin/env bash\n"
        'case "$*" in\n'
        "  *test_roadmap_version_ratchet*)\n"
        '    echo "FAIL: CURRENT.md declara un SHA fuera de la ventana" >&2\n'
        "    exit 1;;\n"
        "esac\n"
        f'exec "{real}" "$@"\n'
    )


def test_a_stale_snapshot_stops_the_publisher_before_the_gate() -> None:
    """The behaviour: the ratchet's refusal is the publisher's refusal."""
    with tempfile.TemporaryDirectory() as tmp:
        bindir = Path(tmp) / "bin"
        bindir.mkdir()
        started = Path(tmp) / "gate-started.txt"
        posted = Path(tmp) / "posted.txt"

        write_fake(bindir, "python3", python_that_refuses_the_ratchet())
        write_fake(bindir, "git-control", '#!/usr/bin/env bash\necho "DECISIÓN: ALLOW"\n')
        write_fake(
            bindir,
            "pipelinek",
            f'#!/usr/bin/env bash\nprintf "started\\n" > {started}\nexit 0\n',
        )
        write_fake(
            bindir,
            "gh",
            "#!/usr/bin/env bash\n"
            'case "$*" in\n'
            f'  *"--method POST"*) printf "%s\\n" "$*" > {posted};;\n'
            "esac\n"
            'printf "7\\tfix(ci): una cabecera corta\\n"\n',
        )

        environment = dict(os.environ)
        environment["PATH"] = f"{bindir}:{environment['PATH']}"
        proc = subprocess.run(
            ["bash", str(PUBLISHER), "HEAD", "owner/name"],
            capture_output=True,
            text=True,
            env=environment,
            cwd=REPO_ROOT,
            check=False,
        )
        check(
            proc.returncode != 0,
            f"the publisher reported success (exit {proc.returncode}) with a "
            f"snapshot the ratchet refuses.",
        )
        check(
            not started.exists(),
            "the gate started although the snapshot was already refused. Its "
            "minutes prove a tree that cannot be accepted.",
        )
        check(
            not posted.exists(),
            "a status was published for a tree whose snapshot the ratchet "
            "refuses.",
        )


def test_the_publisher_asks_the_ratchet_and_does_not_measure_on_its_own() -> None:
    """One owner for the window of drift: the ratchet, run as it is."""
    text = PUBLISHER.read_text(encoding="utf-8") if PUBLISHER.is_file() else ""
    check(
        RATCHET_REL in text,
        f"{PUBLISHER_REL} never runs {RATCHET_REL}, so a stale snapshot is "
        f"still found only after the gate has compiled the tree.",
    )
    check(
        "SNAPSHOT_MAX_DRIFT" not in text,
        f"{PUBLISHER_REL} carries its own window of drift. The ratchet owns "
        f"it; a copy here is a second truth.",
    )


def test_the_snapshot_is_judged_before_the_gate_starts() -> None:
    """The ordering: the free judgement comes before the expensive gate."""
    text = PUBLISHER.read_text(encoding="utf-8") if PUBLISHER.is_file() else ""
    judgement = text.find(RATCHET_REL)
    gate = text.find("pipelinek run")
    check(
        judgement >= 0 and gate >= 0 and judgement < gate,
        f"{PUBLISHER_REL} must judge the snapshot before `pipelinek run`.",
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
        "PASS — a stale roadmap snapshot is refused by the ratchet before the "
        "gate starts"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
