//! Status classification for MCP tool call results.
//!
//! Implements ADR-034 decision 3: status classification taxonomy.
//! Status values: ok, stub, gated, error, missing, skip

use cognicode_core::interface::mcp::error::{ErrorKind, InterfaceError, InterfaceResult};

/// Tools known to return STUB responses per ADR-034.
/// These tools are not yet fully implemented and return placeholder data.
/// M2 Sprint: all 6 tools now fully implemented (M2.9 complete).
const STUB_TOOLS: &[&str] = &[];

/// Classifies a tool call result into a status label.
///
/// Returns one of: "ok", "stub", "gated", "error", "missing", "skip"
///
/// A failure is sorted by its [`ErrorKind`] and by nothing else: the words of
/// its message never decide the label, so an error that happens to say
/// "not configured" is not a gated capability.
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
        Err(e) => match e.kind() {
            // A tool the registry does not know is its own case.
            ErrorKind::ToolNotFound => "missing",
            // The call was well-formed; the capability behind it is not
            // configured in this build or session.
            ErrorKind::NotConfigured => "gated",
            _ => "error",
        },
    }
}

/// The `error_type` label the error counter carries.
///
/// The same rule as [`classify_status`] — the kind decides, the message never
/// does — so the metric and the status label cannot disagree about what a
/// failure was.
pub fn error_type(error: &InterfaceError) -> &'static str {
    match error.kind() {
        ErrorKind::ToolNotFound => "missing",
        ErrorKind::Timeout => "timeout",
        ErrorKind::RateLimited => "rate_limit_exceeded",
        _ => "error",
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
        let result: InterfaceResult<String> = Err(InterfaceError::NotConfigured(
            "generate_contract requires a persistence layer".to_string(),
        ));
        assert_eq!(classify_status("generate_contract", &result), "gated");
    }

    #[test]
    fn an_internal_error_that_says_not_configured_is_still_an_error() {
        // The label comes from the type. A failure whose message happens to
        // use the words of a missing capability is a failure, not a gate.
        for message in [
            "persistence adapter not configured for graph operations",
            "This operation requires persistence",
            "generate_contract requires a persistence layer",
        ] {
            let result: InterfaceResult<String> =
                Err(InterfaceError::Internal(message.to_string()));
            assert_eq!(
                classify_status("generate_contract", &result),
                "error",
                "{message}"
            );
        }
    }

    #[test]
    fn a_capability_that_is_not_configured_is_gated_whatever_the_tool() {
        let result: InterfaceResult<String> =
            Err(InterfaceError::NotConfigured("no backend".to_string()));
        assert_eq!(classify_status("get_file_symbols", &result), "gated");
    }

    #[test]
    fn the_refusals_of_the_server_are_errors() {
        for refusal in [
            InterfaceError::Timeout,
            InterfaceError::RateLimited,
            InterfaceError::ReadOnly {
                tool: "write_file".to_string(),
            },
            InterfaceError::Ambiguous {
                name: "helper".to_string(),
                candidates: vec!["a.rs:helper:1".to_string()],
            },
        ] {
            let result: InterfaceResult<String> = Err(refusal);
            assert_eq!(classify_status("get_file_symbols", &result), "error");
        }
    }

    #[test]
    fn the_error_type_is_the_kind_and_never_the_message() {
        assert_eq!(error_type(&InterfaceError::Timeout), "timeout");
        assert_eq!(
            error_type(&InterfaceError::RateLimited),
            "rate_limit_exceeded"
        );
        assert_eq!(
            error_type(&InterfaceError::ToolNotFound("x".to_string())),
            "missing"
        );
        // A message that spells a sentinel is still an ordinary failure.
        assert_eq!(
            error_type(&InterfaceError::Internal("timeout".to_string())),
            "error"
        );
        assert_eq!(
            error_type(&InterfaceError::Internal("rate_limit_exceeded".to_string())),
            "error"
        );
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
            cognicode_core::domain::error::DomainError::SymbolNotFound("foo".to_string()),
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
