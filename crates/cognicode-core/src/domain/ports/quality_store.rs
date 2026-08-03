//! `QualityStore` — domain port for the quality lens.
//!
//! Surfaces the `issues`, `baselines`, and `rules` tables behind a
//! `Send + Sync` trait shape. The PG schema is owned by
//! `cognicode-core`'s persistence layer (see migration `m0011_quality.sql`);
//! the in-crate adapter `PostgresQualityStore` reads from it.
//!
//! # Read-only contract (preserve QualityRepository legacy)
//!
//! The 8 read methods must degrade gracefully when the underlying DB
//! is missing (return empty / zero, never an error). Errors are
//! reserved for actual I/O / parse failures (e.g. a corrupted DB
//! file).
//!
//! # Write contract (preserve QualityWritePort legacy)
//!
//! The 2 write methods DO error on I/O failure — a failed write is a
//! real error, not an empty result.
//!
//! # Phase 0 origin
//!
//! This trait was relocated from `cognicode-explorer::ports::quality_repository`
//! per ADR-028 to make the port importable from `cognicode_core` only.
//! The original split into two traits (`QualityRepository` +
//! `QualityWritePort`) is collapsed into a single 10-method surface
//! here so adapters ship one impl block; callers that need the
//! read-only invariant can use a `&dyn QualityStore` projection that
//! only exercises the read methods (see `WriteOnly` projection in a
//! future ADR if needed).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Insert DTO — mirror of [`QualityIssue`] minus `id` (DB-assigned), plus
/// explicit `workspace_id` (the read struct carries no workspace field
/// because reads are scoped by method arg, not by row ownership).
#[derive(Debug, Clone, Deserialize)]
pub struct NewIssue {
    pub workspace_id: String,
    pub rule_id: String,
    pub severity: String,
    pub category: String,
    pub file_path: String,
    pub line: u32,
    pub message: String,
    pub status: String,
}

/// Summary of an upsert operation — counts of inserted vs updated rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpsertSummary {
    pub inserted: usize,
    pub updated: usize,
}

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
#[derive(Debug, Clone, Default)]
pub struct IssueFilter {
    pub severity: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    /// Boundary-aware: `scope = "src"` does not match `src_extra.rs`.
    pub file_prefix: Option<String>,
    pub limit: Option<usize>,
}

/// Errors returned by [`QualityStore`] write operations.
///
/// Read methods do not return errors in graceful mode (they return
/// empty results on missing DB); only write operations surface
/// [`Store`] / [`Conflict`].
#[derive(Debug, Error)]
pub enum QualityError {
    #[error("quality store error: {0}")]
    Store(String),
    #[error("quality write conflict: {0}")]
    Conflict(String),
    #[error("quality issue not found: {0}")]
    NotFound(i64),
}

/// Read + write port for quality findings, rules, and gate state.
///
/// Implementations must be `Send + Sync` and `Arc`-friendly.
///
/// **Read methods (8)**: gracefully degrade when the underlying DB is
/// missing or empty — returning empty vectors or zero counts instead
/// of errors.
/// **Write methods (2)**: surface I/O errors directly. A failed write
/// is a real error, not an empty result.
pub trait QualityStore: Send + Sync {
    /// Every issue whose `file_path` matches `file` exactly.
    fn issues_for_file(&self, file: &str) -> Result<Vec<QualityIssue>, QualityError>;

    /// Every issue whose `file_path` is `scope` or starts with `scope/`.
    /// Boundary-aware: `scope = "src"` does not match `src_extra.rs`.
    fn issues_for_scope(&self, scope_prefix: &str) -> Result<Vec<QualityIssue>, QualityError>;

    /// Every issue at exactly `(file, line)`.
    fn issues_at_line(&self, file: &str, line: u32) -> Result<Vec<QualityIssue>, QualityError>;

    /// Look up a single issue by its primary key. Returns `Ok(None)`
    /// when the id does not exist.
    fn issue_by_id(&self, id: i64) -> Result<Option<QualityIssue>, QualityError>;

    /// Compact summary of a single rule (open count + description).
    fn rule_summary(&self, rule_id: &str) -> Result<RuleSummary, QualityError>;

    /// Latest quality gate snapshot for the workspace (or all).
    fn quality_gate(&self, workspace_id: Option<&str>) -> Result<QualityGateSummary, QualityError>;

    /// Total count of issues with `status = 'open'`.
    fn open_issues_count(&self, workspace_id: Option<&str>) -> Result<usize, QualityError>;

    /// Workspace-wide issue scan with optional filters.
    fn issues_for_workspace(
        &self,
        workspace_id: Option<&str>,
        filter: &IssueFilter,
    ) -> Result<Vec<QualityIssue>, QualityError>;

    // --- Write methods ---

    /// Upsert a batch of issues; returns insert/update counts.
    fn insert_issues(&self, issues: &[NewIssue]) -> Result<UpsertSummary, QualityError>;

    /// Delete a single issue by `(workspace_id, rule_id, file_path, line)`.
    /// Returns `Ok(true)` when a row was deleted, `Ok(false)` when no
    /// matching row existed.
    fn delete_issue(
        &self,
        workspace_id: &str,
        rule_id: &str,
        file_path: &str,
        line: u32,
    ) -> Result<bool, QualityError>;
}

// =============================================================================
// PostgresQualityStore adapter
// =============================================================================


