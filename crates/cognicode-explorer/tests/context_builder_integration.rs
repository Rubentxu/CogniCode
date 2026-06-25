//! Integration tests for the context-builder handler (`build_context`).
//!
//! Mirrors the pattern used by `lens_mcp_integration.rs` and
//! `internal_mcp_integration.rs`: wire a real `McpContext` with the
//! necessary ports (SearchService, ViewService, GraphQueryPort,
//! QualityRepository) and dispatch the handler end-to-end through
//! `ToolHandlerRegistry::dispatch`.
//!
//! Coverage:
//! - happy path (all ports wired) returns markdown + json + summary
//! - graceful degradation per port:
//!   - no view → lenses skipped, sources_skipped mentions it
//!   - no quality → quality slice is null, sources_skipped mentions it
//!   - no graph_query → graph slice is null, sources_skipped mentions it
//! - error envelopes:
//!   - missing object_id → missing_required_arg
//!   - empty object_id → missing_required_arg
//!   - service error from inspect_object → service_error
//! - markdown rendering includes label + object_id

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::traits::{
    CalleeWithMetadata, CallerWithMetadata, GraphQueryPort, RelationTarget,
    RelationTargetWithMetadata,
};
use cognicode_core::domain::aggregates::CallEntry;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_explorer::dto::{
    ContextualGraphResponse, ContextualView, DesignFinding, FindingSeverity,
    InspectableObjectSummary, InspectableObjectType, LensDescriptor, LensResult, Property,
    SpotterResult, SpotterSearchResult, ViewDescriptorDto,
};
use cognicode_explorer::error::{ExplorerError, ExplorerResult};
use cognicode_explorer::facades::{SearchService, ViewService};
use cognicode_explorer::mcp::handler::ToolHandlerRegistry;
use cognicode_explorer::mcp::handler::context_builder::register_context_builder_handlers;
use cognicode_explorer::mcp::{McpContext, TOOL_BUILD_CONTEXT};
use cognicode_explorer::ports::quality_repository::{
    IssueFilter, QualityGateSummary, QualityIssue, QualityRepository, RuleSummary,
};
use cognicode_explorer::session::SessionRegistry;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};

// ============================================================================
// Helpers (mirror internal_mcp_integration.rs)
// ============================================================================

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
    register_context_builder_handlers(&mut r);
    r
}

// ============================================================================
// Mock ports
// ============================================================================

/// Mock SearchService that returns canned `InspectableObjectSummary`
/// values keyed by object_id.
struct MockSearch {
    summaries: HashMap<String, InspectableObjectSummary>,
    default_error: Option<String>,
}

impl MockSearch {
    fn new() -> Self {
        Self {
            summaries: HashMap::new(),
            default_error: None,
        }
    }

    fn with(mut self, id: &str, summary: InspectableObjectSummary) -> Self {
        self.summaries.insert(id.to_string(), summary);
        self
    }

    fn with_error(mut self, msg: &str) -> Self {
        self.default_error = Some(msg.to_string());
        self
    }
}

fn make_symbol_summary(id: &str, label: &str, file: &str, line: u32) -> InspectableObjectSummary {
    InspectableObjectSummary {
        id: id.to_string(),
        object_type: InspectableObjectType::Symbol,
        label: label.to_string(),
        subtitle: format!("{file}:{line}"),
        properties: vec![
            Property {
                key: "file".to_string(),
                value: json!(file),
                value_type: "string".to_string(),
                source: "extracted".to_string(),
            },
            Property {
                key: "line".to_string(),
                value: json!(line),
                value_type: "integer".to_string(),
                source: "extracted".to_string(),
            },
            Property {
                key: "kind".to_string(),
                value: json!("function"),
                value_type: "string".to_string(),
                source: "extracted".to_string(),
            },
        ],
        available_views: vec![],
    }
}

#[async_trait]
impl SearchService for MockSearch {
    async fn inspect_object(&self, object_id: &str) -> ExplorerResult<InspectableObjectSummary> {
        if let Some(msg) = &self.default_error {
            return Err(ExplorerError::InvalidInput(msg.clone()));
        }
        self.summaries
            .get(object_id)
            .cloned()
            .ok_or_else(|| ExplorerError::NotFound(format!("object {object_id} not found")))
    }

    async fn spotter_search(
        &self,
        _query: &str,
        _kind: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterResult>> {
        Ok(vec![])
    }

    async fn spotter_search_with_viewspecs(
        &self,
        _query: &str,
        _kind: Option<&str>,
        _workspace_id: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterSearchResult>> {
        Ok(vec![])
    }
}

/// Mock ViewService that returns canned LensResult per (object, lens).
struct MockView {
    /// (object_id, lens_id) → Vec<DesignFinding tuples>
    canned: HashMap<(String, String), Vec<(String, String, f32)>>,
    /// lens_id → summary string (otherwise default)
    summaries: HashMap<String, String>,
}

impl MockView {
    fn new() -> Self {
        Self {
            canned: HashMap::new(),
            summaries: HashMap::new(),
        }
    }

    fn with_lens(
        mut self,
        object_id: &str,
        lens_id: &str,
        summary: &str,
        findings: Vec<(&str, &str, f32)>,
    ) -> Self {
        self.canned.insert(
            (object_id.to_string(), lens_id.to_string()),
            findings
                .into_iter()
                .map(|(t, sev, c)| (t.to_string(), sev.to_string(), c))
                .collect(),
        );
        self.summaries.insert(lens_id.to_string(), summary.to_string());
        self
    }
}

#[async_trait]
impl ViewService for MockView {
    async fn available_views(
        &self,
        _object_id: &str,
    ) -> ExplorerResult<Vec<ViewDescriptorDto>> {
        Ok(vec![])
    }
    async fn contextual_view(
        &self,
        _object_id: &str,
        _view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn build_contextual_graph(
        &self,
        _focus_id: &str,
        _level: &str,
        _depth: u8,
        _max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn available_lenses(
        &self,
        _object_id: &str,
    ) -> ExplorerResult<Vec<LensDescriptor>> {
        Ok(vec![])
    }
    async fn apply_lens(
        &self,
        object_id: &str,
        lens_id: &str,
    ) -> ExplorerResult<LensResult> {
        let summary = self
            .summaries
            .get(lens_id)
            .cloned()
            .unwrap_or_else(|| format!("default summary for {lens_id}"));
        let findings = self
            .canned
            .get(&(object_id.to_string(), lens_id.to_string()))
            .cloned()
            .unwrap_or_default();
        let design_findings: Vec<DesignFinding> = findings
            .into_iter()
            .enumerate()
            .map(|(i, (title, severity_str, confidence))| {
                let severity = match severity_str.as_str() {
                    "Critical" => FindingSeverity::Critical,
                    "Warning" => FindingSeverity::Warning,
                    "Info" => FindingSeverity::Info,
                    _ => FindingSeverity::Info,
                };
                DesignFinding {
                    id: format!("f:{lens_id}:{i}"),
                    lens_id: lens_id.to_string(),
                    title,
                    hypothesis: String::new(),
                    severity,
                    confidence,
                    object_ids: vec![],
                    evidence_ids: vec![],
                }
            })
            .collect();
        Ok(LensResult {
            lens_id: lens_id.to_string(),
            findings: design_findings,
            summary,
        })
    }
    async fn execute_view_spec(
        &self,
        _spec: &cognicode_explorer::dto::ViewSpec,
        _object_id: &str,
    ) -> ExplorerResult<ContextualView> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
}

/// Mock GraphQueryPort backed by a `CallGraph` + traversal methods.
struct MockGraph {
    graph: CallGraph,
}

impl MockGraph {
    fn new(graph: CallGraph) -> Self {
        Self { graph }
    }
}

#[async_trait]
impl GraphQueryPort for MockGraph {
    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        vec![]
    }
    fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        vec![]
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
    fn traverse_callees(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry> {
        self.graph.traverse_callees(id, max_depth)
    }
    fn traverse_callers(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry> {
        self.graph.traverse_callers(id, max_depth)
    }
}

/// Mock QualityRepository keyed by file.
struct MockQuality {
    by_file: HashMap<String, Vec<QualityIssue>>,
}

impl MockQuality {
    fn new() -> Self {
        Self {
            by_file: HashMap::new(),
        }
    }
    fn with(mut self, file: &str, issues: Vec<QualityIssue>) -> Self {
        self.by_file.insert(file.to_string(), issues);
        self
    }
}

fn issue(id: i64, severity: &str, file: &str, msg: &str, line: u32) -> QualityIssue {
    QualityIssue {
        id,
        rule_id: format!("R{id}"),
        severity: severity.to_string(),
        category: "complexity".to_string(),
        file: file.to_string(),
        line,
        message: msg.to_string(),
        status: "open".to_string(),
    }
}

#[async_trait]
impl QualityRepository for MockQuality {
    fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<QualityIssue>> {
        Ok(self.by_file.get(file).cloned().unwrap_or_default())
    }
    fn issues_for_scope(&self, scope: &str) -> ExplorerResult<Vec<QualityIssue>> {
        Ok(self
            .by_file
            .iter()
            .filter(|(f, _)| f.starts_with(scope))
            .flat_map(|(_, v)| v.clone())
            .collect())
    }
    fn issues_at_line(&self, file: &str, line: u32) -> ExplorerResult<Vec<QualityIssue>> {
        Ok(self
            .by_file
            .get(file)
            .map(|v| v.iter().filter(|i| i.line == line).cloned().collect())
            .unwrap_or_default())
    }
    fn issue_by_id(&self, _id: i64) -> ExplorerResult<Option<QualityIssue>> {
        Ok(None)
    }
    fn rule_summary(&self, _rule_id: &str) -> ExplorerResult<RuleSummary> {
        Ok(RuleSummary {
            rule_id: "mock".into(),
            description: "mock".into(),
            open_count: 0,
        })
    }
    fn quality_gate(&self) -> ExplorerResult<QualityGateSummary> {
        Ok(QualityGateSummary::default())
    }
    fn open_issues_count(&self) -> ExplorerResult<usize> {
        Ok(0)
    }
    fn issues_for_workspace(
        &self,
        _workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> ExplorerResult<Vec<QualityIssue>> {
        let mut out: Vec<QualityIssue> = self
            .by_file
            .values()
            .flat_map(|v| v.iter().cloned())
            .filter(|i| filter.severity.as_deref().is_none_or(|s| i.severity == s))
            .filter(|i| filter.category.as_deref().is_none_or(|c| i.category == c))
            .filter(|i| filter.status.as_deref().is_none_or(|s| i.status == s))
            .filter(|i| match &filter.file_prefix {
                None => true,
                Some(p) => i.file == *p || i.file.starts_with(&format!("{p}/")),
            })
            .collect();
        if let Some(n) = filter.limit {
            out.truncate(n);
        }
        Ok(out)
    }
}

// ============================================================================
// Build a small graph for testing
// ============================================================================

fn build_simple_graph() -> CallGraph {
    let mut g = CallGraph::new();
    let main = g.add_symbol(Symbol::new(
        "main",
        SymbolKind::Function,
        Location::new("main.rs", 1, 0),
    ));
    let helper = g.add_symbol(Symbol::new(
        "helper",
        SymbolKind::Function,
        Location::new("main.rs", 10, 0),
    ));
    g.add_dependency(&main, &helper, DependencyType::Calls)
        .unwrap();
    g
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn build_context_happy_path_all_ports_wired() {
    let graph = build_simple_graph();
    let main_id = graph
        .find_by_name("main")
        .first()
        .map(|s| s.fully_qualified_name().to_string())
        .unwrap();
    let helper_id = graph
        .find_by_name("helper")
        .first()
        .map(|s| s.fully_qualified_name().to_string())
        .unwrap();

    let search = Arc::new(MockSearch::new().with(
        &main_id,
        make_symbol_summary(&main_id, "main", "main.rs", 1),
    ));
    let view = Arc::new(
        MockView::new().with_lens(
            &main_id,
            "lens_find_dead_code",
            "no dead code found",
            vec![("helper is reachable", "Info", 0.9)],
        ),
    );
    let graph_query = Arc::new(MockGraph::new(graph));
    let quality = Arc::new(
        MockQuality::new().with(
            "main.rs",
            vec![issue(1, "warning", "main.rs", "unused parameter `x`", 5)],
        ),
    );

    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_search(search as Arc<dyn SearchService>)
        .with_view(view as Arc<dyn ViewService>)
        .with_graph_query(graph_query as Arc<dyn GraphQueryPort>)
        .with_quality(quality as Arc<dyn QualityRepository>)
        .build();

    let registry = build_registry();
    let result = registry
        .dispatch(
            TOOL_BUILD_CONTEXT,
            &ctx,
            json!({ "object_id": main_id, "depth": 1 }),
        )
        .await;
    let payload = ok_payload(&result);

    // Top-level fields
    assert_eq!(
        payload["json"]["object_id"].as_str(),
        Some(main_id.as_str()),
        "json.object_id should match: {}",
        payload["json"]["object_id"]
    );
    assert_eq!(payload["json"]["label"].as_str(), Some("main"));
    assert_eq!(payload["json"]["object_type"].as_str(), Some("symbol"));
    assert_eq!(payload["summary"].as_str(), Some("# main"));
    assert!(payload["markdown"]["body"]
        .as_str()
        .unwrap()
        .contains("# main"));

    // Lenses
    let lenses = &payload["json"]["lenses"];
    assert!(lenses["requested"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str() == Some("lens_find_dead_code")));
    assert!(lenses["applied"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str() == Some("lens_find_dead_code")));

    // Quality
    let q = &payload["json"]["quality"];
    assert_eq!(q["file"].as_str(), Some("main.rs"));
    assert_eq!(q["total"].as_u64(), Some(1));

    // Graph
    let g = &payload["json"]["graph"];
    assert!(g.is_object(), "graph slice should be present");
    assert!(g["callee_count"].as_u64().unwrap() >= 1);
    let callees = g["callees"].as_array().unwrap();
    assert!(
        callees
            .iter()
            .any(|c| c["id"].as_str().unwrap_or("").contains(&helper_id)),
        "callees should include helper: {callees:?}"
    );

    // Metadata — all sources consulted
    let consulted = payload["metadata"]["sources_consulted"]
        .as_array()
        .unwrap();
    assert!(consulted.iter().any(|v| v.as_str() == Some("search")));
    assert!(consulted.iter().any(|v| v.as_str() == Some("lenses")));
    assert!(consulted.iter().any(|v| v.as_str() == Some("quality")));
    assert!(consulted.iter().any(|v| v.as_str() == Some("graph")));
    assert_eq!(
        payload["metadata"]["sources_skipped"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn build_context_degrades_without_view() {
    let graph = build_simple_graph();
    let main_id = graph
        .find_by_name("main")
        .first()
        .map(|s| s.fully_qualified_name().to_string())
        .unwrap();

    let search = Arc::new(MockSearch::new().with(
        &main_id,
        make_symbol_summary(&main_id, "main", "main.rs", 1),
    ));
    let graph_query = Arc::new(MockGraph::new(graph));

    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_search(search as Arc<dyn SearchService>)
        .with_graph_query(graph_query as Arc<dyn GraphQueryPort>)
        .build();
    // No view, no quality.

    let registry = build_registry();
    let result = registry
        .dispatch(TOOL_BUILD_CONTEXT, &ctx, json!({ "object_id": main_id }))
        .await;
    let payload = ok_payload(&result);

    // Lenses: requested but applied is empty
    assert!(payload["json"]["lenses"]["requested"].is_array());
    assert_eq!(
        payload["json"]["lenses"]["applied"].as_array().unwrap().len(),
        0
    );

    // Quality: null
    assert!(payload["json"]["quality"].is_null());

    // Graph: present
    assert!(payload["json"]["graph"].is_object());

    // sources_skipped mentions "ViewService"
    let skipped = payload["metadata"]["sources_skipped"]
        .as_array()
        .unwrap();
    let skipped_text: Vec<&str> = skipped.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        skipped_text.iter().any(|s| s.contains("ViewService")),
        "sources_skipped should mention ViewService: {skipped_text:?}"
    );
}

#[tokio::test]
async fn build_context_degrades_without_quality() {
    let graph = build_simple_graph();
    let main_id = graph
        .find_by_name("main")
        .first()
        .map(|s| s.fully_qualified_name().to_string())
        .unwrap();

    let search = Arc::new(MockSearch::new().with(
        &main_id,
        make_symbol_summary(&main_id, "main", "main.rs", 1),
    ));

    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_search(search as Arc<dyn SearchService>)
        .build();
    // No view, no quality, no graph_query.

    let registry = build_registry();
    let result = registry
        .dispatch(TOOL_BUILD_CONTEXT, &ctx, json!({ "object_id": main_id }))
        .await;
    let payload = ok_payload(&result);

    assert!(payload["json"]["quality"].is_null());
    assert!(payload["json"]["graph"].is_null());

    let skipped = payload["metadata"]["sources_skipped"]
        .as_array()
        .unwrap();
    let skipped_text: Vec<&str> = skipped.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        skipped_text.iter().any(|s| s.contains("quality")),
        "sources_skipped should mention quality: {skipped_text:?}"
    );
    assert!(
        skipped_text.iter().any(|s| s.contains("graph")),
        "sources_skipped should mention graph: {skipped_text:?}"
    );
}

#[tokio::test]
async fn build_context_rejects_empty_object_id() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_BUILD_CONTEXT, &ctx, json!({ "object_id": "" }))
        .await;
    assert_eq!(
        err_code(&result),
        "missing_required_arg",
        "empty object_id should be rejected"
    );
}

#[tokio::test]
async fn build_context_rejects_missing_object_id() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_BUILD_CONTEXT, &ctx, json!({}))
        .await;
    assert_eq!(
        err_code(&result),
        "invalid_args",
        "missing object_id should yield invalid_args (serde error)"
    );
}

#[tokio::test]
async fn build_context_returns_service_error_when_inspect_fails() {
    let search = Arc::new(MockSearch::new().with_error("downstream DB unreachable"));
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_search(search as Arc<dyn SearchService>)
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_BUILD_CONTEXT,
            &ctx,
            json!({ "object_id": "any" }),
        )
        .await;
    assert_eq!(
        err_code(&result),
        "service_error",
        "service failure should yield service_error"
    );
}

#[tokio::test]
async fn build_context_rejects_when_search_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_BUILD_CONTEXT,
            &ctx,
            json!({ "object_id": "any" }),
        )
        .await;
    assert_eq!(
        err_code(&result),
        "service_unavailable",
        "missing SearchService should yield service_unavailable"
    );
}

#[tokio::test]
async fn build_context_includes_source_stub_when_requested() {
    let graph = build_simple_graph();
    let main_id = graph
        .find_by_name("main")
        .first()
        .map(|s| s.fully_qualified_name().to_string())
        .unwrap();

    let search = Arc::new(MockSearch::new().with(
        &main_id,
        make_symbol_summary(&main_id, "main", "main.rs", 1),
    ));
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_search(search as Arc<dyn SearchService>)
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_BUILD_CONTEXT,
            &ctx,
            json!({ "object_id": main_id, "include_source": true }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["json"]["include_source"].as_bool(), Some(true));
    assert!(payload["markdown"]["body"]
        .as_str()
        .unwrap()
        .contains("## Source (stub)"));
}

#[tokio::test]
async fn build_context_honors_custom_lenses() {
    let graph = build_simple_graph();
    let main_id = graph
        .find_by_name("main")
        .first()
        .map(|s| s.fully_qualified_name().to_string())
        .unwrap();

    let search = Arc::new(MockSearch::new().with(
        &main_id,
        make_symbol_summary(&main_id, "main", "main.rs", 1),
    ));
    let view = Arc::new(
        MockView::new()
            .with_lens(&main_id, "lens_a", "lens A done", vec![])
            .with_lens(&main_id, "lens_b", "lens B done", vec![]),
    );

    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_search(search as Arc<dyn SearchService>)
        .with_view(view as Arc<dyn ViewService>)
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_BUILD_CONTEXT,
            &ctx,
            json!({ "object_id": main_id, "lenses": ["lens_a", "lens_b"] }),
        )
        .await;
    let payload = ok_payload(&result);

    let requested = payload["json"]["lenses"]["requested"]
        .as_array()
        .unwrap();
    assert_eq!(requested.len(), 2);
    let applied = payload["json"]["lenses"]["applied"]
        .as_array()
        .unwrap();
    assert_eq!(applied.len(), 2);
}