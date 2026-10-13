#!/usr/bin/env python3
"""Focused black-box tests for the generated MCP tool catalog."""

from __future__ import annotations

import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATOR = ROOT / "scripts/product/generate_tool_catalog.py"
SCHEMA = ROOT / "product/schemas/tools.v1.schema.json"
BASELINE = "ebdb62febc0b1a4853cdc6e172d0b6ae1fb49018"
DISPATCH_ALLOWLIST_SOURCE = (
    ROOT / "crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs"
)
# Cargo features enabled in the default `cognicode-core` build, mirroring
# `interface::catalog::tool_feature_enabled`. A tool whose catalog entry
# declares a feature outside this set is filtered out of `tools/list` by
# `load_tool_catalog_active`, so it is correctly absent from the dispatch
# allowlist and its absence is not a ghost tool.
ENABLED_FEATURES = frozenset({"persistence"})


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(GENERATOR), *args],
        text=True,
        capture_output=True,
        check=False,
    )


def fixture() -> list[dict]:
    return [
        {
            "name": "zeta_tool",
            "description": "Zeta operation. More details.",
            "inputSchema": {"type": "object", "properties": {"value": {"type": "string"}}},
            "_meta": {
                "cognicode": {
                    "stability": "stable",
                    "category": "graph",
                    "authority": "read",
                    "requires_graph": True,
                    "requires_persistence": False,
                    "estimated_latency_ms": 120,
                }
            },
        },
        {
            "name": "alpha_tool",
            "description": "Alpha operation.",
            "inputSchema": {"type": "object"},
            "_meta": {
                "cognicode": {
                    "stability": "experimental",
                    "category": "aix",
                    "authority": "read",
                    "requires_graph": False,
                    "requires_persistence": False,
                    "estimated_latency_ms": 500,
                }
            },
        },
    ]


def test_generation_is_sorted_and_preserves_runtime_metadata() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "tools-list.json"
        output = Path(directory) / "tools.json"
        source.write_text(json.dumps(fixture()), encoding="utf-8")
        result = run(
            "--input",
            str(source),
            "--output",
            str(output),
            "--source-commit",
            BASELINE,
        )
        assert result.returncode == 0, result.stderr
        catalog = json.loads(output.read_text(encoding="utf-8"))
        assert catalog["schema_version"] == "cognicode.tools/v1"
        assert catalog["source_commit"] == BASELINE
        assert [tool["name"] for tool in catalog["tools"]] == ["alpha_tool", "zeta_tool"]
        alpha = catalog["tools"][0]
        assert alpha["stability"] == "experimental"
        assert alpha["public_profiles"] == ["experimental"]
        assert alpha["output_schema"] is None
        assert alpha["requirements"]["cache"] is None
        assert alpha["requirements_status"] == "not_declared"
        zeta = catalog["tools"][1]
        assert zeta["input_schema"] == fixture()[0]["inputSchema"]
        assert zeta["public_profiles"] == ["reviewer"]


def test_duplicate_names_are_rejected_without_output() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "tools-list.json"
        output = Path(directory) / "tools.json"
        duplicate = fixture() + [fixture()[0]]
        source.write_text(json.dumps(duplicate), encoding="utf-8")
        result = run(
            "--input",
            str(source),
            "--output",
            str(output),
            "--source-commit",
            BASELINE,
        )
        assert result.returncode != 0
        assert not output.exists()


def test_invalid_source_commit_is_rejected_without_output() -> None:
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "tools-list.json"
        output = Path(directory) / "tools.json"
        source.write_text(json.dumps(fixture()), encoding="utf-8")
        result = run(
            "--input",
            str(source),
            "--output",
            str(output),
            "--source-commit",
            "not-a-sha",
        )
        assert result.returncode != 0
        assert not output.exists()


def test_committed_catalog_is_self_consistent_and_schema_valid() -> None:
    result = run(
        "--input",
        str(ROOT / "product/tool-catalog-runtime.json"),
        "--output",
        str(ROOT / "product/tools.json"),
        "--source-commit",
        BASELINE,
        "--check",
    )
    assert result.returncode == 0, result.stderr
    catalog = json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))
    # The count is derived, not written as a literal. A literal here is a second
    # copy of a fact the capture already states, and it goes stale silently: the
    # committed capture holds 73 tools while the catalog has been extended since,
    # so a hardcoded number cannot tell a reader which of the two is wrong.
    capture = json.loads((ROOT / "product/tool-catalog-runtime.json").read_text(encoding="utf-8"))
    assert len(catalog["tools"]) > len(capture), (
        "the catalog must be a superset of the capture it is generated from; "
        "a smaller catalog means the generator and the hand-edits disagree"
    )
    assert catalog["runtime_tool_count"] == len(catalog["tools"]), (
        "runtime_tool_count disagrees with the number of tools actually listed. "
        "This count is what the skills and the README are checked against, so a "
        "mismatch here publishes two different tool counts at once."
    )
    # `jsonschema` is a hard requirement, not an optional one. This block used to
    # be wrapped in `except ImportError: return`, so on any machine without the
    # library the schema was never applied and the test still passed. That is how
    # `product/schemas/tools.v1.schema.json` fell so far behind the catalog that
    # 79 of its entries carried keys the schema forbids under
    # `additionalProperties: false`, and nothing noticed. A validation gate that
    # can silently not run is not a gate.
    import jsonschema

    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    jsonschema.validate(catalog, schema)


def test_every_catalog_field_is_declared_by_the_schema() -> None:
    """The catalog and the schema must describe the same surface.

    The schema and the Rust `ToolDescriptor` are two declarations of one
    contract. B3.W1/W2/W3 and B4.W1/W3 added fields the runtime reads and its
    tests pin, while the schema kept forbidding them. This is the check that does
    not need the `jsonschema` library: any key in the catalog that the schema
    does not declare is either dead weight or a schema that is behind the
    runtime, and both need a decision rather than a silent pass.
    """
    catalog = json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    allowed = set(schema["$defs"]["tool"]["properties"])
    required = set(schema["$defs"]["tool"]["required"])

    for tool in catalog["tools"]:
        extra = set(tool) - allowed
        assert not extra, (
            f"tool {tool['name']!r} carries fields the schema does not declare: "
            f"{sorted(extra)}. Either the field is dead weight or the schema is "
            f"behind the runtime `ToolDescriptor`, which is the authority on which "
            f"of the two is right."
        )
        missing = required - set(tool)
        assert not missing, (
            f"tool {tool['name']!r} is missing required fields: {sorted(missing)}"
        )


def test_no_catalog_tool_is_undispatchable() -> None:
    """A tool the server advertises must be one it can actually run.

    `build_all_tools` reads `product/tools.json`, filters by each entry's
    declared cargo `feature`, and publishes the rest through `tools/list`.
    `cogh` was published there from B6.W3 while being a CLI binary with no
    dispatch arm, and calling it returned `tool not found: cogh`. Advertising a
    capability the server does not have is worse than omitting it: an agent plans
    around a tool that cannot run.

    This mirrors `catalog::tool_feature_enabled` rather than re-deriving it. An
    earlier version of this test asserted that every catalog name appears in
    `dispatchable_tool_names()`, which wrongly flagged `list_evidence` and
    `search_evidence`: both are gated behind `evidence-cli-ladybug`, so they are
    correctly absent from `tools/list` in this build, and gating is not a defect.
    The gate list and the parity test both key off the enabled features.
    """
    catalog = json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))
    enabled_features = ENABLED_FEATURES
    for tool in catalog["tools"]:
        name = tool["name"]
        feature = tool.get("feature")
        if feature is not None and feature not in enabled_features:
            # Feature-gated out of this build: correctly not advertised.
            continue
        assert f'"{name}"' in DISPATCH_ALLOWLIST_SOURCE.read_text(encoding="utf-8"), (
            f"tool {name!r} is published to tools/list in this build but has no "
            f"dispatch arm. It is a ghost tool: either give it a handler or take "
            f"it out of the catalog."
        )


if __name__ == "__main__":
    tests = [
        test_generation_is_sorted_and_preserves_runtime_metadata,
        test_duplicate_names_are_rejected_without_output,
        test_invalid_source_commit_is_rejected_without_output,
        test_committed_catalog_is_self_consistent_and_schema_valid,
        test_every_catalog_field_is_declared_by_the_schema,
        test_no_catalog_tool_is_undispatchable,
    ]
    for test in tests:
        test()
        print(f"PASS {test.__name__}")
