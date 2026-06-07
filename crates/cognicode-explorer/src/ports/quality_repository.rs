//! Domain port for the quality lens.
//!
//! Surfaces the `issues` table behind the same `Send + Sync` trait shape
//! the other explorer ports use. The schema is owned by `cognicode-db`;
//! this crate reads from it via short-lived `rusqlite::Connection`s and
//! never assumes writes — quality data is owned by the quality agent.
//!
//! All methods are read-only and must degrade gracefully when the
//! underlying DB is missing (return empty / zero, never an error).

use crate::error::ExplorerResult;

/// A single quality finding, lifted from the `issues` table.
///
/// `file` is the struct-side name (the column is `file_path` in the
/// schema); the adapter does the column→field mapping so callers do
/// not have to know the SQLite column name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityIssue {
    pub id: i64,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    pub file: String,
    pub line: u32,
    pub message: String,
    pub status: String,
}

/// Compact summary of a single rule — its open count and a short
/// description. The current schema does not store rule metadata, so
/// the description defaults to the rule id itself.
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
}
