#!/usr/bin/env python3
"""The release lane's platform declarations, read from the lane that builds them.

Both product generators — `generate_product_manifest.py` and
`generate_support_matrix.py` — used to read `.github/workflows/release.yml` to
learn which platforms are published, and each then asserted the answer against
its own hardcoded `expected` set. That was three places to keep in agreement
about the same fact, one of which is a file the cutover deletes.

There is one authority now: `release-candidate.pipeline.kts` declares

    val targets = listOf("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")

and maps each one to the release tool's platform spelling in `platformOf`. A
product manifest that says what is published must agree with the thing that
publishes it, so that is the file to read.

The two spellings are read together and required to cover the same set. They
are the same fact stated twice in one file, and a target added to `targets`
without a `platformOf` arm would otherwise be built and then packaged under an
unnameable platform — the pipeline guards against that with `error(...)`, and
this guard fails first and says which target is missing.

Deliberately not a Kotlin parser. Two declarations with a known shape are read
by shape; a general parser would be a second implementation of Kotlin's grammar
that drifts the first time the syntax is used differently. When a rule here
stops matching, the failure names the declaration it was reading.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path
from typing import NamedTuple

RELEASE_LANE = "release-candidate.pipeline.kts"


class ReleasePlatform(NamedTuple):
    """One published platform, named the two ways the repository names it.

    A plain `(target, platform)` tuple is what this was first written as, and
    the one caller that wanted the opposite order unpacked it backwards and
    produced a manifest whose `id` was a cargo triple and whose `target` was a
    platform id. Nothing failed at the type level; the committed JSON was wrong
    and only a test comparing the two sets caught it.

    Two names for one thing is exactly when a positional tuple is the wrong
    shape. The attributes say which is which, so reading it wrong is a
    `AttributeError` rather than a plausible-looking document.
    """

    target: str
    """Cargo target triple, e.g. `x86_64-unknown-linux-gnu`."""

    platform: str
    """The release tool's platform id, e.g. `linux-x86_64`."""

TARGETS_DECL = re.compile(
    r"^val\s+targets\s*=\s*listOf\((?P<body>[^)]*)\)\s*$", re.MULTILINE
)

# A cargo target triple is `<arch>-<vendor>-<os>-<abi>` or `<arch>-unknown-<os>`.
# The first version of this pattern required the literal `unknown-`, which is a
# Linux convention rather than part of the grammar: it rejected
# `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`, the exact targets the
# deferred-platform list is about. That made the check which guards those
# entries unreachable — you cannot add a macOS target to the lane and have the
# reader notice, because the reader cannot see macOS targets.
PLATFORM_ARM = re.compile(
    r'^\s*"(?P<target>[a-z0-9_]+(?:-[a-z0-9]+){1,4})"\s*->\s*"(?P<platform>[a-z0-9_\-]+)"\s*$',
    re.MULTILINE,
)


class ReleaseLaneUnreadable(ValueError):
    """The release lane no longer says what it is supposed to say.

    Raised rather than returning an empty list, because an empty list would
    make every downstream check vacuously true: a manifest generated from no
    platforms claims none, and a manifest is a claim.
    """


def _lane_text(root: Path) -> str:
    path = root / RELEASE_LANE
    if not path.is_file():
        raise ReleaseLaneUnreadable(
            f"{RELEASE_LANE} is missing. It is the authority for which "
            f"platforms are published; without it a product manifest would be "
            f"listing platforms nothing builds."
        )
    return path.read_text(encoding="utf-8")


def release_targets(root: Path) -> list[str]:
    """Every cargo target the release lane builds, in declaration order."""
    text = _lane_text(root)
    match = TARGETS_DECL.search(text)
    if not match:
        raise ReleaseLaneUnreadable(
            f"{RELEASE_LANE} declares no `val targets = listOf(...)`. The "
            f"published platform set is read from there, and a reader that "
            f"returned nothing would let a product manifest claim zero "
            f"platforms without failing."
        )
    targets = re.findall(r'"([^"]+)"', match.group("body"))
    if not targets:
        raise ReleaseLaneUnreadable(
            f"{RELEASE_LANE} declares `val targets = listOf()` with no entries. "
            f"An empty list is a claim that nothing is published, which is a "
            f"change of policy and not something a manifest should absorb "
            f"silently."
        )
    return targets


def release_platform_map(root: Path) -> dict[str, str]:
    """Cargo target -> the release tool's platform spelling, from `platformOf`."""
    text = _lane_text(root)
    mapping = {
        m.group("target"): m.group("platform") for m in PLATFORM_ARM.finditer(text)
    }
    if not mapping:
        raise ReleaseLaneUnreadable(
            f"{RELEASE_LANE} has no `platformOf` mapping arms. Each target it "
            f"builds needs one, or the release tool has no name to package it "
            f"under."
        )
    return mapping


def release_platforms(root: Path) -> list[ReleasePlatform]:
    """Every published platform, from both declarations, reconciled.

    Raises when a target is built without a platform name or a platform name
    exists for a target that is not built. Either means the lane's two
    declarations have drifted apart, and a manifest built from one of them
    would describe a release that cannot be built.
    """
    targets = set(release_targets(root))
    mapping = release_platform_map(root)

    missing = sorted(targets - set(mapping))
    if missing:
        raise ReleaseLaneUnreadable(
            f"{RELEASE_LANE} lists {missing} in `val targets` but `platformOf` "
            f"has no arm for {'it' if len(missing) == 1 else 'them'}. The "
            f"pipeline itself fails closed on this, with `error(...)`; failing "
            f"here first names the target instead of failing later in the "
            f"release."
        )
    extra = sorted(set(mapping) - targets)
    if extra:
        raise ReleaseLaneUnreadable(
            f"{RELEASE_LANE} names a platform for {extra}, which is not in "
            f"`val targets`. A platform that is not built must not appear in a "
            f"product manifest as certified."
        )
    return sorted(
        ReleasePlatform(target=target, platform=mapping[target]) for target in targets
    )


def main(argv: list[str]) -> int:
    """Print the release lane's platforms as `<platform-id>\\t<target>` lines.

    Exists so `scripts/check-release-matrix.sh` reads the same two declarations
    through the same reader rather than growing its own third parse. A shell
    script cannot import a module, and re-deriving the mapping in `grep` would
    make the published-platform fact live in two languages.

    Exit codes: 0 with one line per platform; non-zero with the reason on
    stderr if the lane is unreadable or its two declarations disagree.
    """
    root = Path(argv[0]) if argv else Path(__file__).resolve().parents[2]
    try:
        platforms = release_platforms(root)
    except ReleaseLaneUnreadable as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    for entry in platforms:
        print(f"{entry.platform}\t{entry.target}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
