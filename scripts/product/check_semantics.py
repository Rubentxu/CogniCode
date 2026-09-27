#!/usr/bin/env python3
"""Shared comparison semantics for the ``--check`` mode of the product
generators.

Why this module exists
----------------------
The product generators (``generate_profiles.py``,
``generate_support_matrix.py``) stamp ``source_commit`` into every document
they emit, defaulting it to ``git rev-parse HEAD``. That field is
**provenance**: it records which commit the artefact was produced from. It
is not derived from the repository's content, and nothing can regenerate it
byte-identically on a later commit.

Comparing whole documents therefore made ``--check`` unsatisfiable. On any
checkout newer than the commit that produced the file, the only difference
is the provenance stamp, so the check reported drift that did not exist and
failed on every fresh commit. A gate that always fails is a gate nobody
reads, and one that has to be skipped is not a gate at all.

What ``--check`` means now
--------------------------
Two separate properties, deliberately not conflated:

1. **Content must match exactly.** Every derived field, the full document
   body. This is the property that detects real drift, and it stays strict.
2. **Provenance must be well-formed.** ``source_commit`` must be present
   and a lowercase 40-character Git SHA. It is *not* required to equal
   ``HEAD``, because that would reintroduce the self-invalidation.

Both generators use this module so the rule has exactly one definition.
Duplicating it per generator is how the two drift apart, and then neither
one is trustworthy.
"""

from __future__ import annotations

import json
import re
from typing import Any

SOURCE_COMMIT_FIELD = "source_commit"
_SHA_RE = re.compile(r"[0-9a-f]{40}")


def content_mismatch(actual: str, rendered: str) -> str | None:
    """Return why ``actual`` differs from ``rendered`` beyond provenance.

    Returns ``None`` when the documents agree on everything that is derived
    from repository content, even if their ``source_commit`` stamps differ.

    A specific message matters: "files differ" tells a reader nothing about
    where to look, and these are generated files large enough that the
    reader would otherwise have to diff them by hand.
    """
    if actual == rendered:
        return None

    try:
        left = json.loads(actual)
        right = json.loads(rendered)
    except json.JSONDecodeError as exc:
        return f"the committed document is not valid JSON: {exc}"

    stripped_left = _without_source_commit(left)
    stripped_right = _without_source_commit(right)
    if stripped_left == stripped_right:
        return None

    if isinstance(stripped_left, dict) and isinstance(stripped_right, dict):
        changed = sorted(
            key
            for key in set(stripped_left) | set(stripped_right)
            if stripped_left.get(key) != stripped_right.get(key)
        )
        if not changed:
            # The documents compare equal without provenance, so any
            # remaining difference is formatting rather than content.
            return "formatting differs from the canonical rendering"
        return f"content changed at {changed}"
    return "content differs"


def recorded_source_commit(actual: str) -> str | None:
    """Return the ``source_commit`` recorded in ``actual``, or ``None``."""
    try:
        document = json.loads(actual)
    except json.JSONDecodeError:
        return None
    if not isinstance(document, dict):
        return None
    recorded = document.get(SOURCE_COMMIT_FIELD)
    return recorded if isinstance(recorded, str) else None


def check_provenance(actual: str, label: str) -> None:
    """Validate the provenance stamp of a committed document.

    Raises ``ValueError`` naming ``label`` when the stamp is missing or
    malformed. Existence and shape are what can be checked without a
    regeneration round trip; equality with ``HEAD`` deliberately is not.
    """
    recorded = recorded_source_commit(actual)
    if recorded is None:
        raise ValueError(f"{label} has no {SOURCE_COMMIT_FIELD} to validate")
    if not _SHA_RE.fullmatch(recorded):
        raise ValueError(
            f"{label} has a malformed {SOURCE_COMMIT_FIELD}: {recorded!r} "
            "is not a lowercase 40-character Git SHA"
        )


def _without_source_commit(document: Any) -> Any:
    if isinstance(document, dict):
        return {
            key: value
            for key, value in document.items()
            if key != SOURCE_COMMIT_FIELD
        }
    return document
