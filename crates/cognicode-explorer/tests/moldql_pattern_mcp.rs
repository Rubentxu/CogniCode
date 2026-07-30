//! Tests for MCP Pattern Profile tools: `moldql_pattern_query` and `moldql_pattern_capabilities`.
//!
//! T7: MCP tools for Pattern Profile.

use cognicode_explorer::error::ExplorerError;
use cognicode_explorer::mcp::{TOOL_PATTERN_CAPABILITIES, TOOL_PATTERN_QUERY};

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

/// Test that `ExplorerError::FeatureDisabled` produces a typed error string
/// that the MCP layer can surface to callers.
#[test]
fn mcp_pattern_feature_disabled_is_typed_error() {
    let err =
        ExplorerError::FeatureDisabled("Pattern Profile executor not wired in this view".into());
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("feature") && msg.to_lowercase().contains("disabled"),
        "FeatureDisabled error should contain 'feature disabled', got: {}",
        msg
    );
}
