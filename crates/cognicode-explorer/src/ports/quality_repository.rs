//! Domain port for the quality lens.
//!
//! Surfaces the `issues`, `baselines`, and `rules` tables behind the
//! same `Send + Sync` trait shape the other explorer ports use. The
//! schema is owned by `cognicode-core`'s PostgreSQL persistence layer
//! (see `cognicode-core/src/infrastructure/persistence/m0011_quality.sql`);
//! this crate reads from it via the `PostgresQualityRepository` adapter
//! (introduced alongside PR #54 of the postgres-canonical quality
//! stack rebuild). Never assumes writes — quality data is owned by the
//! quality agent.
//!
//! **History**: a SQLite-backed version of this port lived in a
//! workspace-internal persistence layer that was retired during the
//! Graph Intelligence v2 cleanup. The current canonical store is
//! PostgreSQL (verify report archived as engram obs #1829).
//!
//! All methods are read-only and must degrade gracefully when the
//! underlying DB is missing (return empty / zero, never an error).

use crate::error::ExplorerResult;
use serde::Serialize;

/// A single quality finding, lifted from the `issues` table.
///
/// `file_path` is the struct-side name, matching the DB column
/// `issues.file_path`. The adapter does the column→field mapping so
/// callers do not have to know the SQL column name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualityIssue {
    pub id: i64,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    #[serde(rename = "file_path", alias = "file")]
    pub file_path: String,
    pub line: u32,
    pub message: String,
    pub status: String,
}

/// Compact summary of a single rule — its open count and a short
/// description. The `rules` table stores description + category; the
/// description defaults to the rule id when empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSummary {
    pub rule_id: String,
    pub description: String,
    pub open_count: usize,
}

/// Quality gate snapshot — the latest `baselines` row, plus a current
/// open-issue count. Used by file/scope views to surface a "score card"
/// without forcing the caller to issue multiple queries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QualityGateSummary {
    pub rating: Option<String>,
    pub total_issues: usize,
    pub blockers: usize,
    pub criticals: usize,
    pub debt_minutes: u64,
    pub last_run: Option<String>,
}

/// Optional filter applied to `issues_for_workspace`. All fields are
/// `AND`-combined; `None` means "no filter on this dimension".
///
/// Introduced alongside `issues_for_workspace()` (PR #53) so the
/// `find_quality_issues` MCP tool can do workspace-wide aggregations
/// without enumerating files first. v1 of the port returned an empty
/// aggregation for workspace-wide queries because the SQLite backend
/// had no file index; the new method delegates the workspace scan to
/// the PG backend, where `idx_issues_workspace_status` makes the
/// `WHERE workspace_id = $1` filter an index seek.
#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    /// Optional severity filter (e.g. `"critical"`).
    pub severity: Option<String>,
    /// Optional category filter (e.g. `"complexity"`).
    pub category: Option<String>,
    /// Optional status filter (e.g. `"open"`).
    pub status: Option<String>,
    /// Optional file-path prefix filter. Boundary-aware: `scope = "src"`
    /// does not match `src_extra.rs`.
    pub file_prefix: Option<String>,
    /// Maximum number of issues to return. None = no limit (caller
    /// is expected to set a reasonable bound).
    pub limit: Option<usize>,
}

/// Read-only port for quality findings, rules, and gate state.
///
/// All methods must be safe to call on an empty / missing DB: they
/// return empty vectors or zero counts instead of errors. Errors are
/// reserved for actual I/O / parse failures (e.g. a corrupted DB file).
pub trait QualityRepository: Send + Sync {
    /// Every issue whose `file_path` matches `file` exactly.
    fn issues_for_file(&self, file: &str) -> ExplorerResult<Vec<QualityIssue>>;

    /// Every issue whose `file_path` is `scope` or starts with `scope/`.
    /// Boundary-aware: `scope = "src"` does not match `src_extra.rs`.
    fn issues_for_scope(&self, scope_prefix: &str) -> ExplorerResult<Vec<QualityIssue>>;

    /// Every issue at exactly `(file, line)`. Used by the symbol quality
    /// view to surface findings on the symbol's declaration line.
    fn issues_at_line(&self, file: &str, line: u32) -> ExplorerResult<Vec<QualityIssue>>;

    /// Look up a single issue by its primary key. Returns `None` when
    /// the id does not exist.
    fn issue_by_id(&self, id: i64) -> ExplorerResult<Option<QualityIssue>>;

    /// Compact summary of a single rule (open count + description).
    /// `open_count` is 0 when the rule has no open issues.
    fn rule_summary(&self, rule_id: &str) -> ExplorerResult<RuleSummary>;

    /// Latest quality gate snapshot. Returns the default (all zeros,
    /// `None` rating) when no baseline has been recorded yet.
    fn quality_gate(&self) -> ExplorerResult<QualityGateSummary>;

    /// Total count of issues with `status = 'open'`. Used by the
    /// workspace summary's "open quality issues" indicator.
    fn open_issues_count(&self) -> ExplorerResult<usize>;

    /// Workspace-wide issue scan with optional filters.
    ///
    /// Backed by the PG `issues` table; the `workspace_id` column is
    /// the dominant predicate (`idx_issues_workspace_status`) so the
    /// first-class query is a single index seek + filter chain.
    ///
    /// When `workspace_id` is `None`, the scan returns issues across
    /// ALL workspaces — used by the v1 `find_quality_issues` handler
    /// when the caller has not scoped to a specific workspace.
    /// Adapters should treat `None` as "no workspace predicate".
    ///
    /// The `limit` field in `filter` is the only mandatory bound;
    /// adapters may return fewer rows if the underlying query plan
    /// decides that's faster, but MUST honor `filter.limit` as an
    /// upper bound.
    fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> ExplorerResult<Vec<QualityIssue>>;
}
