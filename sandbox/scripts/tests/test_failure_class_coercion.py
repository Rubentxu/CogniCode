"""Regression tests for release_scorecard._normalize_failure_class.

Tracks: openspec/changes/scorecard-failure-class-coercion

These tests exist because the scorecard silently aborted (TypeError) when
result.json files in sandbox/results/full_run/* carried dict-shaped
failure_class values. The helper was added to coerce them to flat strings.

Run: python3 -m pytest sandbox/scripts/tests/test_failure_class_coercion.py -v
"""

import sys
from pathlib import Path

# Make the sibling release_scorecard importable.
_SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_SCRIPTS_DIR))

from release_scorecard import _normalize_failure_class  # noqa: E402


def test_string_passthrough():
    assert _normalize_failure_class("pass") == "pass"
    assert _normalize_failure_class("expected_fail") == "expected_fail"
    assert _normalize_failure_class("") == ""


def test_none_default():
    assert _normalize_failure_class(None) == "pass"


def test_mcp_tool_error_with_tool():
    fc = {"mcp_tool_error": {"tool_name": "trace_path", "error_message": "mcp_error"}}
    assert _normalize_failure_class(fc) == "mcp_tool_error:trace_path"


def test_mcp_tool_error_various_tools():
    for tool in ("find_references", "get_call_hierarchy", "graph_all_paths", "detect_drift"):
        fc = {"mcp_tool_error": {"tool_name": tool, "error_message": "x"}}
        assert _normalize_failure_class(fc) == f"mcp_tool_error:{tool}", tool


def test_mcp_tool_error_empty_inner():
    fc = {"mcp_tool_error": {}}
    assert _normalize_failure_class(fc) == "mcp_tool_error"


def test_mcp_tool_error_inner_non_dict():
    fc = {"mcp_tool_error": "some message"}
    assert _normalize_failure_class(fc) == "mcp_tool_error"


def test_generic_dict_uses_outer_key():
    assert _normalize_failure_class({"some_class": "value"}) == "some_class"


def test_empty_dict_default():
    assert _normalize_failure_class({}) == "pass"


def test_non_string_non_dict_fallback():
    # The historical contract expected strings, but be defensive: any object
    # convertible via str() is acceptable.
    assert _normalize_failure_class(123) == "123"
    assert _normalize_failure_class(0) == "0"


def test_aggregate_does_not_crash_on_dict_failure_class():
    """Simulate the original failure mode by feeding dict-shaped failure_class
    through a tiny synthetic aggregate flow that mirrors the scorecard read site.
    """
    sample_results = [
        {"tool": "trace_path", "failure_class": {"mcp_tool_error": {"tool_name": "trace_path", "error_message": "x"}}},
        {"tool": "find_references", "failure_class": "pass"},
        {"tool": "get_call_hierarchy", "failure_class": None},
        {"tool": "graph_all_paths", "failure_class": {"mcp_tool_error": {"tool_name": "graph_all_paths", "error_message": "y"}}},
    ]
    dist: dict = {}
    for r in sample_results:
        fc = _normalize_failure_class(r.get("failure_class", "pass"))
        dist[fc] = dist.get(fc, 0) + 1

    # Must be hashable strings, no TypeError.
    assert dist == {
        "mcp_tool_error:trace_path": 1,
        "pass": 2,
        "mcp_tool_error:graph_all_paths": 1,
    }


if __name__ == "__main__":
    import pytest
    sys.exit(pytest.main([__file__, "-v"]))