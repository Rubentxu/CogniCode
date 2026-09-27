#!/usr/bin/env python3
"""Generate and validate CogniCode's canonical product manifest."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "cognicode.product/v1"
MCP_PROTOCOL_REVISION = "2025-03-26"
SUPPORT_LEVELS = {"certified", "supported", "experimental", "unsupported"}
PUBLIC_BINARIES = [
    {
        "name": "cogh",
        "kind": "bootstrap-cli",
        "package": "cognicode-cli",
        "support_level": "supported",
    },
    {
        "name": "cognicode",
        "kind": "analysis-cli",
        "package": "cognicode-cli",
        "support_level": "supported",
    },
    {
        "name": "cognicode-mcp",
        "kind": "mcp-stdio-server",
        "package": "cognicode-mcp",
        "support_level": "supported",
    },
]
PUBLIC_PROFILES_PATH = Path("product/profiles.json")
PROFILES_SCHEMA_VERSION = "cognicode.profiles/v1"
LANGUAGE_NAMES = {
    "C": "c",
    "Cpp": "cpp",
    "CSharp": "csharp",
    "Hcl": "hcl",
    "Yaml": "yaml",
    "Php": "php",
    "JavaScript": "javascript",
    "TypeScript": "typescript",
    "PowerShell": "powershell",
    "SystemVerilog": "systemverilog",
}


def run_git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git command failed")
    return result.stdout.strip()


def workspace_version(root: Path) -> str:
    cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(
        r"(?ms)^\[workspace\.package\]\s+.*?^version\s*=\s*[\"']([^\"']+)[\"']",
        cargo,
    )
    if not match:
        raise ValueError("workspace package version is missing")
    return match.group(1)


def parser_languages(root: Path) -> list[str]:
    source = (root / "crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs").read_text(
        encoding="utf-8"
    )
    match = re.search(r"pub enum Language\s*\{(?P<body>.*?)^\}", source, re.MULTILINE | re.DOTALL)
    if not match:
        raise ValueError("Language enum is missing")
    variants = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*,", match.group("body"), re.MULTILINE)
    if not variants:
        raise ValueError("Language enum has no variants")
    return [LANGUAGE_NAMES.get(variant, variant.lower()) for variant in variants]


def certified_platforms(root: Path) -> list[str]:
    workflow = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
    targets = sorted(set(re.findall(r"([a-z0-9_]+-unknown-linux-gnu)", workflow)))
    expected = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]
    if targets != expected:
        raise ValueError(f"release workflow certified target set changed: {targets!r}")
    return targets


def build_manifest(root: Path, source_commit: str) -> dict[str, Any]:
    version = workspace_version(root)
    languages = [
        {
            "name": name,
            "support_level": "experimental",
            "evidence": "tree-sitter parser enum; detailed support matrix is A-005",
        }
        for name in parser_languages(root)
    ]
    platforms = [
        {
            "target": target,
            "support_level": "certified",
            "evidence": ".github/workflows/release.yml Linux GNU release lane",
        }
        for target in certified_platforms(root)
    ]
    profiles = _load_public_profiles(root)
    return {
        "schema_version": SCHEMA_VERSION,
        "product": "cognicode",
        "version": version,
        "source_commit": source_commit,
        "public_surface": {
            "binaries": PUBLIC_BINARIES,
            "profiles": profiles,
            "mcp": {
                "transport": "stdio",
                "protocol_revision": MCP_PROTOCOL_REVISION,
                "tool_catalog": "tools.json",
            },
        },
        "languages": languages,
        "platforms": platforms,
        "artifacts": {
            "tools": "tools.json",
            "languages": "languages.json",
            "platforms": "platforms.json",
            "profiles": "profiles.json",
        },
        "feature_flags": {
            "read_only_mode": True,
            "mutations": "developer-explicit",
            "telemetry_default": "off",
            "network_default": "denied",
        },
    }


def _load_public_profiles(root: Path) -> list[dict[str, Any]]:
    """Load public profiles by projecting ``product/profiles.json``.

    The contract is the single source of truth for the public profile
    surface; the manifest MUST NOT hard-code it (lesson EvidenceStore).
    """
    path = root / PUBLIC_PROFILES_PATH if not PUBLIC_PROFILES_PATH.is_absolute() else PUBLIC_PROFILES_PATH
    if not path.exists():
        raise ValueError(
            f"public profile contract is missing at {path}; "
            f"run 'python3 scripts/product/generate_profiles.py --source-commit <SHA>'"
        )
    document = json.loads(path.read_text(encoding="utf-8"))
    if document.get("schema_version") != PROFILES_SCHEMA_VERSION:
        raise ValueError(
            f"public profile contract uses {document.get('schema_version')!r}, "
            f"expected {PROFILES_SCHEMA_VERSION!r}"
        )
    projected = [
        {
            "name": profile["id"],
            "description": profile["description"],
            "mutating": profile["mutating"],
            "install": profile["install"],
            "stability": profile["stability"],
        }
        for profile in document["profiles"]
    ]
    return projected


def validate_manifest(manifest: dict[str, Any]) -> None:
    required = {
        "schema_version",
        "product",
        "version",
        "source_commit",
        "public_surface",
        "languages",
        "platforms",
        "artifacts",
        "feature_flags",
    }
    missing = required - manifest.keys()
    if missing:
        raise ValueError(f"missing required fields: {sorted(missing)}")
    if manifest["schema_version"] != SCHEMA_VERSION:
        raise ValueError(f"unsupported schema_version: {manifest['schema_version']!r}")
    if not re.fullmatch(r"[0-9a-f]{40}", manifest["source_commit"]):
        raise ValueError("source_commit must be a lowercase 40-character Git SHA")
    surface = manifest["public_surface"]
    names = [item["name"] for item in surface["binaries"]]
    if names != [item["name"] for item in PUBLIC_BINARIES]:
        raise ValueError(f"public binary surface drifted: {names!r}")
    profile_names = [item["name"] for item in surface["profiles"]]
    if sorted(profile_names) != ["core", "developer", "experimental", "reviewer"]:
        raise ValueError(f"public profile surface drifted: {profile_names!r}")
    profile_mutations = {item["name"]: item["mutating"] for item in surface["profiles"]}
    expected_mutations = {
        "core": False,
        "developer": True,
        "experimental": False,
        "reviewer": False,
    }
    if profile_mutations != expected_mutations:
        raise ValueError(
            f"public profile mutating posture drifted: {profile_mutations!r}"
        )
    if surface["mcp"]["protocol_revision"] != MCP_PROTOCOL_REVISION:
        raise ValueError("MCP protocol revision is not the verified revision")
    if "tools" in surface or "tools" in manifest:
        raise ValueError("product manifest must not duplicate the tool catalog")
    for collection in (manifest["languages"], manifest["platforms"]):
        for item in collection:
            if item["support_level"] not in SUPPORT_LEVELS:
                raise ValueError(f"unsupported support level: {item['support_level']!r}")
    certified = {
        item["target"] for item in manifest["platforms"] if item["support_level"] == "certified"
    }
    if certified != set(certified_platforms_from_manifest(manifest)):
        raise ValueError("certified platform set is not the release-certified Linux set")


def certified_platforms_from_manifest(manifest: dict[str, Any]) -> list[str]:
    return ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"]


def render(manifest: dict[str, Any]) -> str:
    return json.dumps(manifest, indent=2, sort_keys=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, default=Path("product/product-manifest.json"))
    parser.add_argument("--source-commit")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    output = args.output if args.output.is_absolute() else root / args.output
    try:
        source_commit = args.source_commit or run_git(root, "rev-parse", "HEAD")
        manifest = build_manifest(root, source_commit)
        validate_manifest(manifest)
        rendered = render(manifest)
        if args.check:
            actual = output.read_text(encoding="utf-8")
            if actual != rendered:
                raise ValueError(f"generated manifest differs from {output}")
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(rendered, encoding="utf-8")
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"product manifest generation failed: {exc}", file=sys.stderr)
        return 1
    print(f"product manifest {'validated' if args.check else 'generated'}: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
