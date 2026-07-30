//! C4 and trace Mermaid export tool handlers.
//!
//! Implements 2 MCP tools:
//! - `export_c4_mermaid` — render a C4-level architecture as a Mermaid C4 diagram
//! - `export_trace_mermaid` — render a trace (call-graph, impact-radius,
//!   decision-trace, vertical-slice) as a Mermaid `flowchart` diagram

use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde_json::Value;

use crate::domain::c4_mermaid::{C4Level, c4_to_mermaid};
#[cfg(feature = "multimodal")]
use crate::domain::trace_mermaid::decision_trace_to_mermaid;
use crate::domain::trace_mermaid::{
    TraceEmitContext, TraceMermaidViewKind, call_graph_to_mermaid, impact_radius_to_mermaid,
    vertical_slice_to_mermaid,
};
use crate::dto::{InspectionTarget, SubgraphResponse};
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_EXPORT_C4_MERMAID, TOOL_EXPORT_TRACE_MERMAID};

// ============================================================================
// ToolHandler implementation
// ============================================================================

struct ExportC4MermaidHandler;

#[async_trait]
impl ToolHandler for ExportC4MermaidHandler {
    fn name(&self) -> &'static str {
        TOOL_EXPORT_C4_MERMAID
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "enum": ["context", "container", "component"],
                    "description": "C4 diagram level (context | container | component)"
                }
            },
            "required": ["level"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        struct Args {
            level: String,
        }

        let args: Args = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "invalid_args",
                    &format!("{TOOL_EXPORT_C4_MERMAID}: invalid args: {e}"),
                );
            }
        };

        let c4_level = match C4Level::parse(&args.level) {
            Ok(l) => l,
            Err(e) => {
                return err_envelope(TOOL_EXPORT_C4_MERMAID, "invalid_level", &e.to_string());
            }
        };

        let graph_svc = match ctx.graph_service.as_ref() {
            Some(gs) => gs,
            None => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "facade_unavailable",
                    "graph service not wired",
                );
            }
        };

        let workspace_svc = match ctx.workspace.as_ref() {
            Some(ws) => ws,
            None => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "facade_unavailable",
                    "workspace service not wired",
                );
            }
        };

        let workspace = match workspace_svc.current_workspace() {
            Ok(ws) => ws,
            Err(e) => {
                return err_envelope(TOOL_EXPORT_C4_MERMAID, "workspace_error", &e.to_string());
            }
        };

        let architecture: SubgraphResponse =
            match graph_svc.build_architecture(&workspace.root_path).await {
                Ok(resp) => resp,
                Err(e) => {
                    return err_envelope(TOOL_EXPORT_C4_MERMAID, "service_error", &e.to_string());
                }
            };

        let mermaid = c4_to_mermaid(&architecture.nodes, &architecture.edges, c4_level);
        // Return the raw mermaid string as a JSON payload
        ok_envelope(TOOL_EXPORT_C4_MERMAID, &mermaid)
    }
}

// ============================================================================
// ExportTraceMermaidHandler
// ============================================================================

struct ExportTraceMermaidHandler;

#[async_trait]
impl ToolHandler for ExportTraceMermaidHandler {
    fn name(&self) -> &'static str {
        TOOL_EXPORT_TRACE_MERMAID
    }

    fn arg_schema(&self) -> Value {
        // decision_trace is only available when multimodal feature is enabled
        #[cfg(feature = "multimodal")]
        let view_kind_enum = serde_json::json!([
            "call_graph",
            "impact_radius",
            "decision_trace",
            "vertical_slice"
        ]);
        #[cfg(not(feature = "multimodal"))]
        let view_kind_enum = serde_json::json!(["call_graph", "impact_radius", "vertical_slice"]);

        serde_json::json!({
            "type": "object",
            "properties": {
                "view_kind": {
                    "type": "string",
                    "enum": view_kind_enum,
                    "description": "Trace view kind"
                },
                "target": {
                    "type": "string",
                    "description": "Target symbol id or decision id"
                }
            },
            "required": ["view_kind", "target"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        struct Args {
            view_kind: String,
            target: String,
        }

        let args: Args = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_TRACE_MERMAID,
                    "invalid_args",
                    &format!("{TOOL_EXPORT_TRACE_MERMAID}: invalid args: {e}"),
                );
            }
        };

        // Parse and validate view_kind
        let view_kind = match TraceMermaidViewKind::from_str(&args.view_kind) {
            Ok(vk) => vk,
            Err(e) => {
                return err_envelope(TOOL_EXPORT_TRACE_MERMAID, "invalid_view_kind", &e);
            }
        };

        let graph_svc = match ctx.graph_service.as_ref() {
            Some(gs) => gs,
            None => {
                return err_envelope(
                    TOOL_EXPORT_TRACE_MERMAID,
                    "facade_unavailable",
                    "graph service not wired",
                );
            }
        };

        let graph_query = match graph_svc.graph_query() {
            Some(gq) => gq,
            None => {
                return err_envelope(
                    TOOL_EXPORT_TRACE_MERMAID,
                    "graph_unavailable",
                    "call graph not loaded",
                );
            }
        };

        // Resolve the target symbol
        let resolved = match graph_svc.resolve_symbol(&args.target).await {
            Ok(Some(r)) => r,
            Ok(None) => {
                return err_envelope(
                    TOOL_EXPORT_TRACE_MERMAID,
                    "symbol_not_found",
                    &format!("target not found: {}", args.target),
                );
            }
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_TRACE_MERMAID,
                    "resolution_failed",
                    &e.to_string(),
                );
            }
        };

        let target = InspectionTarget::Symbol(resolved);
        let trace_ctx = TraceEmitContext {
            graph_query: graph_query.as_ref(),
            target: &target,
        };

        let mermaid = match view_kind {
            TraceMermaidViewKind::CallGraph => call_graph_to_mermaid(&trace_ctx, &args.target),
            TraceMermaidViewKind::ImpactRadius => {
                impact_radius_to_mermaid(&trace_ctx, &args.target)
            }
            #[cfg(feature = "multimodal")]
            TraceMermaidViewKind::DecisionTrace => {
                return match decision_trace_to_mermaid(&trace_ctx, &args.target) {
                    Ok(m) => ok_envelope(TOOL_EXPORT_TRACE_MERMAID, &m),
                    Err(e) => {
                        err_envelope(TOOL_EXPORT_TRACE_MERMAID, "not_implemented", &e.to_string())
                    }
                };
            }
            TraceMermaidViewKind::VerticalSlice => {
                vertical_slice_to_mermaid(&trace_ctx, &args.target)
            }
        };

        ok_envelope(TOOL_EXPORT_TRACE_MERMAID, &mermaid)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the export-family handlers into the registry.
pub fn register_export_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(ExportC4MermaidHandler);
    registry.register(ExportTraceMermaidHandler);
}

// ============================================================================
// Integration tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::dto::SubgraphResponse;
    use crate::error::{ExplorerError, ExplorerResult};
    use crate::facades::GraphService;
    use crate::mcp::handler::ToolHandlerRegistry;
    use crate::ports::symbol_repository::ResolvedSymbol;
    use crate::session::SessionRegistry;
    use async_trait::async_trait;
    use cognicode_core::domain::aggregates::{CallEntry, SymbolId};
    use cognicode_core::domain::traits::{
        CalleeWithMetadata, CallerWithMetadata, GraphQueryPort, RelationTarget,
        RelationTargetWithMetadata,
    };
    use cognicode_core::domain::value_objects::SymbolKind;
    use rmcp::model::CallToolResult;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::Arc;

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

    fn ok_payload(result: &CallToolResult) -> Value {
        assert_eq!(
            result.is_error,
            Some(false),
            "expected ok envelope, got: {result:?}"
        );
        extract_env(result)["payload"].clone()
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

    fn build_registry() -> ToolHandlerRegistry {
        let mut r = ToolHandlerRegistry::new();
        register_export_handlers(&mut r);
        r
    }

    // ------------------------------------------------------------------------
    // Mock GraphQueryPort for trace handler tests
    // ------------------------------------------------------------------------

    #[derive(Clone)]
    struct MockGraphQueryPort {
        callers_result: Vec<RelationTarget>,
        callees_result: Vec<RelationTarget>,
        traverse_callers_result: Vec<CallEntry>,
        traverse_callees_result: Vec<CallEntry>,
    }

    impl MockGraphQueryPort {
        fn new() -> Self {
            Self {
                callers_result: vec![],
                callees_result: vec![],
                traverse_callers_result: vec![],
                traverse_callees_result: vec![],
            }
        }
        fn with_callers(mut self, callers: Vec<RelationTarget>) -> Self {
            self.callers_result = callers;
            self
        }
        fn with_callees(mut self, callees: Vec<RelationTarget>) -> Self {
            self.callees_result = callees;
            self
        }
        fn with_traverse_callers(mut self, entries: Vec<CallEntry>) -> Self {
            self.traverse_callers_result = entries;
            self
        }
        fn with_traverse_callees(mut self, entries: Vec<CallEntry>) -> Self {
            self.traverse_callees_result = entries;
            self
        }
    }

    impl GraphQueryPort for MockGraphQueryPort {
        fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            self.callers_result.clone()
        }
        fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            self.callees_result.clone()
        }
        fn fan_in(&self, _id: &SymbolId) -> usize {
            0
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
        }
        fn callers_with_metadata(&self, _id: &SymbolId) -> Vec<CallerWithMetadata> {
            vec![]
        }
        fn callees_with_metadata(&self, _id: &SymbolId) -> Vec<CalleeWithMetadata> {
            vec![]
        }
        fn dependencies_with_metadata(&self, _id: &SymbolId) -> Vec<RelationTargetWithMetadata> {
            vec![]
        }
        fn traverse_callees(&self, _id: &SymbolId, _max_depth: u8) -> Vec<CallEntry> {
            self.traverse_callees_result.clone()
        }
        fn traverse_callers(&self, _id: &SymbolId, _max_depth: u8) -> Vec<CallEntry> {
            self.traverse_callers_result.clone()
        }
    }

    fn make_relation_target(id: &str, name: &str) -> RelationTarget {
        RelationTarget {
            id: SymbolId::new(id.to_string()),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: "test.rs".to_string(),
            line: 1,
            signature: None,
        }
    }

    fn make_call_entry(id: &str, name: &str, depth: u8) -> CallEntry {
        CallEntry {
            symbol_id: SymbolId::new(id.to_string()),
            symbol_name: name.to_string(),
            file: "test.rs".to_string(),
            line: 1,
            column: 1,
            depth,
        }
    }

    fn make_resolved_symbol(id: &str, name: &str) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(id.to_string()),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: "test.rs".to_string(),
            line: 1,
            signature: None,
        }
    }

    #[derive(Clone)]
    struct MockGraphService {
        resolved_symbols: HashMap<String, ResolvedSymbol>,
        graph_query: Arc<MockGraphQueryPort>,
    }

    impl MockGraphService {
        fn new(graph_query: MockGraphQueryPort) -> Self {
            Self {
                resolved_symbols: HashMap::new(),
                graph_query: Arc::new(graph_query),
            }
        }
        fn with_symbol(mut self, id: &str, symbol: ResolvedSymbol) -> Self {
            self.resolved_symbols.insert(id.to_string(), symbol);
            self
        }
    }

    #[async_trait]
    impl GraphService for MockGraphService {
        async fn resolve_symbol(&self, id: &str) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.resolved_symbols.get(id).cloned())
        }
        fn graph_query(&self) -> Option<Arc<dyn GraphQueryPort>> {
            Some(self.graph_query.clone() as Arc<dyn GraphQueryPort>)
        }
        async fn build_subgraph(
            &self,
            _root_id: &str,
            _depth: u8,
            _direction: crate::facades::SubgraphDirection,
            _max_nodes: u32,
        ) -> ExplorerResult<SubgraphResponse> {
            Ok(SubgraphResponse {
                root: String::new(),
                nodes: vec![],
                edges: vec![],
                truncated: false,
                truncated_reason: None,
                corroboration_scores: std::collections::HashMap::new(),
            })
        }
        async fn build_architecture(&self, _root_path: &str) -> ExplorerResult<SubgraphResponse> {
            Ok(SubgraphResponse {
                root: String::new(),
                nodes: vec![],
                edges: vec![],
                truncated: false,
                truncated_reason: None,
                corroboration_scores: std::collections::HashMap::new(),
            })
        }
        async fn compare_architecture(
            &self,
            _root_path: &str,
        ) -> ExplorerResult<crate::dto::DriftReport> {
            Ok(crate::dto::DriftReport {
                findings: vec![],
                summary: String::new(),
                missing_containers: 0,
                extra_containers: 0,
                wrong_sub_kinds: 0,
                boundary_violations: 0,
            })
        }
        async fn landing_entry_points(
            &self,
            _limit: usize,
        ) -> ExplorerResult<(Vec<ResolvedSymbol>, usize)> {
            Ok((vec![], 0))
        }
        async fn landing_hot_paths(
            &self,
            _limit: usize,
            _min_fan_in: usize,
        ) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        async fn landing_god_nodes(
            &self,
            _limit: usize,
        ) -> ExplorerResult<Vec<crate::dto::GodNodeEntry>> {
            Ok(vec![])
        }
    }

    fn make_ctx(graph_svc: MockGraphService) -> McpContext {
        McpContext::builder()
            .with_graph(None)
            .with_session_registry(SessionRegistry::new())
            .with_graph_service(Arc::new(graph_svc) as Arc<dyn GraphService>)
            .build()
    }

    // ------------------------------------------------------------------------
    // export_trace_mermaid — happy path
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn export_trace_mermaid_call_graph_ok() {
        let mock_gq = MockGraphQueryPort::new()
            .with_callers(vec![make_relation_target("caller1", "caller_one")])
            .with_callees(vec![make_relation_target("callee1", "callee_one")]);

        let graph_svc = MockGraphService::new(mock_gq).with_symbol(
            "symbol:test:fn:1",
            make_resolved_symbol("symbol:test:fn:1", "my_function"),
        );

        let ctx = make_ctx(graph_svc);
        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(
                &ctx,
                json!({"view_kind": "call_graph", "target": "symbol:test:fn:1"}),
            )
            .await;

        let payload = ok_payload(&result);
        assert!(payload.is_string(), "payload should be mermaid string");
        let s = payload.as_str().unwrap();
        assert!(s.contains("flowchart TD"), "should be flowchart TD");
        assert!(
            s.contains("subgraph call_graph"),
            "should have call_graph subgraph"
        );
    }

    #[tokio::test]
    async fn export_trace_mermaid_impact_radius_ok() {
        let mock_gq = MockGraphQueryPort::new().with_traverse_callers(vec![
            make_call_entry("caller1", "direct_caller", 1),
            make_call_entry("caller2", "indirect_caller", 2),
        ]);

        let graph_svc = MockGraphService::new(mock_gq).with_symbol(
            "symbol:test:fn:1",
            make_resolved_symbol("symbol:test:fn:1", "target_fn"),
        );

        let ctx = make_ctx(graph_svc);
        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(
                &ctx,
                json!({"view_kind": "impact_radius", "target": "symbol:test:fn:1"}),
            )
            .await;

        let payload = ok_payload(&result);
        let s = payload.as_str().unwrap();
        assert!(s.contains("flowchart TD"), "should be flowchart TD");
        assert!(
            s.contains("subgraph impact_radius"),
            "should have impact_radius subgraph"
        );
    }

    #[tokio::test]
    async fn export_trace_mermaid_vertical_slice_ok() {
        let mock_gq = MockGraphQueryPort::new().with_traverse_callees(vec![
            make_call_entry("usecase1", "create_user", 1),
            make_call_entry("domain1", "user_entity", 2),
        ]);

        let graph_svc = MockGraphService::new(mock_gq).with_symbol(
            "symbol:test:handle:1",
            make_resolved_symbol("symbol:test:handle:1", "handle_request"),
        );

        let ctx = make_ctx(graph_svc);
        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(
                &ctx,
                json!({"view_kind": "vertical_slice", "target": "symbol:test:handle:1"}),
            )
            .await;

        let payload = ok_payload(&result);
        let s = payload.as_str().unwrap();
        assert!(s.contains("flowchart TD"), "should be flowchart TD");
        assert!(
            s.contains("subgraph vertical_slice"),
            "should have vertical_slice subgraph"
        );
    }

    // ------------------------------------------------------------------------
    // export_trace_mermaid — invalid args
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn export_trace_mermaid_invalid_view_kind() {
        let mock_gq = MockGraphQueryPort::new();
        let graph_svc = MockGraphService::new(mock_gq);
        let ctx = make_ctx(graph_svc);

        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(
                &ctx,
                json!({"view_kind": "invalid_view", "target": "symbol:test:fn:1"}),
            )
            .await;

        assert_eq!(err_code(&result), "invalid_view_kind");
    }

    #[tokio::test]
    async fn export_trace_mermaid_missing_target() {
        let mock_gq = MockGraphQueryPort::new();
        let graph_svc = MockGraphService::new(mock_gq);
        let ctx = make_ctx(graph_svc);

        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(&ctx, json!({"view_kind": "call_graph"}))
            .await;

        assert_eq!(err_code(&result), "invalid_args");
    }

    #[tokio::test]
    async fn export_trace_mermaid_symbol_not_found() {
        let mock_gq = MockGraphQueryPort::new();
        let graph_svc = MockGraphService::new(mock_gq);
        let ctx = make_ctx(graph_svc);

        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(
                &ctx,
                json!({"view_kind": "call_graph", "target": "nonexistent"}),
            )
            .await;

        assert_eq!(err_code(&result), "symbol_not_found");
    }

    // ------------------------------------------------------------------------
    // export_trace_mermaid — decision_trace multimodal gate
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn export_trace_mermaid_decision_trace_requires_multimodal() {
        let mock_gq = MockGraphQueryPort::new();
        let graph_svc = MockGraphService::new(mock_gq).with_symbol(
            "symbol:test:fn:1",
            make_resolved_symbol("symbol:test:fn:1", "test_fn"),
        );

        let ctx = make_ctx(graph_svc);
        let handler = ExportTraceMermaidHandler;
        let result = handler
            .handle(
                &ctx,
                json!({"view_kind": "decision_trace", "target": "symbol:test:fn:1"}),
            )
            .await;

        // In non-multimodal builds, decision_trace is not in the schema enum,
        // so from_str returns invalid_view_kind error
        #[cfg(not(feature = "multimodal"))]
        {
            assert_eq!(err_code(&result), "invalid_view_kind");
        }
        // In multimodal builds, decision_trace_to_mermaid returns
        // Err(TraceMermaidError::NotImplemented) (E24.3 deferred)
        #[cfg(feature = "multimodal")]
        {
            assert_eq!(err_code(&result), "not_implemented");
        }
    }

    // ------------------------------------------------------------------------
    // export_trace_mermaid — tool in registry
    // ------------------------------------------------------------------------

    #[test]
    fn export_trace_mermaid_registered_in_registry() {
        let mut registry = ToolHandlerRegistry::new();
        register_export_handlers(&mut registry);

        let handler = registry.get(TOOL_EXPORT_TRACE_MERMAID);
        assert!(
            handler.is_some(),
            "export_trace_mermaid should be registered"
        );
        assert_eq!(handler.unwrap().name(), TOOL_EXPORT_TRACE_MERMAID);
    }
}
