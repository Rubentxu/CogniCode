//! Status classification for MCP tool call results.
//!
//! Implements ADR-034 decision 3: status classification taxonomy.
//! Status values: ok, stub, gated, error, missing, skip

use crate::interface::mcp::error::{InterfaceError, InterfaceResult};

/// Tools known to return STUB responses per ADR-034.
/// These tools are not yet fully implemented and return placeholder data.
/// M2 Sprint: all 6 tools now fully implemented (M2.9 complete).
const STUB_TOOLS: &[&str] = &[];

/// Tools that return gated errors when capability is not configured.
/// These tools detect missing PostgreSQL adapter or persistence.
const GATED_TOOLS: &[&str] = &[
    "graph_diff",
    "graph_timeline",
    "generate_contract",
    "compare_graph",
];

/// Gated error message patterns indicating capability not configured.
const GATED_PATTERNS: &[&str] = &[
    "not configured",
    "requires persistence",
    "PostgreSQL adapter",
];

/// Classifies a tool call result into a status label.
///
/// Returns one of: "ok", "stub", "gated", "error", "missing", "skip"
///
/// # Arguments
/// * `tool_name` - Name of the tool being classified
/// * `result` - The result of the tool call
pub fn classify_status(tool_name: &str, result: &InterfaceResult<String>) -> &'static str {
    match result {
        Ok(body) => {
            // Check if body indicates a STUB response
            if is_stub_response(tool_name, body) {
                return "stub";
            }
            "ok"
        }
        Err(e) => {
            // missing takes priority - ToolNotFound is a distinct case
            if matches!(e, InterfaceError::ToolNotFound(_)) {
                return "missing";
            }

            // gated: Internal error with configuration message for known gated tools
            if let InterfaceError::Internal(msg) = e {
                if is_gated_error(tool_name, msg) {
                    return "gated";
                }
            }

            // All other errors
            "error"
        }
    }
}

/// Checks if the tool is known to be a STUB implementation.
fn is_known_stub_tool(tool_name: &str) -> bool {
    STUB_TOOLS.contains(&tool_name)
}

/// Checks if the response body indicates a STUB pattern.
fn has_stub_body_pattern(body: &str) -> bool {
    // Empty results list
    if body.contains("\"results\":[]") || body.contains("\"results\": []") {
        return true;
    }
    // Zero candidates
    if body.contains("\"total_candidates\":0") || body.contains("\"total_candidates\": 0") {
        return true;
    }
    // Note prefix (placeholder responses)
    if body.contains("\"note\":") {
        return true;
    }
    // STUB marker in content
    if body.contains("\"stub\":true") || body.contains("\"stub\": true") {
        return true;
    }
    false
}

/// Determines if a result is a STUB response.
fn is_stub_response(tool_name: &str, body: &str) -> bool {
    is_known_stub_tool(tool_name) && has_stub_body_pattern(body)
}

/// Checks if an error message indicates a gated (not configured) capability.
/// Gated applies only to known GATED_TOOLS when message contains configuration patterns.
fn is_gated_error(tool_name: &str, message: &str) -> bool {
    GATED_TOOLS.contains(&tool_name)
        && GATED_PATTERNS
            .iter()
            .any(|pattern| message.to_lowercase().contains(&pattern.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_status() {
        let result: InterfaceResult<String> = Ok(r#"{"results": [{"id": 1}]}"#.to_string());
        assert_eq!(classify_status("get_file_symbols", &result), "ok");
    }

    #[test]
    fn test_missing_status() {
        let result: InterfaceResult<String> =
            Err(InterfaceError::ToolNotFound("unknown_tool".to_string()));
        assert_eq!(classify_status("unknown_tool", &result), "missing");
    }

    #[test]
    fn test_gated_status() {
        let result: InterfaceResult<String> = Err(InterfaceError::Internal(
            "PostgreSQL adapter not configured for graph operations".to_string(),
        ));
        assert_eq!(classify_status("graph_diff", &result), "gated");
    }

    #[test]
    fn test_gated_status_requires_persistence() {
        let result: InterfaceResult<String> = Err(InterfaceError::Internal(
            "This operation requires persistence".to_string(),
        ));
        assert_eq!(classify_status("graph_timeline", &result), "gated");
    }

    #[test]
    fn test_error_status() {
        let result: InterfaceResult<String> =
            Err(InterfaceError::InvalidInput("bad input".to_string()));
        assert_eq!(classify_status("get_file_symbols", &result), "error");
    }

    #[test]
    fn test_error_status_domain() {
        let result: InterfaceResult<String> = Err(InterfaceError::Domain(
            crate::domain::error::DomainError::SymbolNotFound("foo".to_string()),
        ));
        assert_eq!(classify_status("get_file_symbols", &result), "error");
    }

    #[test]
    fn test_ok_non_stub_tool() {
        // Non-STUB tool with empty results is still "ok" (valid empty response)
        let result: InterfaceResult<String> = Ok(r#"{"results":[]}"#.to_string());
        assert_eq!(classify_status("get_file_symbols", &result), "ok");
    }
}
