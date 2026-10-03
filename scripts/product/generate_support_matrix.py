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

# Loaded as a sibling module when this file runs as a script, and by path
# when the test suite imports it through `importlib.util.spec_from_file_location`.
# The second form does not put this directory on `sys.path`, so a bare
# `import check_semantics` would work in CI and fail in the test suite. Resolve
# the sibling explicitly so both paths behave the same.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_lane  # noqa: E402
import language_source  # noqa: E402
from check_semantics import check_provenance, content_mismatch  # noqa: E402

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

# Platforms the product does not claim. The reason each is unsupported is that
# the release lane does not build it, and that is now checked rather than
# asserted in prose: `deferred_evidence` verifies the target is absent from
# `val targets` and fails if it appears.
#
# The strings used to read "No macOS lane exists in .github/workflows/release.yml"
# — a claim about a file. If someone added a macOS target to the release lane,
# the evidence would have kept saying no lane existed while the lane built one,
# which is worse than having no evidence at all.
DEFERRED_PUBLIC_PLATFORMS = [
    {
        "id": "macos-arm64",
        "target": "aarch64-apple-darwin",
        "name": "macOS arm64",
        "support_level": "unsupported",
        "tracking": "A-027 tracks macOS arm64 build/certification",
    },
    {
        "id": "macos-x86-64",
        "target": "x86_64-apple-darwin",
        "name": "macOS x86_64",
        "support_level": "unsupported",
        "tracking": "A-027 tracks macOS build/certification",
    },
    {
        "id": "windows-x86-64",
        "target": "x86_64-pc-windows-msvc",
        "name": "Windows x86_64",
        "support_level": "unsupported",
        "tracking": "A-028 tracks Windows x64 build/certification",
    },
]


def deferred_evidence(root: Path, entry: dict[str, object]) -> list[str]:
    """Evidence for a platform this product does not claim: verified absence.

    The evidence for "unsupported" is a negative fact — nothing builds this —
    so the check is that the release lane does not name the target. A
    hand-written sentence claiming absence cannot notice the day it stops
    being true.
    """
    target = str(entry["target"])
    if target in release_lane.release_targets(root):
        raise ValueError(
            f"{release_lane.RELEASE_LANE} builds `{target}`, but "
            f"`{entry['id']}` is listed here as unsupported. The release lane "
            f"is the authority for what is published; either the platform is "
            f"now supported and this entry is stale, or the lane is building "
            f"something the product does not claim."
        )
    return [
        f"Not built by {release_lane.RELEASE_LANE} (no entry in `val targets`)",
        str(entry["tracking"]),
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
    """The `Language` enum's variants, checked against the grammar mappings.

    The declaration and the mapping live in two files on purpose, so this
    reads both. It used to read only the parser file, which is where the
    enum was before it was lifted into the domain layer; see
    `language_source.py` for why that is not a layering decision to preserve.
    """
    variants = language_source.language_variants(root)
    unknown = sorted(set(variants) - set(LANGUAGE_INFO))
    if unknown:
        raise ValueError(f"unmapped Language variants: {unknown}")
    missing_mappings = language_source.unmapped_variants(root, variants)
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


def release_platforms(root: Path) -> list[release_lane.ReleasePlatform]:
    """Every published platform, from the lane that publishes it.

    This read `.github/workflows/release.yml` with a regex shaped around a
    YAML matrix entry, and then asserted the pairs against its own `expected`
    set. Both halves are gone: `release_lane.py` reads the declarations in
    `release-candidate.pipeline.kts` and reconciles them with each other.

    A matrix regex was the fragile part. It matched
    `platform: <id> runner: … rust_target: <triple>` across a YAML block, so
    reformatting one entry — adding a comment, reordering keys — would have
    read as "the release platform lanes drifted" and failed a manifest
    generation for a reason that had nothing to do with the platforms.

    The return type names both parts. This function's first version returned a
    plain tuple and the one caller unpacked it in the opposite order, producing
    a manifest whose `id` was a cargo triple; the attributes make that a
    mistake you can see rather than a document that looks fine.
    """
    return release_lane.release_platforms(root)


def musl_target(root: Path) -> str:
    """The musl target the integration lane smoke-builds.

    Read from `integration.pipeline.kts`, which runs the build. It used to be
    read from `ci.yml` by substring search: the check was "this file mentions
    the target", which is satisfied by a comment and would have survived the
    build being deleted.
    """
    source = read_required(root, "integration.pipeline.kts")
    match = re.search(r"--target\s+([a-z0-9_]+-unknown-linux-musl)", source)
    if not match:
        raise ValueError(
            "integration.pipeline.kts builds no musl target. The platform "
            "matrix lists linux-x86-64-musl as an experimental smoke build, so "
            "either the build moved to another lane or the entry is a claim "
            "nothing backs."
        )
    return match.group(1)


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
            # Both paths, because the declaration and the grammar mapping are
            # in different layers on purpose. Citing only the parser file
            # would point a reader at a file that no longer declares
            # `Language` at all.
            "Language enum in domain/value_objects/language.rs; tree-sitter "
            "grammar mapping in infrastructure/parser/tree_sitter_parser.rs",
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
    for entry in release_platforms(root):
        platforms.append(
            {
                "id": entry.platform,
                "target": entry.target,
                "name": entry.platform.replace("-", " ").title(),
                "support_level": "certified",
                "evidence": [
                    f"Native publishable lane in {release_lane.RELEASE_LANE}",
                    f"Certified by {release_lane.RELEASE_LANE} and published unchanged by release.pipeline.kts",
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
                "Smoke build in integration.pipeline.kts",
                f"No publishable release lane in {release_lane.RELEASE_LANE}",
            ],
        }
    )
    for entry in DEFERRED_PUBLIC_PLATFORMS:
        platforms.append(
            {
                "id": entry["id"],
                "target": entry["target"],
                "name": entry["name"],
                "support_level": entry["support_level"],
                "evidence": deferred_evidence(root, entry),
            }
        )

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
            actual_languages = languages_output.read_text(encoding="utf-8")
            actual_platforms = platforms_output.read_text(encoding="utf-8")
            # Content is compared strictly; the `source_commit` stamp is
            # provenance and is validated for shape only. Requiring it to
            # equal HEAD made `--check` fail on every commit newer than the
            # artefacts. See check_semantics for the reasoning.
            for label, actual, rendered in (
                ("languages", actual_languages, rendered_languages),
                ("platforms", actual_platforms, rendered_platforms),
            ):
                if actual != rendered:
                    mismatch = content_mismatch(actual, rendered)
                    if mismatch:
                        raise ValueError(
                            f"generated {label} differ from the committed "
                            f"document: {mismatch}"
                        )
                check_provenance(actual, label)
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
