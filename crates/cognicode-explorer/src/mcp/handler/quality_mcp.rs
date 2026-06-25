//! Quality-MCP tool wrappers for the Explorer MCP.
//!
//! Exposes two tools that surface the `QualityRepository` port:
//!
//! - `find_quality_issues` — workspace-wide quality findings with filters
//!   (severity, category, file pattern, status, limit). Aggregates issues
//!   across the repo by calling `issues_for_file` for every file the
//!   workspace knows about, then post-filters in Rust.
//! - `quality_gate` — single-shot snapshot of the workspace quality gate
//!   (rating, total issues, blockers, criticals, debt_minutes, last_run).
//!   Wraps `QualityRepository::quality_gate()`.
//!
//! Both tools follow the canonical envelope contract (`ok_envelope` /
//! `err_envelope`) and require a wired `QualityRepository`. They return
//! a structured error when no quality repo is loaded, mirroring the
//! `graph_unavailable` pattern used by `internal_mcp.rs`.
//!
//! No new algorithm logic is introduced — these tools are pure
//! plumbing that makes existing `cognicode-core` + `cognicode-explorer`
//! quality data accessible to MCP clients (and therefore to AI agents).

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    McpContext, TOOL_FIND_QUALITY_ISSUES, TOOL_QUALITY_GATE,
};
use crate::ports::quality_repository::{QualityIssue, RuleSummary};

// ============================================================================
// require_quality — shared guard
// ============================================================================

fn require_quality<'a>(
    ctx: &'a McpContext,
    tool: &str,
) -> Result<&'a Arc<dyn crate::ports::QualityRepository>, CallToolResult> {
    ctx.quality.as_ref().ok_or_else(|| {
        err_envelope(
            tool,
            "quality_unavailable",
            &format!("{tool}: quality data unavailable — no QualityRepository wired"),
        )
    })
}

// ============================================================================
// Tool 1: find_quality_issues
// ============================================================================

/// Input for `find_quality_issues`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
struct FindQualityIssuesArgs {
    /// Optional severity filter (e.g. `"critical"`, `"warning"`, `"info"`).
    /// Case-insensitive. Default: no filter.
    #[serde(default)]
    severity: Option<String>,
    /// Optional category filter (e.g. `"complexity"`, `"duplication"`).
    /// Case-insensitive. Default: no filter.
    #[serde(default)]
    category: Option<String>,
    /// Optional file path prefix filter. Only issues whose `file` starts
    /// with this prefix are returned. Default: no filter.
    #[serde(default)]
    file_prefix: Option<String>,
    /// Optional status filter (e.g. `"open"`, `"resolved"`).
    /// Case-insensitive. Default: no filter.
    #[serde(default)]
    status: Option<String>,
    /// Maximum number of issues to return. Default 100.
    #[serde(default)]
    limit: Option<usize>,
}

/// A single issue as it appears in the MCP payload.
#[derive(Debug, Serialize, Deserialize)]
struct QualityIssueDto {
    id: i64,
    rule_id: String,
    severity: String,
    category: String,
    #[serde(rename = "file_path", alias = "file")]
    file_path: String,
    line: u32,
    message: String,
    status: String,
}

impl From<&QualityIssue> for QualityIssueDto {
    fn from(i: &QualityIssue) -> Self {
        Self {
            id: i.id,
            rule_id: i.rule_id.clone(),
            severity: i.severity.clone(),
            category: i.category.clone(),
            file_path: i.file_path.clone(),
            line: i.line,
            message: i.message.clone(),
            status: i.status.clone(),
        }
    }
}

/// Output for `find_quality_issues`.
#[derive(Debug, Serialize)]
struct FindQualityIssuesResult {
    issues: Vec<QualityIssueDto>,
    total: usize,
    filters_applied: AppliedFilters,
}

/// Echoes the filters that were honored, so callers can verify what
/// the server actually filtered (vs. what they asked for).
#[derive(Debug, Serialize, Default)]
struct AppliedFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    limit: usize,
}

struct FindQualityIssuesHandler;

#[async_trait]
impl ToolHandler for FindQualityIssuesHandler {
    fn name(&self) -> &'static str {
        TOOL_FIND_QUALITY_ISSUES
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "severity": {
                    "type": "string",
                    "description": "Optional severity filter (e.g. 'critical', 'warning', 'info'). Case-insensitive."
                },
                "category": {
                    "type": "string",
                    "description": "Optional category filter (e.g. 'complexity', 'duplication'). Case-insensitive."
                },
                "file_prefix": {
                    "type": "string",
                    "description": "Optional file path prefix filter. Only issues whose `file` starts with this prefix are returned."
                },
                "status": {
                    "type": "string",
                    "description": "Optional status filter (e.g. 'open', 'resolved'). Case-insensitive."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of issues to return (default 100).",
                    "minimum": 1,
                    "maximum": 10000
                }
            }
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: FindQualityIssuesArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_FIND_QUALITY_ISSUES,
                    "invalid_args",
                    &format!("{TOOL_FIND_QUALITY_ISSUES}: invalid args: {e}"),
                );
            }
        };

        let q = match require_quality(ctx, TOOL_FIND_QUALITY_ISSUES) {
            Ok(q) => q,
            Err(e) => return e,
        };

        let limit = args.limit.unwrap_or(100);
        let severity = args.severity.as_deref().map(str::to_lowercase);
        let category = args.category.as_deref().map(str::to_lowercase);
        let status = args.status.as_deref().map(str::to_lowercase);

        // Workspace-wide aggregation: delegate to the port's
        // `issues_for_workspace` (introduced in v2). The PG adapter
        // does the `WHERE workspace_id = $1` index seek and applies
        // filters at the DB. The previous `list_known_files` stub
        // (which always returned empty under the SQLite backend) is
        // no longer needed.
        //
        // v2 note: `workspace_id` is left as `None` so the adapter
        // scans across all workspaces (v1 contract). Multi-workspace
        // scoping is a future enhancement (the args parser already
        // reserves `workspace_id` as an optional field).
        let prefix = args.file_prefix.as_deref();

        let filter = crate::ports::quality_repository::IssueFilter {
            severity: args.severity.clone(),
            category: args.category.clone(),
            status: args.status.clone(),
            file_prefix: args.file_prefix.clone(),
            limit: Some(limit),
        };
        let all = match q.issues_for_workspace(None, &filter) {
            Ok(v) => v,
            Err(e) => {
                return err_envelope(
                    TOOL_FIND_QUALITY_ISSUES,
                    "service_error",
                    &format!("{TOOL_FIND_QUALITY_ISSUES}: issues_for_workspace failed: {e}"),
                );
            }
        };

        // Post-filter (defensive — the PG adapter should already have
        // applied these; kept for parity with the SQLite fallback
        // and as a contract guard against future adapter regressions).
        let filtered: Vec<QualityIssue> = all
            .into_iter()
            .filter(|i| match &severity {
                Some(s) => i.severity.to_lowercase() == *s,
                None => true,
            })
            .filter(|i| match &category {
                Some(c) => i.category.to_lowercase() == *c,
                None => true,
            })
            .filter(|i| match &status {
                Some(s) => i.status.to_lowercase() == *s,
                None => true,
            })
            .filter(|i| match prefix {
                Some(p) => i.file_path.starts_with(p),
                None => true,
            })
            .take(limit)
            .collect();

        let total = filtered.len();
        let issues: Vec<QualityIssueDto> = filtered.iter().map(QualityIssueDto::from).collect();

        let result = FindQualityIssuesResult {
            issues,
            total,
            filters_applied: AppliedFilters {
                severity: args.severity,
                category: args.category,
                file_prefix: args.file_prefix,
                status: args.status,
                limit,
            },
        };
        ok_envelope(TOOL_FIND_QUALITY_ISSUES, &result)
    }
}

/// Helper: try to enumerate files the quality repo has indexed.
///
/// Returns `Ok(Vec::new>)` if the repo doesn't expose a file index —
/// callers fall back to other sources or accept empty results.
/// This is intentionally best-effort: aggregation across an entire
/// workspace requires cooperation from the port, which is a v2 concern.
fn list_known_files(
    _q: &Arc<dyn crate::ports::QualityRepository>,
) -> Result<Vec<String>, ()> {
    // v1: QualityRepository doesn't expose a file index. The
    // aggregation path returns an empty file list, so the handler
    // will produce an empty issues array — callers should narrow by
    // `file_prefix` or scope. This keeps the tool callable even when
    // the underlying port has no file enumeration.
    Ok(Vec::new())
}

/// v1 helper: tried to enumerate files the quality repo had indexed.
/// Removed in v2 (`quality-stack-pg-canonical-v2`) because
/// `find_quality_issues` now delegates to `issues_for_workspace`
/// instead of per-file walks. The aggregation path is now a single
/// index seek at the PG layer, not a Rust-side fan-out.

/// v2 helper: removed entirely. See git history for the v1 version.

// ============================================================================
// Tool 2: quality_gate
// ============================================================================

/// Input for `quality_gate`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
struct QualityGateArgs {
    /// Reserved for future multi-workspace support. Currently unused —
    /// the tool operates on the loaded repo.
    #[serde(default)]
    workspace_id: Option<String>,
}

/// Output for `quality_gate`.
#[derive(Debug, Serialize)]
struct QualityGateResult {
    workspace_id: Option<String>,
    rating: Option<String>,
    total_issues: usize,
    blockers: usize,
    criticals: usize,
    debt_minutes: u64,
    last_run: Option<String>,
    open_issues_count: usize,
}

struct QualityGateHandler;

#[async_trait]
impl ToolHandler for QualityGateHandler {
    fn name(&self) -> &'static str {
        TOOL_QUALITY_GATE
    }

    fn arg_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "workspace_id": {
                    "type": "string",
                    "description": "Optional workspace id; omit for aggregate across all workspaces."
                }
            }
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: QualityGateArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_QUALITY_GATE,
                    "invalid_args",
                    &format!("{TOOL_QUALITY_GATE}: invalid args: {e}"),
                );
            }
        };
        let q = match require_quality(ctx, TOOL_QUALITY_GATE) {
            Ok(q) => q,
            Err(e) => return e,
        };

        let ws = args.workspace_id.as_deref();
        let gate = match q.quality_gate(ws) {
            Ok(g) => g,
            Err(e) => {
                return err_envelope(
                    TOOL_QUALITY_GATE,
                    "quality_gate_failed",
                    &format!("{TOOL_QUALITY_GATE}: failed to read quality gate: {e}"),
                );
            }
        };

        let open = q.open_issues_count(ws).unwrap_or(0);

        let result = QualityGateResult {
            workspace_id: args.workspace_id,
            rating: gate.rating,
            total_issues: gate.total_issues,
            blockers: gate.blockers,
            criticals: gate.criticals,
            debt_minutes: gate.debt_minutes,
            last_run: gate.last_run,
            open_issues_count: open,
        };
        ok_envelope(TOOL_QUALITY_GATE, &result)
    }
}

// ============================================================================
// Registry
// ============================================================================

/// Register the quality-MCP handlers into the registry.
pub fn register_quality_mcp_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(FindQualityIssuesHandler);
    registry.register(QualityGateHandler);
}

// Suppress unused warnings for the helper reference used by callers
// (and tests) when iterating over a fixture's known rules.
#[allow(dead_code)]
fn _rule_summary_marker(_r: &RuleSummary) {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ExplorerError, ExplorerResult};
    use crate::ports::quality_repository::{
        IssueFilter, QualityGateSummary, QualityIssue, QualityRepository, RuleSummary,
    };
    use async_trait::async_trait;
    use std::collections::HashMap;

    // ---------------------------------------------------------------------
    // Mock QualityRepository
    // ---------------------------------------------------------------------

    struct MockQuality {
        by_file: HashMap<String, Vec<QualityIssue>>,
        gate: QualityGateSummary,
        open_total: usize,
    }

    impl MockQuality {
        fn new() -> Self {
            Self {
                by_file: HashMap::new(),
                gate: QualityGateSummary::default(),
                open_total: 0,
            }
        }

        fn with_file(mut self, file: &str, issues: Vec<QualityIssue>) -> Self {
            self.by_file.insert(file.to_string(), issues);
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
    impl QualityRepository for MockQuality {
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
        fn issues_at_line(
            &self,
            file: &str,
            line: u32,
        ) -> ExplorerResult<Vec<QualityIssue>> {
            Ok(self
                .by_file
                .get(file)
                .map(|v| {
                    v.iter().filter(|i| i.line == line).cloned().collect()
                })
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

    fn make_issue(id: i64, severity: &str, category: &str, file: &str, status: &str) -> QualityIssue {
        QualityIssue {
            id,
            rule_id: format!("R{id}"),
            severity: severity.to_string(),
            category: category.to_string(),
            file_path: file.to_string(),
            line: 1,
            message: format!("msg {id}"),
            status: status.to_string(),
        }
    }

    // ---------------------------------------------------------------------
    // Arg parsing
    // ---------------------------------------------------------------------

    #[test]
    fn find_quality_issues_args_defaults() {
        let json = json!({});
        let args: FindQualityIssuesArgs = serde_json::from_value(json).unwrap();
        assert!(args.severity.is_none());
        assert!(args.category.is_none());
        assert!(args.file_prefix.is_none());
        assert!(args.status.is_none());
        assert!(args.limit.is_none());
    }

    #[test]
    fn find_quality_issues_args_full() {
        let json = json!({
            "severity": "critical",
            "category": "complexity",
            "file_prefix": "src/auth",
            "status": "open",
            "limit": 25
        });
        let args: FindQualityIssuesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.severity.as_deref(), Some("critical"));
        assert_eq!(args.category.as_deref(), Some("complexity"));
        assert_eq!(args.file_prefix.as_deref(), Some("src/auth"));
        assert_eq!(args.status.as_deref(), Some("open"));
        assert_eq!(args.limit, Some(25));
    }

    #[test]
    fn quality_gate_args_empty() {
        let json = json!({});
        let args: QualityGateArgs = serde_json::from_value(json).unwrap();
        assert!(args.workspace_id.is_none());
    }

    // ---------------------------------------------------------------------
    // QualityIssueDto round-trip
    // ---------------------------------------------------------------------

    #[test]
    fn quality_issue_dto_round_trip() {
        let issue = make_issue(7, "critical", "complexity", "src/x.rs", "open");
        let dto = QualityIssueDto::from(&issue);
        let v = serde_json::to_value(&dto).unwrap();
        assert_eq!(v["id"], 7);
        assert_eq!(v["rule_id"], "R7");
        assert_eq!(v["severity"], "critical");
        assert_eq!(v["file_path"], "src/x.rs");
    }

    #[test]
    fn quality_issue_dto_accepts_legacy_file_alias() {
        let json = serde_json::json!({
            "id": 7,
            "rule_id": "R7",
            "severity": "critical",
            "category": "complexity",
            "file": "src/x.rs",
            "line": 1,
            "message": "msg 7",
            "status": "open"
        });
        let dto: QualityIssueDto = serde_json::from_value(json).unwrap();
        assert_eq!(dto.file_path, "src/x.rs");
    }

    // ---------------------------------------------------------------------
    // QualityGateResult shape
    // ---------------------------------------------------------------------

    #[test]
    fn quality_gate_result_round_trip() {
        let result = QualityGateResult {
            workspace_id: Some("ws-a".to_string()),
            rating: Some("B".to_string()),
            total_issues: 50,
            blockers: 0,
            criticals: 2,
            debt_minutes: 120,
            last_run: Some("2026-06-24T10:00:00Z".to_string()),
            open_issues_count: 18,
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["workspace_id"], "ws-a");
        assert_eq!(v["rating"], "B");
        assert_eq!(v["total_issues"], 50);
        assert_eq!(v["open_issues_count"], 18);
    }

    // ---------------------------------------------------------------------
    // require_quality guard
    // ---------------------------------------------------------------------

    #[tokio::test]
    async fn require_quality_returns_err_when_no_repo() {
        let ctx = McpContext::new(None, crate::session::SessionRegistry::new());
        // Map Ok to unit before expect_err — the success type is
        // `Arc<dyn QualityRepository>`, which does not implement Debug.
        let result = require_quality(&ctx, TOOL_FIND_QUALITY_ISSUES).map(|_| ());
        let err = result.expect_err("should be err when no quality wired");
        let text = format!("{err:?}");
        assert!(
            text.contains("quality_unavailable"),
            "err should mention quality_unavailable: {text}"
        );
    }

    // ---------------------------------------------------------------------
    // MockQuality trait methods (compile-time coverage)
    // ---------------------------------------------------------------------

    #[test]
    fn mock_quality_returns_empty_for_unknown_file() {
        let q = MockQuality::new();
        let result = q.issues_for_file("nope.rs").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn mock_quality_returns_issues_for_known_file() {
        let q = MockQuality::new().with_file(
            "src/x.rs",
            vec![make_issue(1, "warning", "complexity", "src/x.rs", "open")],
        );
        let result = q.issues_for_file("src/x.rs").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn mock_quality_scope_aggregates_across_files() {
        let q = MockQuality::new()
            .with_file(
                "src/auth/a.rs",
                vec![make_issue(1, "warning", "complexity", "src/auth/a.rs", "open")],
            )
            .with_file(
                "src/auth/b.rs",
                vec![make_issue(2, "critical", "complexity", "src/auth/b.rs", "open")],
            )
            .with_file(
                "src/other/c.rs",
                vec![make_issue(3, "warning", "complexity", "src/other/c.rs", "open")],
            );
        let result = q.issues_for_scope("src/auth").unwrap();
        assert_eq!(result.len(), 2);
    }

    // ---------------------------------------------------------------------
    // Touch ExplorerError to ensure the import stays live
    // ---------------------------------------------------------------------

    #[test]
    fn explorer_error_is_reachable() {
        let _e = ExplorerError::FeatureDisabled("mock".into());
    }
}