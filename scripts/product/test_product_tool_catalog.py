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


def source_commit() -> str:
    """The commit the committed catalog was generated from.

    Derived from the catalog itself rather than pinned to a literal. The
    generator stamps this field, so checking a regeneration against a pinned
    SHA only passes until the next commit -- which is exactly how a stale
    artifact stays on disk looking current.
    """
    committed = json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))
    return committed["source_commit"]


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(GENERATOR), *args],
        text=True,
        capture_output=True,
        check=False,
    )


def run_without_overlay(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the generator with an explicitly empty affordance overlay.

    Fixture-driven tests build a catalog from a synthetic one- or two-tool
    capture. Without this, the generator would also fold in the repository's
    real `product/tool-affordances.json`, which names tools that fixture does not
    contain -- and `merge_overlay` rejects exactly that, by design, so a stale
    overlay entry cannot hide behind a passing test.
    """
    with tempfile.TemporaryDirectory() as directory:
        empty = Path(directory) / "overlay.json"
        empty.write_text(
            json.dumps(
                {
                    "schema_version": "cognicode.tool-overlay/v1",
                    "tools": {},
                    "runtime_metadata_flags": {},
                }
            ),
            encoding="utf-8",
        )
        return run(*args, "--overlay", str(empty))


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
        result = run_without_overlay(
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
        result = run_without_overlay(
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
        result = run_without_overlay(
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
        source_commit(),
        "--check",
    )
    assert result.returncode == 0, result.stderr
    catalog = json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))
    # The count is derived, not written as a literal. A literal here is a second
    # copy of a fact the capture already states, and it goes stale silently.
    #
    # The capture and the catalog hold the same tool names by construction: the
    # generator copies every descriptor it is given. They used to differ (73 vs
    # 80) because B6.W3 hand-edited the generated catalog. An assertion that the
    # catalog is a strict *superset* of the capture encoded that hand-editing as
    # the contract, and would have rejected a correct regeneration, so equality
    # of the name sets is asserted instead.
    capture = json.loads((ROOT / "product/tool-catalog-runtime.json").read_text(encoding="utf-8"))
    assert {t["name"] for t in catalog["tools"]} == {t["name"] for t in capture}, (
        "the catalog must list exactly the tools the committed capture observed. "
        "A mismatch means generated output was hand-edited: regenerate with "
        "scripts/product/generate_tool_catalog.py from a real tools/list capture "
        "instead of typing descriptors in."
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


def test_affordance_overlay_survives_regeneration() -> None:
    """Hand-authored affordances must survive a regeneration.

    These four fields -- what a tool cannot do, what it needs first, what
    usually comes next, and a declarative mirror of its required arguments --
    cannot be observed from a `tools/list` capture, so they live in
    `product/tool-affordances.json`. Before that split they existed only in the
    generated catalog, and regenerating silently erased them: `limitations`
    vanished from 73 tools, `required_args` from 50, `prerequisites` from 41,
    `follow_ups` from 26, and four `b3_w3_agent_discovery` tests failed.

    The assertion is that the committed catalog carries what the overlay
    declares. A generator that stopped merging would still pass the pure
    self-consistency check -- it would just be consistently empty -- so this
    compares the two artefacts against each other instead.
    """
    overlay = json.loads((ROOT / "product/tool-affordances.json").read_text(encoding="utf-8"))
    assert overlay["schema_version"] == "cognicode.tool-overlay/v1"
    assert overlay["tools"], "the overlay must not be empty; an empty one hides the defect"
    catalog = {
        t["name"]: t
        for t in json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))["tools"]
    }
    for name, affordances in overlay["tools"].items():
        assert name in catalog, f"overlay names unknown tool {name!r}"
        for field, value in affordances.items():
            assert catalog[name].get(field) == value, (
                f"{name}.{field} was not carried into the catalog. Either the "
                f"generator stopped merging the overlay, or the committed "
                f"catalog is stale with respect to product/tool-affordances.json."
            )


def test_overlay_may_not_claim_a_tool_the_runtime_does_not_serve() -> None:
    """A stale overlay entry is a claim about a capability that is gone.

    Silently dropping such an entry would let the overlay rot into a second,
    unreadable catalog that nobody notices is describing a previous product.
    `merge_overlay` raises on this in the generator; this pins it on the
    committed artefacts so the rot is visible without running the generator.
    """
    catalog = {
        t["name"]
        for t in json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))["tools"]
    }
    overlay = json.loads((ROOT / "product/tool-affordances.json").read_text(encoding="utf-8"))
    unknown = sorted((set(overlay["tools"]) | set(overlay["runtime_metadata_flags"])) - catalog)
    assert not unknown, (
        f"tool-affordances.json describes tools the catalog does not publish: {unknown}"
    )


def test_runtime_metadata_flags_survive_regeneration() -> None:
    """Authored `runtime_metadata` flags must survive a regeneration too.

    `check_architecture` declares `gate_authority: false` and
    `produces_observations_only: true` so an MCP client can verify the
    read-only diagnostic contract without reading the prompt. Those claims live
    only inside `runtime_metadata`, which the runtime does not emit, so a
    regeneration emptied them and `catalog_declares_check_architecture_gate_
    authority_false` failed with "gate_authority must be a bool".
    """
    overlay = json.loads((ROOT / "product/tool-affordances.json").read_text(encoding="utf-8"))
    catalog = {
        t["name"]: t
        for t in json.loads((ROOT / "product/tools.json").read_text(encoding="utf-8"))["tools"]
    }
    for name, flags in overlay["runtime_metadata_flags"].items():
        runtime_metadata = catalog[name]["runtime_metadata"]
        for field, value in flags.items():
            assert runtime_metadata.get(field) == value, (
                f"{name}.runtime_metadata.{field} was not carried into the "
                f"catalog; the committed catalog is stale with respect to "
                f"product/tool-affordances.json"
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
