//! Tests for MCP Pattern Profile tools: `moldql_pattern_query` and `moldql_pattern_capabilities`.
//!
//! T7: MCP tools for Pattern Profile.

use cognicode_explorer::mcp::{
    TOOL_PATTERN_CAPABILITIES, TOOL_PATTERN_QUERY,
};

/// Test that `moldql_pattern_query` tool name is correctly defined.
#[test]
fn moldql_pattern_query_tool_name() {
    assert_eq!(TOOL_PATTERN_QUERY, "moldql_pattern_query");
}

/// Test that `moldql_pattern_capabilities` tool name is correctly defined.
#[test]
fn moldql_pattern_capabilities_tool_name() {
    assert_eq!(TOOL_PATTERN_CAPABILITIES, "moldql_pattern_capabilities");
}
