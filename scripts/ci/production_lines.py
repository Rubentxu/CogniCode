#!/usr/bin/env python3
"""The production code of a source file: the code before its test module.

One rule, one owner.

A `#[cfg(test)]` is the boundary of the production code only when it OPENS A
MODULE. A test-only import or a test-only helper does not open one: the code
that follows it still ships. A measurement that stops at the first
`#[cfg(test)]` it finds measures a file that is not the file it was asked
about -- a 2 100-line file counted as 38 lines is a file the size ratchet
stops watching and a walk the traversal ratchet stops seeing.

Two consumers measure this and they used to measure it differently: the
ratchets (`scripts/ci/test_ratchets.py`) knew the rule and wrote down why;
the contract of the conditional write (`scripts/ci/test_conditional_write_contracts.py`)
cut the file at the first `#[cfg(test)]` it found, so a test-only import near
the top of a file made the whole production code invisible to it -- and it
reported a service that publishes atomically as a service that does not.

The rule lives here so that there is one thing to change when it changes.
"""

from __future__ import annotations

import re

# The attribute that opens the test module.
CFG_TEST = re.compile(r"^#\[cfg\(test\)\]")
# The attributes that may sit between a `#[cfg(test)]` and the module it opens.
ATTRIBUTE = re.compile(r"^\s*#\[")
# A module declaration, with or without visibility.
TEST_MODULE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\b")


def source_lines(text: str) -> list[str]:
    """The lines of a source file, without the empty one the text ends with."""
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    return lines


def production_boundary(lines: list[str]) -> int:
    """The index of the line that opens the test module; the end if there is none.

    A `#[cfg(test)]` is the boundary only when it opens a module. A test-only
    import or a test-only helper does not start one: the code that follows it
    still ships, and a measurement that stops there measures a file that is
    not the file it was asked about.
    """
    for index, line in enumerate(lines):
        if not CFG_TEST.match(line):
            continue
        if TEST_MODULE.match(line[line.index("]") + 1 :]):
            return index
        probe = index + 1
        while probe < len(lines):
            candidate = lines[probe].strip()
            if candidate == "" or candidate.startswith("//"):
                probe += 1
                continue
            if ATTRIBUTE.match(lines[probe]):
                probe += 1
                continue
            if TEST_MODULE.match(lines[probe]):
                return index
            break
    return len(lines)


def production_lines(text: str) -> int:
    """Lines before the test module starts; all lines if it never starts."""
    return production_boundary(source_lines(text))


def production_text(text: str) -> str:
    """The production code of a file: the text before its test module.

    The whole file when it has no test module.
    """
    lines = source_lines(text)
    return "\n".join(lines[: production_boundary(lines)])
