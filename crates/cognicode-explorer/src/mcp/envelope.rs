//! Shared result envelope builders for MCP tool handlers.
//!
//! All 9 handler files previously defined local copies of these three helpers.
//! This module centralises them so the wire-level JSON envelope format is
//! consistent and one bug (`sessions.rs:988` — `err_with_code` returning
//! `CallToolResult::success`) can be fixed in one place.
//!
//! # Envelope shape
//!
//! Every result — success or error — is serialized as a JSON object with these
//! top-level keys:
//!
//! | Key | Type | Description |
//! |-----|------|-------------|
//! | `tool_name` | string | Canonical tool name that produced this result |
//! | `version` | string | `CARGO_PKG_VERSION` at construction time |
//! | `timestamp` | string | RFC 3339 timestamp at construction time |
//! | `provenance` | `ProvenanceMetadata \| null` | Source subsystem + confidence; `null` for plain success |
//! | `payload` | any | Tool-specific result or `EnvelopeError` on failure |
//! | `suggested_follow_ups` | array | Always `[]` in v1 |

use rmcp::model::{CallToolResult, Content, RawContent};
use serde::Serialize;
use serde_json::Value;

use super::explorer::ProvenanceMetadata;

// ============================================================================
// Public API
// ============================================================================

/// Build a `CallToolResult::success` carrying the canonical result envelope.
///
/// The `provenance` field is `null` (no source metadata).
///
/// # Type parameter
///
/// `T` must implement `serde::Serialize`. This includes `serde_json::Value`
/// and all primitive types, so call sites can pass `&serde_json::Value` or
/// `&Vec<String>` without conversion.
pub fn ok_envelope<T: Serialize>(tool_name: &str, payload: &T) -> CallToolResult {
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": serde_json::Value::Null,
        "payload": payload,
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::success(vec![Content::text(pretty)])
}

/// Build a `CallToolResult::success` carrying the canonical result envelope
/// with provenance metadata.
///
/// Use this for results that originate from a known subsystem such as
/// `"ask-router"` or `"brain-session"`.
pub fn ok_envelope_with_provenance<T: Serialize>(
    tool_name: &str,
    payload: &T,
    provenance: ProvenanceMetadata,
) -> CallToolResult {
    let provenance_json = serde_json::to_value(provenance).unwrap_or(serde_json::Value::Null);
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": provenance_json,
        "payload": payload,
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::success(vec![Content::text(pretty)])
}

/// Build a `CallToolResult::error` carrying the canonical error envelope.
///
/// The `payload` field contains `error_code` and `error` keys so MCP clients
/// can programmatically distinguish error families.
pub fn err_envelope(tool_name: &str, code: &str, message: &str) -> CallToolResult {
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": serde_json::Value::Null,
        "payload": {
            "error_code": code,
            "error": message,
        },
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::error(vec![Content::text(pretty)])
}

// ============================================================================
// Regression tests — lock the wire-level JSON shape
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolResult;
    use serde_json::json;

    // ------------------------------------------------------------------------
    // ok_envelope
    // ------------------------------------------------------------------------

    #[test]
    fn ok_envelope_json_shape() {
        let payload = json!({"nodes": [], "edges": []});
        let result = ok_envelope("graph_subgraph", &payload);

        assert!(result.is_error == Some(false));
        let items = &result.content;
        assert!(!items.is_empty());
        let Content {
            raw: RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();

        assert_eq!(parsed["tool_name"], "graph_subgraph");
        assert!(parsed["provenance"].is_null());
        assert_eq!(parsed["payload"]["nodes"], json!([]));
        assert_eq!(parsed["payload"]["edges"], json!([]));
        assert!(parsed["suggested_follow_ups"].is_array());
        assert_eq!(parsed["suggested_follow_ups"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn ok_envelope_is_success_variant() {
        let result = ok_envelope("test_tool", &json!({}));
        assert!(result.is_error == Some(false));
    }

    #[test]
    fn ok_envelope_accepts_borrowed_value() {
        let data = serde_json::json!({"key": "value"});
        let result = ok_envelope("test_tool", &data);
        assert!(result.is_error == Some(false));
    }

    #[test]
    fn ok_envelope_accepts_vec_string() {
        let data: Vec<String> = vec!["a".to_string(), "b".to_string()];
        let result = ok_envelope("impact_radius", &data);
        assert!(result.is_error == Some(false));
    }

    // ------------------------------------------------------------------------
    // err_envelope
    // ------------------------------------------------------------------------

    #[test]
    fn err_envelope_json_shape() {
        let msg = "no session with the supplied id";
        let result = err_envelope("brain_open", "session_not_found", msg);

        assert!(result.is_error == Some(true));
        let items = &result.content;
        assert!(!items.is_empty());
        let Content {
            raw: RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();

        assert_eq!(parsed["tool_name"], "brain_open");
        assert!(parsed["provenance"].is_null());
        assert_eq!(parsed["payload"]["error_code"], "session_not_found");
        assert_eq!(parsed["payload"]["error"], msg);
        assert!(parsed["suggested_follow_ups"].is_array());
        assert_eq!(parsed["suggested_follow_ups"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn err_envelope_is_error_variant() {
        let result = err_envelope("test_tool", "err_code", "error message");
        assert!(result.is_error == Some(true));
    }

    #[test]
    fn err_envelope_top_level_keys() {
        let result = err_envelope("tool_x", "code_y", "msg_z");
        assert!(result.is_error == Some(true));
        let items = &result.content;
        assert!(!items.is_empty());
        let Content {
            raw: RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            panic!()
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();

        let keys: Vec<_> = parsed.as_object().unwrap().keys().collect();
        assert!(keys.contains(&&"tool_name".to_string()));
        assert!(keys.contains(&&"version".to_string()));
        assert!(keys.contains(&&"timestamp".to_string()));
        assert!(keys.contains(&&"provenance".to_string()));
        assert!(keys.contains(&&"payload".to_string()));
        assert!(keys.contains(&&"suggested_follow_ups".to_string()));
    }

    // ------------------------------------------------------------------------
    // ok_envelope_with_provenance
    // ------------------------------------------------------------------------

    #[test]
    fn ok_envelope_with_provenance_json_shape() {
        let payload = json!({"answer": "42"});
        let prov = ProvenanceMetadata {
            source: Some("brain-session".to_string()),
            confidence: None,
        };
        let result = ok_envelope_with_provenance("brain_ask", &payload, prov);

        assert!(result.is_error == Some(false));
        let items = &result.content;
        assert!(!items.is_empty());
        let Content {
            raw: RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();

        assert_eq!(parsed["tool_name"], "brain_ask");
        assert_eq!(parsed["provenance"]["source"], "brain-session");
        assert!(parsed["provenance"]["confidence"].is_null());
        assert_eq!(parsed["payload"]["answer"], "42");
    }

    #[test]
    fn ok_envelope_with_provenance_with_confidence() {
        let payload = json!({"result": "ok"});
        let prov = ProvenanceMetadata {
            source: Some("ask-router".to_string()),
            confidence: Some(0.0),
        };
        let result = ok_envelope_with_provenance("cognicode_ask", &payload, prov);

        assert!(result.is_error == Some(false));
        let items = &result.content;
        assert!(!items.is_empty());
        let Content {
            raw: RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();

        assert_eq!(parsed["provenance"]["source"], "ask-router");
        assert_eq!(parsed["provenance"]["confidence"], 0.0);
    }

    #[test]
    fn ok_envelope_with_provenance_is_success_variant() {
        let result =
            ok_envelope_with_provenance("test_tool", &json!({}), ProvenanceMetadata::default());
        assert!(result.is_error == Some(false));
    }

    #[test]
    fn ok_envelope_with_provenance_accepts_borrowed() {
        let data = serde_json::json!({"key": "val"});
        let prov = ProvenanceMetadata::default();
        let result = ok_envelope_with_provenance("tool", &data, prov);
        assert!(result.is_error == Some(false));
    }
}
