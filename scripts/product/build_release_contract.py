#!/usr/bin/env python3
"""Parse CogniCode's Rust release contract into a Python-side view.

This is a textual parser: the e84/e85 release contract is short and pinned,
so we read it as text with regular expressions rather than introducing an
in-process Rust AST. The output of this module is the single Python-side
authoritative view used by the product projections under
``scripts/product/generate_*.py``.

Outputs a JSON-ish dict with:

* ``published_profile_pairs``: ordered list of (id, description) matching
  ``PUBLISHED_PROFILES`` in source order.
* ``components``: list of {stem, profiles, published, layer}.
* ``skill_bundles``: list of {id, profiles, published}.
* ``skill_maturity_alphabet``: list of accepted maturity values from
  ``crates/cognicode-cli/src/cmd/skill.rs``.

The function ``parse_release_contract`` is deterministic: it never reads
from the filesystem outside its callers' ``Path`` argument.
"""

from __future__ import annotations

import re
from pathlib import Path


_COMPONENT_BLOCK_RE = re.compile(
    r"ComponentSpec\s*\{\s*"
    r"kind:\s*ArtifactKind::(?P<kind>[A-Za-z]+),\s*"
    r"profiles:\s*&(?P<profiles>(?:\[[^\]]*\])|\[\]),\s*"
    r"published:\s*(?P<published>true|false),\s*"
    r"rationale:\s*\"(?P<rationale>(?:\\.|[^\"\\])*)\",?\s*\}",
    re.DOTALL,
)

_BUNDLE_BLOCK_RE = re.compile(
    r"SkillBundleSpec\s*\{\s*"
    r"id:\s*\"(?P<id>[^\"]+)\",\s*"
    r"profiles:\s*&(?P<profiles>(?:\[[^\]]*\])|\[\]),\s*"
    r"published:\s*(?P<published>true|false),\s*\}",
    re.DOTALL,
)

_PUBLISHED_PROFILES_RE = re.compile(
    r"pub\s+const\s+PUBLISHED_PROFILES:\s*&\["
    r"(?P<body>(?:[^;\]]|\\.)*)\]\s*=\s*&\[(?P<entries>(?:[^]]|\](?!,))*?)\]\s*;",
    re.DOTALL,
)

_PROFILE_ENTRY_RE = re.compile(
    r'\(\s*"(?P<id>[^"]+)"\s*,\s*"(?P<desc>(?:\\.|[^"\\])*)"\s*\)'
)


def _parse_profiles_literal(literal: str) -> list[str]:
    """Parse a Rust slice literal like ``&["core", "reviewer"]`` or ``&[]``."""
    literal = literal.strip()
    if literal in ("&[]", "&[\"\": \"\"]"):
        return []
    if not (literal.startswith("&[") and literal.endswith("]")):
        raise ValueError(f"unexpected profiles literal: {literal!r}")
    body = literal[2:-1]
    return [token.strip().strip('"') for token in body.split(",") if token.strip()]


# Mapping from `ArtifactKind::*` Rust identifier (lowercased) to the canonical
# public stem returned by ``ArtifactKind::stem()`` in `release_contract.rs`.
# Components that are pure metadata (BundleManifest, ReleaseInventory,
# Checksums) are excluded from the public profile component list.
_PUBLIC_STEM_OVERRIDE = {
    "daemoncli": "cognicode-mcp",
    "cogh": "cogh",
    "cognicode": "cognicode",
}
_METADATA_KINDS = {"bundle_manifest", "release_inventory", "checksums"}


def parse_release_contract(source: str) -> dict[str, object]:
    """Parse the textual source of ``release_contract.rs``.

    Raises ``ValueError`` if a structural key is missing.
    """

    components: list[dict[str, object]] = []
    for match in _COMPONENT_BLOCK_RE.finditer(source):
        kind = match.group("kind")
        kind_lower = kind.lower()
        if kind_lower in _METADATA_KINDS:
            continue
        stem = _PUBLIC_STEM_OVERRIDE.get(kind_lower, kind_lower)
        components.append(
            {
                "stem": stem,
                "profiles": _parse_profiles_literal("&" + match.group("profiles")),
                "published": match.group("published") == "true",
                "rationale": match.group("rationale"),
            }
        )

    skill_bundles: list[dict[str, object]] = []
    for match in _BUNDLE_BLOCK_RE.finditer(source):
        skill_bundles.append(
            {
                "id": match.group("id"),
                "profiles": _parse_profiles_literal("&" + match.group("profiles")),
                "published": match.group("published") == "true",
            }
        )

    published_match = _PUBLISHED_PROFILES_RE.search(source)
    if not published_match:
        raise ValueError("PUBLISHED_PROFILES constant not found")
    entries = published_match.group("entries")
    pairs: list[list[str]] = []
    for entry_match in _PROFILE_ENTRY_RE.finditer(entries):
        pairs.append([entry_match.group("id"), entry_match.group("desc")])

    return {
        "published_profile_pairs": pairs,
        "components": components,
        "skill_bundles": skill_bundles,
    }


def parse_skill_maturity_alphabet(source: str) -> list[str]:
    """Parse the accepted maturity values from ``skill.rs``."""
    pattern = re.compile(
        r"if\s*!\[\s*\"(?P<a>[^\"]+)\"\s*,\s*\"(?P<b>[^\"]+)\"\s*,"
        r"\s*\"(?P<c>[^\"]+)\"\s*,\s*\"(?P<d>[^\"]+)\""
    )
    match = pattern.search(source)
    if not match:
        raise ValueError("skill maturity alphabet not found")
    return [match.group(name) for name in ("a", "b", "c", "d")]


def load_release_contract(root: Path) -> dict[str, object]:
    """Load and parse ``release_contract.rs`` from the repository ``root``."""
    path = root / "crates/cognicode-cli/src/cmd/release_contract.rs"
    return parse_release_contract(path.read_text(encoding="utf-8"))


def load_skill_maturity_alphabet(root: Path) -> list[str]:
    """Load and parse ``skill.rs`` to read the maturity alphabet."""
    path = root / "crates/cognicode-cli/src/cmd/skill.rs"
    return parse_skill_maturity_alphabet(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    import json
    import sys

    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".").resolve()
    contract = load_release_contract(root)
    alphabet = load_skill_maturity_alphabet(root)
    print(json.dumps({"contract": contract, "maturity": alphabet}, indent=2))
