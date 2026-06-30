//! Snapshot export MCP tool.
//!
//! Renders a canonical Mermaid diagram (C4 or trace) to PNG or SVG and
//! returns the result as base64-encoded bytes inside the MCP JSON envelope.
//!
//! This tool is gated by the `multimodal` feature because it depends on
//! `SnapshotService` which requires `mmdc` (Mermaid CLI) to be installed.

#[cfg(feature = "multimodal")]

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::c4_mermaid::{c4_to_mermaid, C4Level};
use crate::domain::snapshot::{SnapshotError, SnapshotFormat, SnapshotService};
use crate::domain::trace_mermaid::{
    call_graph_to_mermaid, impact_radius_to_mermaid, vertical_slice_to_mermaid, TraceEmitContext,
};
use crate::dto::InspectionTarget;
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::McpContext;

#[cfg(feature = "multimodal")]
use crate::mcp::TOOL_EXPORT_SNAPSHOT;

/// Whitelist of view kinds that support snapshot rendering.
const SNAPSHOT_VIEW_KINDS: &[&str] = &[
    "c4_context",
    "c4_container",
    "c4_component",
    "call_graph",
    "impact_radius",
    "vertical_slice",
];

/// Snapshot-specific view kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotViewKind {
    C4Context,
    C4Container,
    C4Component,
    CallGraph,
    ImpactRadius,
    VerticalSlice,
}

impl SnapshotViewKind {
    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "c4_context" => Ok(Self::C4Context),
            "c4_container" => Ok(Self::C4Container),
            "c4_component" => Ok(Self::C4Component),
            "call_graph" => Ok(Self::CallGraph),
            "impact_radius" => Ok(Self::ImpactRadius),
            "vertical_slice" => Ok(Self::VerticalSlice),
            other => Err(other.to_string()),
        }
    }

    fn is_trace_kind(&self) -> bool {
        matches!(
            self,
            Self::CallGraph | Self::ImpactRadius | Self::VerticalSlice
        )
    }
}

/// Payload returned on success — base64-encoded image bytes + format field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotPayload {
    /// Base64-encoded PNG or SVG bytes.
    pub data: String,
    /// Output format: `"png"` or `"svg"`.
    pub format: String,
}

// ============================================================================
// ToolHandler implementation
// ============================================================================

struct ExportSnapshotHandler;

#[async_trait]
#[cfg(feature = "multimodal")]
impl ToolHandler for ExportSnapshotHandler {
    fn name(&self) -> &'static str {
        TOOL_EXPORT_SNAPSHOT
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "view_kind": {
                    "type": "string",
                    "enum": SNAPSHOT_VIEW_KINDS,
                    "description": "View kind to render (c4_context | c4_container | c4_component | call_graph | impact_radius | vertical_slice)"
                },
                "target": {
                    "type": "string",
                    "description": "Target symbol id or entry point id. Required for trace view kinds (call_graph, impact_radius, vertical_slice); ignored for C4 view kinds."
                },
                "format": {
                    "type": "string",
                    "enum": ["png", "svg"],
                    "description": "Output format: png or svg"
                }
            },
            "required": ["view_kind", "format"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        struct Args {
            view_kind: String,
            target: Option<String>,
            format: String,
        }

        let args: Args = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_SNAPSHOT,
                    "invalid_args",
                    &format!("{TOOL_EXPORT_SNAPSHOT}: invalid args: {e}"),
                );
            }
        };

        // Validate view_kind against whitelist
        if !SNAPSHOT_VIEW_KINDS.contains(&args.view_kind.as_str()) {
            return err_envelope(
                TOOL_EXPORT_SNAPSHOT,
                "invalid_view_kind",
                &format!(
                    "invalid view_kind: {} (expected: {})",
                    args.view_kind,
                    SNAPSHOT_VIEW_KINDS.join(", ")
                ),
            );
        }

        let view_kind = match SnapshotViewKind::from_str(&args.view_kind) {
            Ok(vk) => vk,
            Err(_) => {
                return err_envelope(
                    TOOL_EXPORT_SNAPSHOT,
                    "invalid_view_kind",
                    &format!("unknown view_kind: {}", args.view_kind),
                );
            }
        };

        // Parse format
        let format = match SnapshotFormat::parse(&args.format) {
            Ok(f) => f,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_SNAPSHOT,
                    "invalid_format",
                    &e.to_string(),
                );
            }
        };

        // Get Mermaid text
        let mermaid_text = match emit_mermaid_for_snapshot(ctx, view_kind, args.target.as_deref()).await
        {
            Ok(text) => text,
            Err(msg) => {
                return err_envelope(TOOL_EXPORT_SNAPSHOT, "mermaid_error", &msg);
            }
        };

        // Render to PNG/SVG via SnapshotService
        let snapshot_svc = SnapshotService::new();
        let bytes = match snapshot_svc.render(&mermaid_text, format).await {
            Ok(data) => data,
            Err(e) => {
                let (code, msg) = snapshot_error_to_code_and_msg(e);
                return err_envelope(TOOL_EXPORT_SNAPSHOT, code, &msg);
            }
        };

        // Base64-encode the bytes
        let encoded = BASE64.encode(&bytes);

        let payload = SnapshotPayload {
            data: encoded,
            format: args.format,
        };

        ok_envelope(TOOL_EXPORT_SNAPSHOT, &payload)
    }
}

/// Emit Mermaid text for a given snapshot view kind.
async fn emit_mermaid_for_snapshot(
    ctx: &McpContext,
    view_kind: SnapshotViewKind,
    target: Option<&str>,
) -> Result<String, String> {
    if view_kind.is_trace_kind() {
        // Trace diagram — use trace emitters
        let target = target.ok_or_else(|| "target is required for trace view kinds")?;

        let graph_svc = ctx
            .graph_service
            .as_ref()
            .ok_or_else(|| "graph service not wired")?;

        let graph_query = graph_svc
            .graph_query()
            .ok_or_else(|| "call graph not loaded")?;

        let resolved = graph_svc
            .resolve_symbol(target)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("target not found: {target}"))?;

        let inspection_target = InspectionTarget::Symbol(resolved);
        let trace_ctx = TraceEmitContext {
            graph_query: graph_query.as_ref(),
            target: &inspection_target,
        };

        let mermaid = match view_kind {
            SnapshotViewKind::CallGraph => call_graph_to_mermaid(&trace_ctx, target),
            SnapshotViewKind::ImpactRadius => impact_radius_to_mermaid(&trace_ctx, target),
            SnapshotViewKind::VerticalSlice => vertical_slice_to_mermaid(&trace_ctx, target),
            _ => unreachable!(),
        };

        Ok(mermaid)
    } else {
        // C4 diagram — use c4_to_mermaid
        let level = match view_kind {
            SnapshotViewKind::C4Context => C4Level::Context,
            SnapshotViewKind::C4Container => C4Level::Container,
            SnapshotViewKind::C4Component => C4Level::Component,
            _ => unreachable!(),
        };

        let workspace_svc = ctx
            .workspace
            .as_ref()
            .ok_or_else(|| "workspace service not wired")?;

        let workspace = workspace_svc
            .current_workspace()
            .map_err(|e| e.to_string())?;

        let graph_svc = ctx
            .graph_service
            .as_ref()
            .ok_or_else(|| "graph service not wired")?;

        let architecture = graph_svc
            .build_architecture(&workspace.root_path)
            .await
            .map_err(|e| e.to_string())?;

        Ok(c4_to_mermaid(
            &architecture.nodes,
            &architecture.edges,
            level,
        ))
    }
}

/// Map SnapshotError to (error_code, message) pair.
fn snapshot_error_to_code_and_msg(err: SnapshotError) -> (&'static str, String) {
    use SnapshotError::*;
    match err {
        MermaidEmpty => ("empty_input", err.to_string()),
        SizeLimitExceeded { size } => {
            ("size_limit_exceeded", format!("mermaid text exceeds 1 MB size limit ({size} bytes)"))
        }
        MmdcNotFound => (
            "mmdc_not_found",
            "mmdc not found — install mermaid-cli: npm install -g @mermaid-js/mermaid-cli".to_string(),
        ),
        RenderFailed(msg) => ("render_failed", format!("mmdc render failed: {msg}")),
        Timeout(dur) => ("timeout", format!("render timed out after {dur:?}")),
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the snapshot-family handlers into the registry.
#[cfg(feature = "multimodal")]
pub fn register_snapshot_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(ExportSnapshotHandler);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::mcp::handler::ToolHandlerRegistry;
    use rmcp::model::CallToolResult;
    use serde_json::{json, Value};

    // ------------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------------

    fn extract_env(result: &CallToolResult) -> Value {
        let text = result
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.as_str())
            .expect("CallToolResult should contain a text content");
        serde_json::from_str(text).expect("response text must be JSON")
    }

    fn err_code(result: &CallToolResult) -> String {
        assert_eq!(
            result.is_error,
            Some(true),
            "expected err envelope, got: {result:?}"
        );
        let env = extract_env(result);
        env["payload"]["error_code"]
            .as_str()
            .expect("err envelope payload must have `error_code`")
            .to_string()
    }

    // ------------------------------------------------------------------------
    // SnapshotViewKind::from_str
    // ------------------------------------------------------------------------

    #[test]
    fn snapshot_view_kind_from_str_valid() {
        assert!(SnapshotViewKind::from_str("c4_context").is_ok());
        assert!(SnapshotViewKind::from_str("c4_container").is_ok());
        assert!(SnapshotViewKind::from_str("c4_component").is_ok());
        assert!(SnapshotViewKind::from_str("call_graph").is_ok());
        assert!(SnapshotViewKind::from_str("impact_radius").is_ok());
        assert!(SnapshotViewKind::from_str("vertical_slice").is_ok());
    }

    #[test]
    fn snapshot_view_kind_from_str_invalid() {
        let result = SnapshotViewKind::from_str("invalid");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "invalid");

        let result2 = SnapshotViewKind::from_str("decision_trace");
        assert!(result2.is_err());
        assert_eq!(result2.unwrap_err(), "decision_trace");
    }

    #[test]
    fn snapshot_view_kind_is_trace_kind() {
        assert!(!SnapshotViewKind::from_str("c4_context").unwrap().is_trace_kind());
        assert!(!SnapshotViewKind::from_str("c4_container").unwrap().is_trace_kind());
        assert!(!SnapshotViewKind::from_str("c4_component").unwrap().is_trace_kind());
        assert!(SnapshotViewKind::from_str("call_graph").unwrap().is_trace_kind());
        assert!(SnapshotViewKind::from_str("impact_radius").unwrap().is_trace_kind());
        assert!(SnapshotViewKind::from_str("vertical_slice").unwrap().is_trace_kind());
    }

    // ------------------------------------------------------------------------
    // Tool is registered
    // ------------------------------------------------------------------------

    #[test]
    fn export_snapshot_registered_in_registry() {
        let mut registry = ToolHandlerRegistry::new();
        register_snapshot_handlers(&mut registry);

        let handler = registry.get(TOOL_EXPORT_SNAPSHOT);
        assert!(handler.is_some(), "export_snapshot should be registered");
        assert_eq!(handler.unwrap().name(), TOOL_EXPORT_SNAPSHOT);
    }

    // ------------------------------------------------------------------------
    // Arg schema
    // ------------------------------------------------------------------------

    #[test]
    fn export_snapshot_arg_schema_has_required_fields() {
        let handler = ExportSnapshotHandler;
        let schema = handler.arg_schema();
        let schema_obj = schema.as_object().expect("schema should be an object");

        // Check properties exist
        let props = schema_obj.get("properties").expect("schema should have properties");
        let props_obj = props.as_object().expect("properties should be an object");

        assert!(props_obj.contains_key("view_kind"), "should have view_kind property");
        assert!(props_obj.contains_key("format"), "should have format property");
        // target is optional for C4 view kinds
        assert!(props_obj.contains_key("target"), "should have target property");

        // Check required fields
        let required = schema_obj.get("required").expect("schema should have required");
        let required_arr = required.as_array().expect("required should be an array");
        let required_strs: Vec<_> = required_arr.iter().filter_map(|v| v.as_str()).collect();
        assert!(required_strs.contains(&"view_kind"), "view_kind should be required");
        assert!(required_strs.contains(&"format"), "format should be required");
    }

    // ------------------------------------------------------------------------
    // Format parsing (delegates to SnapshotFormat::parse)
    // ------------------------------------------------------------------------

    #[test]
    fn snapshot_format_parse_png() {
        use crate::domain::snapshot::SnapshotFormat;
        assert!(SnapshotFormat::parse("png").is_ok());
        assert!(SnapshotFormat::parse("PNG").is_ok());
    }

    #[test]
    fn snapshot_format_parse_svg() {
        use crate::domain::snapshot::SnapshotFormat;
        assert!(SnapshotFormat::parse("svg").is_ok());
        assert!(SnapshotFormat::parse("SVG").is_ok());
    }

    #[test]
    fn snapshot_format_parse_invalid() {
        use crate::domain::snapshot::SnapshotFormat;
        assert!(SnapshotFormat::parse("jpg").is_err());
        assert!(SnapshotFormat::parse("pdf").is_err());
    }

    // ------------------------------------------------------------------------
    // Error code mapping
    // ------------------------------------------------------------------------

    #[test]
    fn snapshot_error_to_code_mmdc_not_found() {
        use crate::domain::snapshot::SnapshotError;
        let err = SnapshotError::MmdcNotFound;
        let (code, _) = snapshot_error_to_code_and_msg(err);
        assert_eq!(code, "mmdc_not_found");
    }

    #[test]
    fn snapshot_error_to_code_render_failed() {
        use crate::domain::snapshot::SnapshotError;
        let err = SnapshotError::RenderFailed("test error".to_string());
        let (code, msg) = snapshot_error_to_code_and_msg(err);
        assert_eq!(code, "render_failed");
        assert!(msg.contains("test error"));
    }

    #[test]
    fn snapshot_error_to_code_empty() {
        use crate::domain::snapshot::SnapshotError;
        let err = SnapshotError::MermaidEmpty;
        let (code, _) = snapshot_error_to_code_and_msg(err);
        assert_eq!(code, "empty_input");
    }
}
