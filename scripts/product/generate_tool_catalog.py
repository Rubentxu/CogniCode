#!/usr/bin/env python3
"""Generate CogniCode's public MCP tool catalog from a real tools/list capture."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "cognicode.tools/v1"
REQUIRED_METADATA = {
    "stability",
    "category",
    "requires_graph",
    "requires_persistence",
    "estimated_latency_ms",
}
STABILITIES = {"stable", "experimental", "gated"}
AUTHORITIES = {"read", "mutating", "execute", "network"}
CORE_CATEGORIES = {"file", "navigation", "search", "view"}
# `_meta.cognicode` keys that are also first-class `ToolDescriptor` fields.
# They are carried in the metadata bag for the wire, and must additionally be
# promoted to the descriptor's top level or serde substitutes the conservative
# default for the real value.
PROMOTED_METADATA_FIELDS = (
    "precision_floor",
    "impact_direction_floor",
    "intent_tags",
)

# Affordances the runtime cannot report, so they are authored by hand and merged
# from `product/tool-affordances.json`. See `load_overlay`.
OVERLAY_SCHEMA_VERSION = "cognicode.tool-overlay/v1"
OVERLAY_FIELDS = (
    "limitations",
    "required_args",
    "prerequisites",
    "follow_ups",
    # Which languages a quality tool actually produces findings for. Authored
    # rather than observed because the answer depends on the tree-sitter
    # grammars a build happens to link, which is not something a `tools/list`
    # capture can answer.
    "language_scope",
)
# Default location of the overlay, relative to the repository root. The
# `--overlay` flag exists so tests can point at a fixture.
DEFAULT_OVERLAY = Path("product/tool-affordances.json")


def title_from_description(description: str) -> str:
    """Return a short display title without inventing runtime semantics."""
    return description.split(".", 1)[0].strip() or "Unnamed tool"


def public_profiles(metadata: dict[str, Any]) -> list[str]:
    """Map declared runtime metadata to the documented public profiles."""
    stability = metadata["stability"]
    authority = metadata.get("authority")
    if stability in {"experimental", "gated"}:
        return ["experimental"]
    if authority == "mutating" or metadata.get("mutates_workspace") is True:
        return ["developer"]
    if metadata["category"] in CORE_CATEGORIES:
        return ["core", "reviewer"]
    return ["reviewer"]


def load_tools(path: Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    tools = payload.get("tools") if isinstance(payload, dict) else payload
    if not isinstance(tools, list) or not tools:
        raise ValueError("input must be a non-empty tools/list array or {tools: [...]}")
    return tools


def normalize_tool(tool: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(tool, dict):
        raise ValueError("every tools/list entry must be an object")
    for field in ("name", "description", "inputSchema", "_meta"):
        if field not in tool:
            raise ValueError(f"tool is missing required runtime field: {field}")
    if not isinstance(tool["name"], str) or not tool["name"]:
        raise ValueError("tool name must be a non-empty string")
    if not isinstance(tool["description"], str):
        raise ValueError(f"tool {tool['name']!r} description must be a string")
    if not isinstance(tool["inputSchema"], dict):
        raise ValueError(f"tool {tool['name']!r} inputSchema must be an object")
    metadata = tool["_meta"].get("cognicode") if isinstance(tool["_meta"], dict) else None
    if not isinstance(metadata, dict):
        raise ValueError(f"tool {tool['name']!r} is missing _meta.cognicode")
    missing = REQUIRED_METADATA - metadata.keys()
    if missing:
        raise ValueError(f"tool {tool['name']!r} metadata missing: {sorted(missing)}")
    if metadata["stability"] not in STABILITIES:
        raise ValueError(f"tool {tool['name']!r} has unsupported stability")
    if metadata.get("authority") is not None and metadata["authority"] not in AUTHORITIES:
        raise ValueError(f"tool {tool['name']!r} has unsupported authority")
    if not isinstance(metadata["estimated_latency_ms"], int):
        raise ValueError(f"tool {tool['name']!r} estimated latency must be an integer")

    requirements = {
        "graph": metadata["requires_graph"],
        "persistence": metadata["requires_persistence"],
        # The current runtime does not declare these fields. Null is deliberate.
        "cache": metadata.get("requires_cache"),
        "network": metadata.get("requires_network"),
    }
    status = "declared" if all(value is not None for value in requirements.values()) else "not_declared"
    descriptor = {
        "name": tool["name"],
        "title": title_from_description(tool["description"]),
        "description": tool["description"],
        "category": metadata["category"],
        "authority": metadata.get("authority"),
        "stability": metadata["stability"],
        "input_schema": tool["inputSchema"],
        # A-004 kept this null on purpose: inventing output contracts before
        # structured-output work would have published shapes the runtime never
        # promised. A-012 is that work — the runtime now derives the schema
        # from the very type that serialises the result, so `outputSchema` on
        # the wire is the only admissible source. `_meta.output_schema` is
        # still honoured when present, but the runtime field wins when both
        # exist: a hand-written copy must never override the derive.
        "output_schema": tool.get("outputSchema") or metadata.get("output_schema"),
        "requirements": requirements,
        "requirements_status": status,
        "estimated_latency_ms": metadata["estimated_latency_ms"],
        "public_profiles": public_profiles(metadata),
        "runtime_metadata": metadata,
    }
    # Contract fields the runtime reports in `_meta.cognicode` but that
    # `ToolDescriptor` reads from the top level of the descriptor.
    #
    # Leaving them only inside `runtime_metadata` did not drop them so much as
    # make them invisible: serde fell back to `default_precision_floor`, and
    # `catalog_declares_precision_floor_for_every_navigation_tool` compared the
    # conservative `ast-heuristic` default against the authoritative `lsp`.
    # Promoting them here is what makes the catalog round-trip through the
    # loader instead of silently degrading on every regeneration.
    for field in PROMOTED_METADATA_FIELDS:
        if field in metadata:
            descriptor[field] = metadata[field]
    return descriptor


def build_catalog(tools: list[dict[str, Any]], source_commit: str) -> dict[str, Any]:
    if not re.fullmatch(r"[0-9a-f]{40}", source_commit):
        raise ValueError("source_commit must be a lowercase 40-character Git SHA")
    names: set[str] = set()
    normalized = []
    for tool in tools:
        entry = normalize_tool(tool)
        if entry["name"] in names:
            raise ValueError(f"duplicate tool name: {entry['name']}")
        names.add(entry["name"])
        normalized.append(entry)
    normalized.sort(key=lambda entry: entry["name"])
    return {
        "schema_version": SCHEMA_VERSION,
        "product": "cognicode",
        "source_commit": source_commit,
        "runtime_tool_count": len(normalized),
        "tools": normalized,
    }


def load_overlay(path: Path) -> dict[str, Any]:
    """Load the hand-authored affordance overlay.

    Some catalog fields cannot be observed from a `tools/list` capture at all:
    what a tool cannot do (`limitations`), what it needs first
    (`prerequisites`), what usually comes next (`follow_ups`), and a declarative
    mirror of its required arguments. They are editorial, per-tool knowledge, so
    they live in `product/tool-affordances.json` and are merged in here.

    Without this split they lived only in the generated file, which meant every
    regeneration silently erased them. That is not hypothetical: regenerating
    from a complete capture dropped `limitations` from 73 tools,
    `required_args` from 50, `prerequisites` from 41, and `follow_ups` from 26,
    and four `b3_w3_agent_discovery` tests failed as a result. A generated file
    is the wrong home for content no generator can produce.
    """
    if not path.exists():
        return {"tools": {}, "runtime_metadata_flags": {}}
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("schema_version") != OVERLAY_SCHEMA_VERSION:
        raise ValueError(
            f"{path} has schema_version {payload.get('schema_version')!r}, "
            f"expected {OVERLAY_SCHEMA_VERSION!r}"
        )
    return {
        "tools": payload.get("tools", {}),
        "runtime_metadata_flags": payload.get("runtime_metadata_flags", {}),
    }


def merge_overlay(catalog: dict[str, Any], overlay: dict[str, Any]) -> dict[str, Any]:
    """Fold overlay affordances into the generated catalog.

    Raises on an overlay entry for a tool the runtime does not serve. That is
    deliberate: a stale overlay entry is a claim about a capability that no
    longer exists, and silently dropping it would let the overlay rot into a
    second, unreadable catalog.
    """
    by_name = {entry["name"]: entry for entry in catalog["tools"]}
    affordances = overlay.get("tools", {})
    metadata_flags = overlay.get("runtime_metadata_flags", {})
    unknown = sorted((set(affordances) | set(metadata_flags)) - set(by_name))
    if unknown:
        raise ValueError(
            f"tool-affordances.json names tools the runtime does not serve: {unknown}"
        )
    for name, fields in affordances.items():
        entry = by_name[name]
        for field, value in fields.items():
            if field not in OVERLAY_FIELDS:
                raise ValueError(
                    f"tool-affordances.json sets unknown field {field!r} on {name!r}"
                )
            entry[field] = value
    # Authored flags that stay inside `runtime_metadata`, which
    # `ToolDescriptor` reads as an untyped bag. `gate_authority: false` for
    # `check_architecture` is the case that matters: it is what lets an MCP
    # client verify the read-only diagnostic contract without reading the
    # prompt, and `runtime_metadata` is the only place that claim is read from.
    for name, flags in metadata_flags.items():
        entry = by_name[name]
        runtime_metadata = entry.get("runtime_metadata")
        if not isinstance(runtime_metadata, dict):
            raise ValueError(
                f"cannot set runtime_metadata flags on {name!r}: its "
                f"runtime_metadata is not an object"
            )
        for field, value in flags.items():
            if field in runtime_metadata:
                raise ValueError(
                    f"tool-affordances.json overrides runtime field {field!r} on "
                    f"{name!r}; the runtime is authoritative for that field"
                )
            runtime_metadata[field] = value
    return catalog


def render(catalog: dict[str, Any]) -> str:
    return json.dumps(catalog, indent=2, sort_keys=False, ensure_ascii=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument(
        "--overlay",
        type=Path,
        default=None,
        help=(
            "hand-authored affordances to merge "
            "(default: product/tool-affordances.json when present)"
        ),
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    overlay_path = args.overlay
    if overlay_path is None:
        default_overlay = DEFAULT_OVERLAY
        if default_overlay.exists():
            overlay_path = default_overlay
    try:
        catalog = build_catalog(load_tools(args.input), args.source_commit)
        if overlay_path is not None:
            merge_overlay(catalog, load_overlay(overlay_path))
        rendered = render(catalog)
        if args.check:
            actual = args.output.read_text(encoding="utf-8")
            if actual != rendered:
                raise ValueError(f"generated catalog differs from {args.output}")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered, encoding="utf-8")
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"tool catalog generation failed: {exc}", file=sys.stderr)
        return 1
    action = "validated" if args.check else "generated"
    print(f"tool catalog {action}: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
