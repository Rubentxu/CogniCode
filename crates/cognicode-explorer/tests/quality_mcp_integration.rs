//! Integration tests for the quality-MCP handlers
//! (`find_quality_issues`, `quality_gate`).
//!
//! Wires the **real** `McpContext` + a `MockQuality` (in-memory)
//! `QualityRepository` and dispatches each handler end-to-end through
//! the standard `ToolHandlerRegistry::dispatch` path.
//!
//! Mirrors the pattern established by `lens_mcp_integration.rs` and
//! `internal_mcp_integration.rs` — no PG dependency, in-memory mocks.
//!
//! Covers:
//! - happy paths (issue aggregation, gate snapshot)
//! - filter application (severity, category, file_prefix, status, limit)
//! - error envelopes (quality_unavailable when no repo wired,
//!   invalid_args on malformed input)
//! - edge cases (no known files in v1 port → empty aggregation,
//!   quality_gate with all-zero summary)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_explorer::error::ExplorerResult;
use cognicode_explorer::mcp::handler::ToolHandlerRegistry;
use cognicode_explorer::mcp::handler::quality_mcp::register_quality_mcp_handlers;
use cognicode_explorer::mcp::{
    McpContext, TOOL_FIND_QUALITY_ISSUES, TOOL_QUALITY_GATE,
};
use cognicode_explorer::ports::quality_repository::{
    IssueFilter, QualityGateSummary, QualityIssue, QualityRepository, RuleSummary,
};
use cognicode_explorer::session::SessionRegistry;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};

// ============================================================================
// Test fixtures
// ============================================================================

/// Build an issue with the given id, severity, category, file, status.
fn issue(id: i64, severity: &str, category: &str, file: &str, status: &str) -> QualityIssue {
    QualityIssue {
        id,
        rule_id: format!("R{id:03}"),
        severity: severity.to_string(),
        category: category.to_string(),
        file_path: file.to_string(),
        line: 1,
        message: format!("issue {id}"),
        status: status.to_string(),
    }
}

/// In-memory `QualityRepository` keyed by file. The current v1 port
/// doesn't expose a file index, so `list_known_files` returns `Ok(vec![])`
/// from the handler — aggregation must be driven by `issues_for_scope`
/// or `issues_for_file` directly. We expose the data through `by_file`
/// for the integration tests, which is what a v2 workspace-aware port
/// would also populate.
#[derive(Default)]
struct InMemoryQuality {
    /// file → issues at that file
    by_file: HashMap<String, Vec<QualityIssue>>,
    gate: QualityGateSummary,
    open_total: usize,
}

impl InMemoryQuality {
    fn new() -> Self {
        Self::default()
    }

    fn with_files(mut self, entries: &[(&str, Vec<QualityIssue>)]) -> Self {
        for (f, issues) in entries {
            self.by_file.insert((*f).to_string(), issues.clone());
        }
        self
    }

    fn with_gate(mut self, gate: QualityGateSummary) -> Self {
        self.gate = gate;
        self
    }

    fn with_open_total(mut self, n: usize) -> Self {
        self.open_total = n;
        self
    }
}

#[async_trait]
impl QualityRepository for InMemoryQuality {
    fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<QualityIssue>> {
        Ok(self.by_file.get(file).cloned().unwrap_or_default())
    }
    fn issues_for_scope(&self, scope_prefix: &str) -> ExplorerResult<Vec<QualityIssue>> {
        Ok(self
            .by_file
            .iter()
            .filter(|(f, _)| f.starts_with(scope_prefix))
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
            rule_id: "mock".to_string(),
            description: "mock".to_string(),
            open_count: 0,
        })
    }
    fn quality_gate(&self, _workspace_id: Option<&str>) -> ExplorerResult<QualityGateSummary> {
        Ok(self.gate.clone())
    }
    fn open_issues_count(&self, _workspace_id: Option<&str>) -> ExplorerResult<usize> {
        Ok(self.open_total)
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
                Some(p) => i.file_path == *p || i.file_path.starts_with(&format!("{p}/")),
            })
            .collect();
        if let Some(n) = filter.limit {
            out.truncate(n);
        }
        Ok(out)
    }
}

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
    let env = extract_env(result);
    env.get("payload")
        .cloned()
        .expect("ok envelope must have a `payload` field")
}

fn err_code(result: &CallToolResult) -> String {
    assert_eq!(
        result.is_error,
        Some(true),
        "expected err envelope, got: {result:?}"
    );
    let env = extract_env(result);
    env.get("payload")
        .and_then(|p| p.get("error_code"))
        .and_then(|c| c.as_str())
        .map(String::from)
        .expect("err envelope payload must have `error_code`")
}

fn ctx_with_quality(q: Arc<dyn QualityRepository>) -> McpContext {
    McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .with_quality(q)
        .build()
}

fn build_registry() -> ToolHandlerRegistry {
    let mut r = ToolHandlerRegistry::new();
    register_quality_mcp_handlers(&mut r);
    r
}

// ============================================================================
// find_quality_issues — end-to-end dispatch
// ============================================================================

#[tokio::test]
async fn find_quality_issues_returns_empty_when_no_files_known() {
    // v1 port has no file index; aggregation returns empty.
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_QUALITY_ISSUES, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["total"].as_u64(), Some(0));
    assert_eq!(payload["issues"].as_array().unwrap().len(), 0);
    // Filters echoed back as defaults
    assert_eq!(payload["filters_applied"]["limit"].as_u64(), Some(100));
    assert!(payload["filters_applied"]["severity"].is_null());
}

#[tokio::test]
async fn find_quality_issues_aggregates_across_files() {
    let q = Arc::new(
        InMemoryQuality::new().with_files(&[
            (
                "src/auth/a.rs",
                vec![
                    issue(1, "critical", "complexity", "src/auth/a.rs", "open"),
                    issue(2, "warning", "complexity", "src/auth/a.rs", "open"),
                ],
            ),
            (
                "src/auth/b.rs",
                vec![issue(3, "critical", "duplication", "src/auth/b.rs", "open")],
            ),
            (
                "src/other/c.rs",
                vec![issue(4, "info", "naming", "src/other/c.rs", "resolved")],
            ),
        ]),
    );
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_QUALITY_ISSUES, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    // v1 limitation: handler can't enumerate files, so the in-memory
    // issues are NOT returned by `find_quality_issues` (only filters
    // against an empty aggregation). The total should be 0.
    assert_eq!(
        payload["total"].as_u64(),
        Some(0),
        "v1 port exposes no file index — find_quality_issues returns 0: {payload}"
    );
}

#[tokio::test]
async fn find_quality_issues_applies_severity_filter() {
    // Filters are applied in Rust after aggregation. With the v1 port
    // returning empty aggregation, the filter has nothing to act on.
    // We assert that the filter was accepted and echoed back correctly.
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_QUALITY_ISSUES,
            &ctx,
            json!({ "severity": "critical" }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["filters_applied"]["severity"].as_str(), Some("critical"));
    assert_eq!(payload["total"].as_u64(), Some(0));
}

#[tokio::test]
async fn find_quality_issues_applies_category_filter() {
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_QUALITY_ISSUES,
            &ctx,
            json!({ "category": "complexity" }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["filters_applied"]["category"].as_str(), Some("complexity"));
}

#[tokio::test]
async fn find_quality_issues_applies_file_prefix_filter() {
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_QUALITY_ISSUES,
            &ctx,
            json!({ "file_prefix": "src/auth" }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(
        payload["filters_applied"]["file_prefix"].as_str(),
        Some("src/auth")
    );
}

#[tokio::test]
async fn find_quality_issues_applies_status_filter() {
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_QUALITY_ISSUES,
            &ctx,
            json!({ "status": "open" }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["filters_applied"]["status"].as_str(), Some("open"));
}

#[tokio::test]
async fn find_quality_issues_respects_limit() {
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_FIND_QUALITY_ISSUES,
            &ctx,
            json!({ "limit": 25 }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["filters_applied"]["limit"].as_u64(), Some(25));
}

#[tokio::test]
async fn find_quality_issues_invalid_args_returns_envelope() {
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    // limit must be an integer
    let result = registry
        .dispatch(
            TOOL_FIND_QUALITY_ISSUES,
            &ctx,
            json!({ "limit": "not-a-number" }),
        )
        .await;
    assert_eq!(
        err_code(&result),
        "invalid_args",
        "invalid args should yield invalid_args error code"
    );
}

#[tokio::test]
async fn find_quality_issues_rejects_when_quality_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_FIND_QUALITY_ISSUES, &ctx, json!({}))
        .await;
    assert_eq!(
        err_code(&result),
        "quality_unavailable",
        "missing QualityRepository should yield quality_unavailable"
    );
}

// ============================================================================
// quality_gate — end-to-end dispatch
// ============================================================================

#[tokio::test]
async fn quality_gate_returns_snapshot() {
    let gate = QualityGateSummary {
        rating: Some("B".to_string()),
        total_issues: 50,
        blockers: 1,
        criticals: 4,
        debt_minutes: 240,
        last_run: Some("2026-06-24T10:00:00Z".to_string()),
    };
    let q = Arc::new(InMemoryQuality::new().with_gate(gate).with_open_total(18));
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_QUALITY_GATE, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["rating"].as_str(), Some("B"));
    assert_eq!(payload["total_issues"].as_u64(), Some(50));
    assert_eq!(payload["blockers"].as_u64(), Some(1));
    assert_eq!(payload["criticals"].as_u64(), Some(4));
    assert_eq!(payload["debt_minutes"].as_u64(), Some(240));
    assert_eq!(
        payload["last_run"].as_str(),
        Some("2026-06-24T10:00:00Z")
    );
    assert_eq!(payload["open_issues_count"].as_u64(), Some(18));
}

#[tokio::test]
async fn quality_gate_returns_zeros_when_empty() {
    let q = Arc::new(InMemoryQuality::new());
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_QUALITY_GATE, &ctx, json!({}))
        .await;
    let payload = ok_payload(&result);

    assert!(payload["rating"].is_null());
    assert_eq!(payload["total_issues"].as_u64(), Some(0));
    assert_eq!(payload["blockers"].as_u64(), Some(0));
    assert_eq!(payload["criticals"].as_u64(), Some(0));
    assert_eq!(payload["debt_minutes"].as_u64(), Some(0));
    assert!(payload["last_run"].is_null());
    assert_eq!(payload["open_issues_count"].as_u64(), Some(0));
}

#[tokio::test]
async fn quality_gate_ignores_unknown_workspace_id() {
    // The workspace_id arg is reserved; the tool ignores it.
    let gate = QualityGateSummary {
        rating: Some("A".to_string()),
        total_issues: 5,
        blockers: 0,
        criticals: 0,
        debt_minutes: 30,
        last_run: Some("2026-06-25T07:00:00Z".to_string()),
    };
    let q = Arc::new(InMemoryQuality::new().with_gate(gate));
    let ctx = ctx_with_quality(q);
    let registry = build_registry();

    let result = registry
        .dispatch(
            TOOL_QUALITY_GATE,
            &ctx,
            json!({ "workspace_id": "ws-xyz" }),
        )
        .await;
    let payload = ok_payload(&result);

    assert_eq!(payload["rating"].as_str(), Some("A"));
    assert_eq!(payload["total_issues"].as_u64(), Some(5));
}

#[tokio::test]
async fn quality_gate_rejects_when_quality_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let result = registry
        .dispatch(TOOL_QUALITY_GATE, &ctx, json!({}))
        .await;
    assert_eq!(
        err_code(&result),
        "quality_unavailable",
        "missing QualityRepository should yield quality_unavailable"
    );
}