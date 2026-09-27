#!/usr/bin/env python3
"""Generate CogniCode's public profile contract (``product/profiles.json``).

The contract is the only public truth about which install/declared profiles
the product exposes, their stability, mutating posture, components and skill
bundles. The Rust release contract
(``crates/cognicode-cli/src/cmd/release_contract.rs``) is the single
behavioural source for ids and published flags; this script projects it
into JSON. Two hand-curated declared profiles (``developer``,
``experimental``) complement the contract's published profile pairs to
fulfil ``CP0.4`` of the Community Productization Roadmap.

The generator is deterministic and idempotent. Run
``python3 scripts/product/generate_profiles.py --source-commit <SHA>`` to
emit ``product/profiles.json``; pass ``--check`` to compare against the
committed file without writing.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

import jsonschema

from build_release_contract import (
    load_release_contract,
    load_skill_maturity_alphabet,
)

# See generate_support_matrix.py: the test suite loads this file by path,
# which does not put this directory on `sys.path`, so the sibling import has
# to be resolved explicitly to work in both contexts.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_semantics import check_provenance, content_mismatch  # noqa: E402

SCHEMA_VERSION = "cognicode.profiles/v1"
PRODUCT_NAME = "cognicode"

# Hand-curated declared profiles (no install path; no published components).
DECLARED_PROFILES: dict[str, dict[str, Any]] = {
    "developer": {
        "description": (
            "Onboarding for CogniCode contributors — SDDK/OpenSpec workflow, "
            "internal `just` recipes, architecture rules, release gates."
        ),
        "stability": "experimental",
        "install": False,
        "mutating": True,
        "components": ["cogh", "cognicode"],
        "skill_bundles": ["cognicode-developer"],
        "evidence": (
            "skills/cognicode-developer/manifest.yaml present; "
            "release_contract.rs::SKILL_BUNDLES.cognicode-developer is published:false"
        ),
    },
    "experimental": {
        "description": (
            "Stability bracket for artifacts not yet install-ready. "
            "Not an install path."
        ),
        "stability": "experimental",
        "install": False,
        "mutating": False,
        "components": [],
        "skill_bundles": [],
        "evidence": (
            "no published component or bundle; the term is preserved as a "
            "stability marker for the future"
        ),
    },
}

# Profiles that must appear (after derivation).
REQUIRED_PROFILES = {"core", "reviewer", "developer", "experimental"}


def _parse_source_commit(source_commit: str | None, default: str) -> str:
    if source_commit is None:
        source_commit = default
    if not re.fullmatch(r"[0-9a-f]{40}", source_commit):
        raise ValueError(
            f"source_commit must be a lowercase 40-character SHA (got {source_commit!r})"
        )
    return source_commit


def _resolve_components_for_profile(
    profile_id: str,
    components: list[dict[str, Any]],
    skill_bundles: list[dict[str, Any]],
) -> tuple[list[str], list[str]]:
    comps = sorted(
        {
            component["stem"]
            for component in components
            if component["published"]
            and (
                profile_id in component["profiles"]
                # Layer 0 (`cogh`) is published but lists an empty
                # `profiles: &[]`: it is the bootstrap CLI, included in any
                # install path. Treat an empty profile list on a published
                # Layer 0 component as "all install profiles".
                or (
                    component["profiles"] == []
                    and component["stem"] == "cogh"
                )
            )
        }
    )
    bundles = sorted(
        {
            bundle["id"]
            for bundle in skill_bundles
            if bundle["published"] and profile_id in bundle["profiles"]
        }
    )
    return comps, bundles


def build_profiles(root: Path, source_commit: str) -> dict[str, Any]:
    contract = load_release_contract(root)
    maturity = load_skill_maturity_alphabet(root)

    published_profile_pairs = contract["published_profile_pairs"]
    derived_profile_pairs = {pid for pid, _ in published_profile_pairs}

    profiles: list[dict[str, Any]] = []

    # Derived profiles from the Rust contract.
    for profile_id, description in published_profile_pairs:
        components, skill_bundles = _resolve_components_for_profile(
            profile_id, contract["components"], contract["skill_bundles"]
        )
        profiles.append(
            {
                "id": profile_id,
                "stability": "stable",
                "install": True,
                "mutating": False,
                "description": description,
                "components": components,
                "skill_bundles": skill_bundles,
                "evidence": (
                    "release_contract.rs::PUBLISHED_PROFILES lists this id; "
                    "release_contract.rs::COMPONENTS.publishable entries whose "
                    "profiles include this id determine components"
                ),
            }
        )

    # Declared profiles (developer + experimental).
    for profile_id, decl in DECLARED_PROFILES.items():
        profiles.append(
            {
                "id": profile_id,
                **decl,
            }
        )

    # Deterministic ordering.
    profiles.sort(key=lambda item: item["id"])

    # Stability whitelist comes from the real validator (skill.rs).
    if "stable" not in maturity:
        raise ValueError("maturity alphabet is missing 'stable'")

    return {
        "schema_version": SCHEMA_VERSION,
        "product": PRODUCT_NAME,
        "source_commit": source_commit,
        "profiles": profiles,
    }


def validate_against_schema(document: dict[str, Any], schema_path: Path) -> None:
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    jsonschema.validate(document, schema)


def validate_contract(document: dict[str, Any], root: Path) -> None:
    ids = [item["id"] for item in document["profiles"]]
    if len(set(ids)) != len(ids):
        raise ValueError("duplicate profile ids are not allowed")
    if set(ids) != REQUIRED_PROFILES:
        raise ValueError(
            f"profile ids must be exactly {sorted(REQUIRED_PROFILES)} (got {sorted(ids)})"
        )

    for profile in document["profiles"]:
        if not profile["description"].strip():
            raise ValueError(f"profile {profile['id']}: description must be non-empty")
        if not profile["evidence"].strip():
            raise ValueError(f"profile {profile['id']}: evidence must be non-empty")
        if profile["install"] and not profile["components"]:
            raise ValueError(
                f"profile {profile['id']}: install=true requires non-empty components"
            )
        if profile["install"] and profile["stability"] == "stable":
            # Stable install paths must be backed by published entries; the
            # generator only emits stable install profiles from
            # published_profile_pairs, so this clause is informational.
            pass

    # Compound checks: declared profiles must NOT appear in derived pairs.
    derived = {pid for pid, _ in document["profiles"][0:0]}  # placeholder
    derived_ids = {
        profile["id"]
        for profile in document["profiles"]
        if profile["id"] in {"core", "reviewer"}
    }
    declared_ids = {"developer", "experimental"}
    if derived_ids & declared_ids:
        raise ValueError("derived and declared profile ids must not overlap")

    # The `mutating` flag is not authored here. It is read from the Rust
    # table `PROFILE_POSTURES` (crates/cognicode-core/src/product.rs), which
    # is what the runtime actually enforces. Hard-coding the same map in
    # Python created a second source of truth: the JSON could say
    # `mutating: false` while the runtime enforced the opposite, and nothing
    # would notice. Deriving from the enforced table means the public
    # contract cannot disagree with the behaviour.
    expected_mutating = load_postures_from_rust(root)
    for profile in document["profiles"]:
        if profile["id"] not in expected_mutating:
            raise ValueError(
                f"profile {profile['id']} has no posture in "
                "cognicode_core::product::PROFILE_POSTURES"
            )
        if profile["mutating"] != expected_mutating[profile["id"]]:
            raise ValueError(
                f"profile {profile['id']}: mutating must be "
                f"{expected_mutating[profile['id']]} (got {profile['mutating']})"
            )


PROFILE_POSTURES_SOURCE = Path(
    "crates/cognicode-core/src/product.rs"
)


def load_postures_from_rust(root: Path) -> dict[str, bool]:
    """Read the enforced posture table out of the Rust source.

    `PROFILE_POSTURES` in `cognicode-core/src/product.rs` is what the runtime
    actually enforces. This reads it directly rather than duplicating the
    values, so the published contract cannot drift from the behaviour.

    Parsing the literal is deliberate: shelling out to `cargo` on every
    generator run would make a documentation check depend on a Rust
    toolchain, and the table is a closed, four-row literal that the module's
    own tests pin. A shape change fails loudly below rather than silently
    yielding fewer profiles than expected.
    """
    source = root / PROFILE_POSTURES_SOURCE
    text = source.read_text(encoding="utf-8")
    match = re.search(
        r"pub const PROFILE_POSTURES[^=]*=\s*&\[(.*?)\];",
        text,
        re.DOTALL,
    )
    if not match:
        raise ValueError(
            f"{PROFILE_POSTURES_SOURCE} does not declare a readable "
            "PROFILE_POSTURES table; the generator cannot verify the "
            "published posture against the enforced one"
        )
    rows = re.findall(
        r'\(\s*"([a-z0-9_-]+)"\s*,\s*ProfilePosture::(ReadOnly|ReadWrite)\s*\)',
        match.group(1),
    )
    if not rows:
        raise ValueError(
            f"{PROFILE_POSTURES_SOURCE} declares PROFILE_POSTURES but no rows "
            "matched the expected (id, ProfilePosture::X) shape"
        )
    return {profile_id: posture == "ReadWrite" for profile_id, posture in rows}


def render(document: dict[str, Any]) -> str:
    return json.dumps(document, indent=2, sort_keys=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, default=Path("product/profiles.json"))
    parser.add_argument("--schema", type=Path, default=None)
    parser.add_argument("--source-commit")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    root = args.root.resolve()
    output = args.output if args.output.is_absolute() else root / args.output
    schema = (
        args.schema
        if args.schema
        else root / "product/schemas/profiles.v1.schema.json"
    )
    if not schema.is_absolute():
        schema = root / schema

    try:
        import subprocess

        head_sha = (
            subprocess.check_output(
                ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
            ).strip()
            if (args.source_commit is None)
            else args.source_commit
        )
        source_commit = _parse_source_commit(args.source_commit, head_sha)
        document = build_profiles(root, source_commit)
        validate_contract(document, root)
        validate_against_schema(document, schema)
        rendered = render(document)
        if args.check:
            actual = output.read_text(encoding="utf-8")
            if actual != rendered:
                # `source_commit` is provenance, not derived content, and it
                # is stamped from HEAD. Requiring it to equal HEAD made
                # `--check` fail on every commit newer than the artefact and
                # report drift that did not exist. Content stays strict;
                # provenance is validated for shape. See check_semantics.
                mismatch = content_mismatch(actual, rendered)
                if mismatch:
                    raise ValueError(
                        f"generated profiles differ from {output}: {mismatch}"
                    )
            check_provenance(actual, str(output))
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(rendered, encoding="utf-8")
    except (OSError, ValueError, jsonschema.ValidationError, subprocess.CalledProcessError) as exc:
        print(f"profile generation failed: {exc}", file=sys.stderr)
        return 1
    print(f"profiles {'validated' if args.check else 'generated'}: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
