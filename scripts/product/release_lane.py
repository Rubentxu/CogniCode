#!/usr/bin/env python3
"""The published platform declarations, read from the product's own matrix.

Both product generators — `generate_product_manifest.py` and
`generate_support_matrix.py` — used to read `.github/workflows/release.yml` to
learn which platforms are published, and each then asserted the answer against
its own hardcoded `expected` set. That was three places to keep in agreement
about the same fact, one of which is a file the cutover deletes.

The authority then became `release-candidate.pipeline.kts`, which declared
`val targets = listOf(...)` and mapped each target to the release tool's
platform spelling in `platformOf`. That file is gone: the release lanes were
retired with the rest of the gate apparatus on 2026-11-04, and a module whose
only input has been deleted has no input.

The authority now is `product/platforms.json`, which is where the platform
matrix already lived and where it still lives. It names every platform the
product knows about — published, experimental, unsupported alike — and marks
each one with a `support_level`. The published set is the one whose level is
`certified`: a platform is published when the product says it is certified,
not when a pipeline happens to build it.

That is the same fact the lane declared, stated by the file that is versioned
as the product's public statement of what it supports. It was already there
and already agreed with the lane — `linux-x86-64` was `certified` with target
`x86_64-unknown-linux-gnu`, which is exactly what the lane built.

Deliberately not a Kotlin parser any more. Two declarations with a known shape
were read by shape; the surviving declaration is JSON, and it is read by the
JSON parser rather than by a regex that guesses at its shape.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import NamedTuple

PLATFORMS_MATRIX = "product/platforms.json"

# The support level that means "this platform is published". The other levels
# (`experimental`, `unsupported`) describe platforms the product knows about
# but does not ship; a manifest built from them would claim a release that
# nothing produces.
PUBLISHED_LEVEL = "certified"


class ReleasePlatform(NamedTuple):
    """One published platform, named the two ways the repository names it.

    A plain `(target, platform)` tuple is what this was first written as, and
    the one caller that wanted the opposite order unpacked it backwards and
    produced a manifest whose `id` was a cargo triple and whose `target` was a
    platform id. Nothing failed at the type level; the committed JSON was wrong
    and only a test comparing the two sets caught it.

    Two names for one thing is exactly when a positional tuple is the wrong
    shape. The attributes say which is which, so reading it wrong is an
    `AttributeError` rather than a plausible-looking document.
    """

    target: str
    """Cargo target triple, e.g. `x86_64-unknown-linux-gnu`."""

    platform: str
    """The release tool's platform id, e.g. `linux-x86-64`."""


class ReleaseLaneUnreadable(ValueError):
    """The platform matrix no longer says what it is supposed to say.

    Raised rather than returning an empty list, because an empty list would
    make every downstream check vacuously true: a manifest generated from no
    platforms claims none, and a manifest is a claim.
    """


def _matrix_path(root: Path) -> Path:
    return root / PLATFORMS_MATRIX


def _read_matrix(root: Path) -> dict:
    path = _matrix_path(root)
    if not path.is_file():
        raise ReleaseLaneUnreadable(
            f"{PLATFORMS_MATRIX} is missing. It is the authority for which "
            f"platforms are published; without it a product manifest would be "
            f"listing platforms nothing builds."
        )
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ReleaseLaneUnreadable(
            f"{PLATFORMS_MATRIX} is not valid JSON: {exc}. A matrix that "
            f"cannot be parsed cannot say what the product publishes."
        ) from exc
    if not isinstance(data, dict):
        raise ReleaseLaneUnreadable(
            f"{PLATFORMS_MATRIX} must contain an object, found "
            f"{type(data).__name__}."
        )
    return data


def release_platforms(root: Path) -> list[ReleasePlatform]:
    """Every published platform, from the product's platform matrix.

    A platform is published when its `support_level` is `certified`. Each such
    entry must carry both a `target` (the cargo triple) and an `id` (the
    release tool's platform spelling); an entry missing either one is a
    platform that cannot be built or cannot be packaged, and a manifest that
    absorbed it would describe a release that cannot be produced.
    """
    data = _read_matrix(root)
    entries = data.get("platforms")
    if not isinstance(entries, list):
        raise ReleaseLaneUnreadable(
            f"{PLATFORMS_MATRIX} has no `platforms` array. The published "
            f"platform set is read from there, and a reader that returned "
            f"nothing would let a product manifest claim zero platforms "
            f"without failing."
        )
    if not entries:
        raise ReleaseLaneUnreadable(
            f"{PLATFORMS_MATRIX} declares an empty `platforms` array. An "
            f"empty list is a claim that nothing is published, which is a "
            f"change of policy and not something a manifest should absorb "
            f"silently."
        )

    published: list[ReleasePlatform] = []
    for entry in entries:
        if not isinstance(entry, dict):
            raise ReleaseLaneUnreadable(
                f"{PLATFORMS_MATRIX} has a `platforms` entry that is not an "
                f"object: {entry!r}."
            )
        if entry.get("support_level") != PUBLISHED_LEVEL:
            continue
        platform = entry.get("id")
        target = entry.get("target")
        missing = [
            name
            for name, value in (("id", platform), ("target", target))
            if not isinstance(value, str) or not value
        ]
        if missing:
            raise ReleaseLaneUnreadable(
                f"{PLATFORMS_MATRIX} marks {platform or target!r} as "
                f"`{PUBLISHED_LEVEL}` but has no {', '.join(missing)}. A "
                f"published platform needs both a cargo target to build and "
                f"a platform name to package it under."
            )
        published.append(ReleasePlatform(target=target, platform=platform))

    if not published:
        raise ReleaseLaneUnreadable(
            f"{PLATFORMS_MATRIX} declares no platform with support_level "
            f"`{PUBLISHED_LEVEL}`. A product that publishes nothing would be "
            f"a claim, not a fact, and a manifest built from it would be a "
            f"document about a product that does not exist."
        )

    return sorted(published, key=lambda entry: entry.platform)


def release_targets(root: Path) -> list[str]:
    """Every cargo target the product publishes, in declaration order."""
    return [entry.target for entry in release_platforms(root)]


def main(argv: list[str]) -> int:
    """Print the published platforms as `<platform-id>\\t<target>` lines.

    Exists so `scripts/check-release-matrix.sh` reads the same declaration
    through the same reader rather than growing its own second parse. A shell
    script cannot import a module, and re-deriving the mapping in `grep` would
    make the published-platform fact live in two languages.

    Exit codes: 0 with one line per platform; non-zero with the reason on
    stderr if the matrix is unreadable or declares a published platform
    without both names.
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
