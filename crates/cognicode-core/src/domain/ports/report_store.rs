//! Domain port for the `graph_reports` table.
//!
//! `graph_reports` stores per-revision summaries produced by the
//! pipeline's Report stage. Used by the `graph_diff` and `graph_timeline`
//! MCP tools.
//!
//! This port exists so application code can read graph reports
//! without depending on `PostgresRepository` concrete types.
//!
//! # ADR-028 §3 contract
//!
//! - `save_report(ws, report)` — append a new report row for `ws`
//! - `latest_report(ws)` — most-recent report for `ws`, or `None`
//! - `reports_for_workspace(ws)` — every report for `ws`, ordered
//!   newest-first (bounded to a sensible default range; the underlying
//!   adapter may filter by `created_at > now() - interval` if the
//!   table grows unbounded)

use async_trait::async_trait;

/// Domain type returned by [`ReportStore`] — the subset of
/// `graph_reports` columns that callers actually use.
#[derive(Debug, Clone)]
pub struct ReportSummary {
    pub id: String,
    pub workspace_id: String,
    pub created_at: String,
    pub report: serde_json::Value,
    pub symbol_count: i32,
    pub edge_count: i32,
    pub health_score: Option<f32>,
}

/// Port for graph report persistence.
#[async_trait]
pub trait ReportStore: Send + Sync {
    /// Persist a new report row for `workspace_id`. The `id` field of
    /// `report` is client-suggested; the store preserves it on insert.
    ///
    /// **PHASE 0: stub**. The full SQL (INSERT INTO graph_reports with
    /// ON CONFLICT DO UPDATE) lands once the ingest state's report
    /// publish step is wired. Until then this method is a no-op +
    /// `Ok(())` placeholder so callers can compile against the
    /// ADR-028 contract.
    async fn save_report(
        &self,
        workspace_id: &str,
        report: &ReportSummary,
    ) -> Result<(), ReportError>;

    /// Load the most recent `graph_reports` row for a workspace.
    ///
    /// Returns `Ok(None)` when no report exists yet.
    async fn latest_report(&self, workspace_id: &str)
    -> Result<Option<ReportSummary>, ReportError>;

    /// Load every `graph_reports` row for a workspace, ordered by
    /// `created_at DESC` (newest-first).
    ///
    /// Returns an empty `Vec` when the workspace has no reports.
    /// Never an error — empty list is a valid state.
    async fn reports_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ReportSummary>, ReportError>;
}

/// Error type for [`ReportStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("graph report store error: {0}")]
    Store(String),
}


