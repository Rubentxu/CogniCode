#!/usr/bin/env python3
"""Which PipelineK script is the merge authority, and what does each one run.

The wiring contracts all need the same two answers — "is this capability
invoked by the pipeline that gates a merge?" — and each of them used to get
them from a different place: `pr-ci.yml`, because that was where the
authorisation lived. That is one fact, and it was being re-derived fourteen
times.

After the cutover the fact lives in exactly one place: the constant below. Every
wiring contract reads it from here, so moving the authority is a one-line
change and a contract cannot disagree with another about which pipeline counts.

What a wiring contract asserts is unchanged by the move. "cargo deny runs in a
job merge-gate needs" becomes "cargo deny runs in a stage of the merge
authority": the property is that a merge cannot pass without the capability,
and both topologies enforce it the same way — by the capability being part of
the run that gates the merge. What does *not* carry over is the mechanism. There
is no `needs:` here, and asking for one would be asking for a GitHub concept in
a system that has none.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# The pipeline a merge is gated on. Declared here and nowhere else.
MERGE_AUTHORITY = "merge-gate.pipeline.kts"

TRIPLE_QUOTE = '"' * 3

# One alternative per opening, each paired with its own closing delimiter.
# Matching an opener and then searching forward for whichever delimiter opened it
# desynchronises on nested quotes and silently truncates bodies — a checker that
# reports fewer problems than exist looks exactly like one that found none.
STEP = re.compile(
    'sh\\(\\s*' + TRIPLE_QUOTE + '(.*?)' + TRIPLE_QUOTE + r"\s*\.trimIndent\(\)"
    r'|sh\(\s*"(.*?)"\s*\)',
    re.DOTALL,
)


def pipeline_paths() -> list[Path]:
    return sorted(REPO_ROOT.glob("*.pipeline.kts"))


def strip_line_comments(text: str) -> str:
    """Drop whole-line comments.

    A comment that quotes a command in backticks is otherwise indistinguishable
    from a step that runs it — in both directions. A comment naming a missing
    gate would satisfy "the gate exists", and a comment explaining why a version
    must be pinned would trip "the version is pinned". Both happened while
    writing the supply-chain contract.
    """
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("//")
    )


def steps(text: str) -> list[str]:
    """Every `sh(...)` body, with the Kotlin escaped dollar resolved."""
    bodies: list[str] = []
    for match in STEP.finditer(text):
        body = match.group(1) if match.group(1) is not None else match.group(2)
        if body is not None:
            bodies.append(body.replace("${'$'}", "$"))
    return bodies


def pipeline_steps(name: str) -> list[str]:
    """The steps of one pipeline. Missing file yields no steps, not an error."""
    path = REPO_ROOT / name
    if not path.is_file():
        return []
    return steps(strip_line_comments(path.read_text(encoding="utf-8")))


def stage_of(text: str, needle: str) -> str | None:
    """The name of the stage a step belongs to, for the failure message.

    Textual rather than structural: enough to say "the skills validator is
    invoked from the wrong lane", which is the thing worth naming.
    """
    lines = strip_line_comments(text).splitlines()
    current: str | None = None
    for line in lines:
        stage = re.search(r'stage\(\s*"([^"]+)"\s*\)', line)
        if stage:
            current = stage.group(1)
        if needle in line:
            return current or "(top level)"
    return None


def invoked_by(invocation: str) -> dict[str, str]:
    """Map pipeline name -> the stage that invokes `invocation`.

    A pipeline whose *comment* mentions the command does not count: the string
    is looked for inside a `sh()` body, not anywhere in the file.
    """
    hosts: dict[str, str] = {}
    for path in pipeline_paths():
        text = strip_line_comments(path.read_text(encoding="utf-8"))
        for body in steps(text):
            if invocation in body:
                hosts[path.name] = stage_of(text, invocation) or "(unknown)"
                break
    return hosts


def is_in_merge_authority(invocation: str) -> bool:
    return MERGE_AUTHORITY in invoked_by(invocation)
