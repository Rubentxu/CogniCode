#!/usr/bin/env python3
"""Synchronize the master catalog's runtime metadata with the runtime's own report.

`product/tools.json` is the hand-authored master: every tool the product
publishes, including the fields no generator owns. Its `runtime_metadata` block
is the mirror of what the runtime publishes in `_meta.cognicode` — including
`tool_version`, which the runtime injects from `CARGO_PKG_VERSION` on every
response (`interface/mcp/rmcp_adapter.rs`).

A mirror that is written once and never revisited drifts. The master's
`tool_version` froze at the version of the runtime that was current when each
entry was authored: 73 of the 78 entries still read `0.99.2`, the version of
the workspace at `ebdb62fe`, while the runtime reports `0.101.31`. The
published document then says of its runtime a version that is not the runtime's,
and `cognicode capabilities --json` publishes that block verbatim.

This script owns exactly one rule:

    every key the runtime publishes in `_meta.cognicode` is present in the
    master's `runtime_metadata` with the runtime's own value

It is a merge, never a replacement. The master may carry curated keys the
runtime does not publish — `check_architecture` carries `gate_authority` and
`produces_observations_only` — and the merge keeps them, with their values.
Nothing else in the document is touched: the nine fields no generator owns
(`limitations`, `required_args`, `prerequisites`, `follow_ups`, `intent_tags`,
`language_scope`, `feature`, `impact_direction_floor`, `precision_floor`) and
the tool set itself stay exactly as authored. Regenerating the document is a
different operation and a different script (`generate_tool_catalog.py`), which
drops all nine.

Use it after a capture is refreshed, or with `--check` to prove that the
committed master is a fixed point of the merge: synchronizing it would change
nothing.

    python3 scripts/product/sync_tool_catalog_runtime_metadata.py
    python3 scripts/product/sync_tool_catalog_runtime_metadata.py --check
"""

from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path
from typing import Any

# The test suite loads this file by path, which does not put this directory on
# `sys.path`, so the sibling import has to be resolved explicitly to work in
# both contexts.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_tool_catalog import render  # noqa: E402

DEFAULT_MASTER = Path("product/tools.json")
DEFAULT_CAPTURE = Path("product/tool-catalog-runtime.json")


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def capture_runtime_metadata(capture: Any) -> dict[str, dict[str, Any]]:
    """The metadata the runtime publishes, by tool name.

    The capture is a real `tools/list`: each entry carries `_meta.cognicode`,
    which is exactly what the runtime puts on the wire.
    """
    if not isinstance(capture, list) or not capture:
        raise ValueError("the capture must be a non-empty tools/list array")
    published: dict[str, dict[str, Any]] = {}
    for entry in capture:
        if not isinstance(entry, dict):
            raise ValueError("every tools/list entry must be an object")
        name = entry.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("tool name must be a non-empty string")
        meta = entry.get("_meta")
        meta = meta.get("cognicode") if isinstance(meta, dict) else None
        if not isinstance(meta, dict):
            raise ValueError(f"tool {name!r} is missing _meta.cognicode")
        published[name] = meta
    return published


def merge_runtime_metadata(
    master: dict[str, Any], capture: Any
) -> tuple[dict[str, Any], list[str]]:
    """Merge the runtime's published metadata into the master's mirror.

    Pure: the inputs are not mutated, the merged document is a new object.
    Returns the merged document and the findings — the tools that could not be
    synchronized because only one of the two sides publishes them.
    """
    published = capture_runtime_metadata(capture)
    merged = copy.deepcopy(master)
    findings: list[str] = []
    matched: set[str] = set()
    for tool in merged.get("tools", []):
        name = tool.get("name")
        if not isinstance(name, str) or not name:
            raise ValueError("every master entry must carry a non-empty name")
        runtime = published.get(name)
        if runtime is None:
            findings.append(
                f"{name}: the master publishes it and the capture does not; its "
                "runtime metadata cannot be synchronized"
            )
            continue
        matched.add(name)
        mirror = tool.get("runtime_metadata")
        if not isinstance(mirror, dict):
            raise ValueError(f"{name}: runtime_metadata must be an object")
        # Keys the runtime publishes win; keys only the master carries (the
        # curated ones) stay, with their values.
        for key, value in runtime.items():
            mirror[key] = value
    for name in published:
        if name not in matched:
            findings.append(
                f"{name}: the runtime publishes it and the master does not"
            )
    return merged, findings


def drifted_tools(master: dict[str, Any], merged: dict[str, Any]) -> list[str]:
    """The tools whose runtime metadata the merge changes."""
    before = {t["name"]: t.get("runtime_metadata") for t in master.get("tools", [])}
    changed = []
    for tool in merged.get("tools", []):
        name = tool.get("name")
        if before.get(name) != tool.get("runtime_metadata"):
            changed.append(name)
    return changed


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--master", type=Path, default=DEFAULT_MASTER)
    parser.add_argument("--capture", type=Path, default=DEFAULT_CAPTURE)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify that the master is synchronized; do not write",
    )
    args = parser.parse_args()
    try:
        master = load_json(args.master)
        capture = load_json(args.capture)
        merged, findings = merge_runtime_metadata(master, capture)
        rendered = render(merged)
        if args.check:
            current = args.master.read_text(encoding="utf-8")
            if current != rendered:
                changed = drifted_tools(master, merged)
                raise ValueError(
                    "the master is not synchronized with the capture; these tools "
                    f"differ: {', '.join(changed)}"
                )
            if findings:
                raise ValueError("; ".join(findings))
            print(f"the master is synchronized with the capture: {args.master}")
            return 0
        args.master.write_text(rendered, encoding="utf-8")
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"tool catalog synchronization failed: {exc}", file=sys.stderr)
        return 1
    changed = drifted_tools(master, merged)
    print(f"the master is synchronized with the capture: {args.master}")
    if changed:
        print(f"synchronized {len(changed)} tools: {', '.join(changed)}")
    for finding in findings:
        print(f"  {finding}", file=sys.stderr)
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
