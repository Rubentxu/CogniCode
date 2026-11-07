#!/usr/bin/env python3
"""Contract: every `curl`/`wget` in tracked shell scripts and the justfile carries a deadline.

A transfer without `--max-time` waits forever when the server accepts the
connection and never answers (AGENTS.md, hard rule 7). The scan joins `\\`
continuations into one logical line and ignores comments, so a deadline on a
continuation line counts for its command.

Run: python3 scripts/run_contract_tests.py scripts/ci/test_shell_curl_deadlines.py
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TRANSFER = re.compile(r"(^|[\s(`\"$|;&@])(curl|wget)\s")
DEADLINE = re.compile(r"--max-time\b|\s-m\s*\d|--timeout\b|\btimeout\s+\d")


def transfers_without_deadline(source: str) -> list[int]:
    """First physical line of each logical command that transfers without a deadline."""
    lines = source.split("\n")
    found: list[int] = []
    i = 0
    while i < len(lines):
        j, logical = i, lines[i]
        while logical.rstrip().endswith("\\") and j + 1 < len(lines):
            j += 1
            logical = logical.rstrip()[:-1] + " " + lines[j]
        if (
            not logical.strip().startswith("#")
            and TRANSFER.search(logical)
            and not DEADLINE.search(logical)
        ):
            found.append(i + 1)
        i = j + 1
    return found


def tracked_shell_files() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "*.sh", "*.bash", "justfile"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
        timeout=60,
    ).stdout.split()
    return [ROOT / name for name in out]


def test_the_scan_sees_a_transfer_without_a_deadline() -> None:
    source = (
        "curl -fsSL https://x/a\n"
        "# curl https://x/comment\n"
        "curl -fsSL --max-time 30 https://x/b\n"
        "curl -fsSL \\\n  --retry 3 \\\n  https://x/c\n"
        "curl -fsSL \\\n  --max-time 30 \\\n  https://x/d\n"
        "x=$(wget -q https://x/e)\n"
        "    @curl -s http://localhost/health\n"
    )
    assert transfers_without_deadline(source) == [1, 4, 10, 11], transfers_without_deadline(source)


def test_the_scan_reaches_the_shipped_installer() -> None:
    names = {p.relative_to(ROOT).as_posix() for p in tracked_shell_files()}
    for needed in ("install.sh", "justfile", "scripts/ci/install-cosign.sh"):
        assert needed in names, f"{needed} no se recorre"


def test_every_shell_transfer_has_a_deadline() -> None:
    offenders = [
        f"{path.relative_to(ROOT)}:{line}"
        for path in tracked_shell_files()
        for line in transfers_without_deadline(path.read_text(encoding="utf-8", errors="replace"))
    ]
    assert not offenders, f"{len(offenders)} curl/wget sin plazo: " + ", ".join(offenders)
