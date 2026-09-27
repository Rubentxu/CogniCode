#!/usr/bin/env python3
"""Generate CogniCode's evidence-bound language and platform matrices."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

LANGUAGE_SCHEMA_VERSION = "cognicode.languages/v1"
PLATFORM_SCHEMA_VERSION = "cognicode.platforms/v1"
SUPPORT_LEVELS = {"certified", "supported", "experimental", "unsupported"}

# These names are the explicit public spelling used by the runtime Language enum.
LANGUAGE_INFO = {
    "Python": ("python", "Python"),
    "Rust": ("rust", "Rust"),
    "JavaScript": ("javascript", "JavaScript"),
    "TypeScript": ("typescript", "TypeScript"),
    "Go": ("go", "Go"),
    "Java": ("java", "Java"),
    "C": ("c", "C"),
    "Cpp": ("cpp", "C++"),
    "CSharp": ("csharp", "C#"),
    "Hcl": ("hcl", "HCL"),
    "Yaml": ("yaml", "YAML"),
    "Ruby": ("ruby", "Ruby"),
    "Php": ("php", "PHP"),
    "Swift": ("swift", "Swift"),
    "Scala": ("scala", "Scala"),
    "Lua": ("lua", "Lua"),
    "Zig": ("zig", "Zig"),
    "Dart": ("dart", "Dart"),
    "Groovy": ("groovy", "Groovy"),
    "Elixir": ("elixir", "Elixir"),
    "Erlang": ("erlang", "Erlang"),
    "Haskell": ("haskell", "Haskell"),
    "Julia": ("julia", "Julia"),
    "Bash": ("bash", "Bash"),
    "R": ("r", "R"),
    "PowerShell": ("powershell", "PowerShell"),
    "Json": ("json", "JSON"),
    "Fortran": ("fortran", "Fortran"),
    "Verilog": ("verilog", "Verilog"),
    "SystemVerilog": ("systemverilog", "SystemVerilog"),
}

DEFERRED_PUBLIC_PLATFORMS = [
    {
        "id": "macos-arm64",
        "target": "aarch64-apple-darwin",
        "name": "macOS arm64",
        "support_level": "unsupported",
        "evidence": [
            "No macOS lane exists in .github/workflows/release.yml",
            "A-027 tracks macOS arm64 build/certification",
        ],
    },
    {
        "id": "macos-x86-64",
        "target": "x86_64-apple-darwin",
        "name": "macOS x86_64",
        "support_level": "unsupported",
        "evidence": [
            "No macOS lane exists in .github/workflows/release.yml",
            "A-027 tracks macOS build/certification",
        ],
    },
    {
        "id": "windows-x86-64",
        "target": "x86_64-pc-windows-msvc",
        "name": "Windows x86_64",
        "support_level": "unsupported",
        "evidence": [
            "No Windows lane exists in .github/workflows/release.yml",
            "A-028 tracks Windows x64 build/certification",
        ],
    },
]


def run_git(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ValueError(result.stderr.strip() or "git command failed")
    return result.stdout.strip()


def read_required(root: Path, relative: str) -> str:
    path = root / relative
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ValueError(f"required source is unreadable: {relative}") from exc
    if not content.strip():
        raise ValueError(f"required source is empty: {relative}")
    return content


def workspace_version(root: Path) -> str:
    cargo = read_required(root, "Cargo.toml")
    match = re.search(
        r"(?ms)^\[workspace\.package\]\s+.*?^version\s*=\s*[\"']([^\"']+)[\"']",
        cargo,
    )
    if not match:
        raise ValueError("workspace package version is missing")
    return match.group(1)


def parser_variants(root: Path) -> list[str]:
    source = read_required(
        root, "crates/cognicode-core/src/infrastructure/parser/tree_sitter_parser.rs"
    )
    match = re.search(r"pub enum Language\s*\{(?P<body>.*?)^\}", source, re.MULTILINE | re.DOTALL)
    if not match:
        raise ValueError("Language enum is missing")
    variants = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*,", match.group("body"), re.MULTILINE)
    if not variants:
        raise ValueError("Language enum has no variants")
    unknown = sorted(set(variants) - set(LANGUAGE_INFO))
    if unknown:
        raise ValueError(f"unmapped Language variants: {unknown}")
    mappings = set(re.findall(r"Language::([A-Z][A-Za-z0-9_]*)\s*=>", source))
    missing_mappings = sorted(set(variants) - mappings)
    if missing_mappings:
        raise ValueError(f"Language variants without tree-sitter mappings: {missing_mappings}")
    return variants


def configured_variants(root: Path) -> set[str]:
    source = read_required(root, "crates/cognicode-core/src/infrastructure/parser/language_config.rs")
    return set(re.findall(r"language:\s*Language::([A-Z][A-Za-z0-9_]*)\s*,", source))


def language_extensions(root: Path) -> dict[str, list[str]]:
    source = read_required(root, "crates/cognicode-core/src/infrastructure/parser/language_config.rs")
    matches = re.finditer(
        r"language:\s*Language::(?P<variant>[A-Z][A-Za-z0-9_]*)\s*,\s*"
        r"extensions:\s*&\[(?P<extensions>.*?)\]",
        source,
        re.DOTALL,
    )
    result: dict[str, list[str]] = {}
    for match in matches:
        result[match.group("variant")] = re.findall(r'["\']([^"\']+)["\']', match.group("extensions"))
    return result


def acceptance_variants(root: Path) -> set[str]:
    sources = [
        read_required(root, "crates/cognicode-core/tests/m06_acceptance.rs"),
        read_required(root, "crates/cognicode-core/tests/m10_acceptance.rs"),
    ]
    variants: set[str] = set()
    for source in sources:
        variants.update(re.findall(r"Language::([A-Z][A-Za-z0-9_]*)", source))
    return variants


def release_platforms(root: Path) -> list[tuple[str, str]]:
    source = read_required(root, ".github/workflows/release.yml")
    pairs = re.findall(
        r"-\s+platform:\s*([a-z0-9-]+)\s+runner:.*?rust_target:\s*([a-z0-9_-]+)",
        source,
        re.DOTALL,
    )
    expected = {
        ("linux-x86-64", "x86_64-unknown-linux-gnu"),
        ("linux-aarch64", "aarch64-unknown-linux-gnu"),
    }
    if set(pairs) != expected:
        raise ValueError(f"release platform lanes drifted: {pairs!r}")
    return sorted(pairs)


def musl_target(root: Path) -> str:
    source = read_required(root, ".github/workflows/ci.yml")
    target = "x86_64-unknown-linux-musl"
    if target not in source:
        raise ValueError("CI musl target is missing")
    return target


def build_documents(root: Path, source_commit: str) -> tuple[dict[str, Any], dict[str, Any]]:
    if not re.fullmatch(r"[0-9a-f]{40}", source_commit):
        raise ValueError("source_commit must be a lowercase 40-character Git SHA")
    variants = parser_variants(root)
    configured = configured_variants(root)
    missing_config = sorted(set(variants) - configured)
    if missing_config:
        raise ValueError(f"Language variants without runtime configuration: {missing_config}")
    accepted = acceptance_variants(root)
    unknown_acceptance = sorted(accepted - set(variants))
    if unknown_acceptance:
        raise ValueError(f"acceptance references unknown languages: {unknown_acceptance}")
    extensions = language_extensions(root)
    if set(extensions) != set(variants):
        raise ValueError("language configuration extensions do not cover the Language enum")

    languages = []
    for variant in variants:
        language_id, name = LANGUAGE_INFO[variant]
        evidence = [
            "Language enum and tree-sitter mapping in tree_sitter_parser.rs",
            "LanguageConfig extension and symbol mapping in language_config.rs",
        ]
        if variant in accepted:
            support_level = "supported"
            evidence.append("Focused parser acceptance coverage in m06_acceptance.rs or m10_acceptance.rs")
        else:
            support_level = "experimental"
            evidence.append("No focused acceptance scenario found in m06_acceptance.rs or m10_acceptance.rs")
        languages.append(
            {
                "id": language_id,
                "name": name,
                "support_level": support_level,
                "extensions": sorted(extensions[variant]),
                "evidence": evidence,
            }
        )

    platforms = []
    for platform_id, target in release_platforms(root):
        platforms.append(
            {
                "id": platform_id,
                "target": target,
                "name": platform_id.replace("-", " ").title(),
                "support_level": "certified",
                "evidence": [
                    "Native publishable lane in .github/workflows/release.yml",
                    "Validated by .github/workflows/release-validate.yml",
                ],
            }
        )
    platforms.append(
        {
            "id": "linux-x86-64-musl",
            "target": musl_target(root),
            "name": "Linux x86_64 musl",
            "support_level": "experimental",
            "evidence": [
                "CI smoke build in .github/workflows/ci.yml",
                "No publishable release lane in .github/workflows/release.yml",
            ],
        }
    )
    platforms.extend(DEFERRED_PUBLIC_PLATFORMS)

    version = workspace_version(root)
    languages_document = {
        "schema_version": LANGUAGE_SCHEMA_VERSION,
        "product": "cognicode",
        "version": version,
        "source_commit": source_commit,
        "languages": sorted(languages, key=lambda item: item["id"]),
    }
    platforms_document = {
        "schema_version": PLATFORM_SCHEMA_VERSION,
        "product": "cognicode",
        "version": version,
        "source_commit": source_commit,
        "platforms": sorted(platforms, key=lambda item: item["id"]),
    }
    validate_documents(languages_document, platforms_document)
    return languages_document, platforms_document


def validate_documents(languages: dict[str, Any], platforms: dict[str, Any]) -> None:
    for document, schema_version, collection_name in (
        (languages, LANGUAGE_SCHEMA_VERSION, "languages"),
        (platforms, PLATFORM_SCHEMA_VERSION, "platforms"),
    ):
        if document.get("schema_version") != schema_version:
            raise ValueError(f"invalid schema_version for {collection_name}")
        if document.get("product") != "cognicode":
            raise ValueError(f"invalid product for {collection_name}")
        if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", document.get("version", "")):
            raise ValueError(f"invalid version for {collection_name}")
        if not re.fullmatch(r"[0-9a-f]{40}", document.get("source_commit", "")):
            raise ValueError(f"invalid source_commit for {collection_name}")
        entries = document.get(collection_name)
        if not isinstance(entries, list) or not entries:
            raise ValueError(f"{collection_name} must be a non-empty list")
        ids = [entry.get("id") for entry in entries]
        if any(not isinstance(item, str) or not item for item in ids):
            raise ValueError(f"{collection_name} contains an invalid id")
        if len(ids) != len(set(ids)):
            raise ValueError(f"duplicate ids in {collection_name}")
        if ids != sorted(ids):
            raise ValueError(f"{collection_name} must be sorted by id")
        for entry in entries:
            if entry.get("support_level") not in SUPPORT_LEVELS:
                raise ValueError(f"invalid support level in {collection_name}")
            evidence = entry.get("evidence")
            if not isinstance(evidence, list) or not evidence or any(not item for item in evidence):
                raise ValueError(f"missing evidence in {collection_name}")


def render(document: dict[str, Any]) -> str:
    return json.dumps(document, indent=2, sort_keys=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--languages-output", type=Path, default=Path("product/languages.json"))
    parser.add_argument("--platforms-output", type=Path, default=Path("product/platforms.json"))
    parser.add_argument("--source-commit")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    languages_output = args.languages_output if args.languages_output.is_absolute() else root / args.languages_output
    platforms_output = args.platforms_output if args.platforms_output.is_absolute() else root / args.platforms_output
    try:
        source_commit = args.source_commit or run_git(root, "rev-parse", "HEAD")
        languages, platforms = build_documents(root, source_commit)
        rendered_languages = render(languages)
        rendered_platforms = render(platforms)
        if args.check:
            if languages_output.read_text(encoding="utf-8") != rendered_languages:
                raise ValueError(f"generated languages differ from {languages_output}")
            if platforms_output.read_text(encoding="utf-8") != rendered_platforms:
                raise ValueError(f"generated platforms differ from {platforms_output}")
        else:
            languages_output.parent.mkdir(parents=True, exist_ok=True)
            platforms_output.parent.mkdir(parents=True, exist_ok=True)
            languages_output.write_text(rendered_languages, encoding="utf-8")
            platforms_output.write_text(rendered_platforms, encoding="utf-8")
    except (OSError, ValueError) as exc:
        print(f"support matrix generation failed: {exc}", file=sys.stderr)
        return 1
    print(f"support matrices {'validated' if args.check else 'generated'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
