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
    return {
        "name": tool["name"],
        "title": title_from_description(tool["description"]),
        "description": tool["description"],
        "category": metadata["category"],
        "authority": metadata.get("authority"),
        "stability": metadata["stability"],
        "input_schema": tool["inputSchema"],
        # A-004 must not invent output contracts before structured-output work.
        "output_schema": metadata.get("output_schema"),
        "requirements": requirements,
        "requirements_status": status,
        "estimated_latency_ms": metadata["estimated_latency_ms"],
        "public_profiles": public_profiles(metadata),
        "runtime_metadata": metadata,
    }


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


def render(catalog: dict[str, Any]) -> str:
    return json.dumps(catalog, indent=2, sort_keys=False, ensure_ascii=False) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    try:
        catalog = build_catalog(load_tools(args.input), args.source_commit)
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
