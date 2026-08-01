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

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use super::{ReportError, ReportStore, ReportSummary};
    use crate::infrastructure::persistence::PostgresRepository;
    use async_trait::async_trait;
    use std::sync::Arc;

    /// Adapter that delegates every [`ReportStore`] method to a
    /// [`PostgresRepository`]. Stores an `Arc` so the adapter can
    /// outlive a single call site and be shared across threads.
    #[cfg(feature = "postgres")]
    pub struct PostgresReportStore {
        repo: Arc<PostgresRepository>,
    }

    #[cfg(feature = "postgres")]
    impl PostgresReportStore {
        /// Build the adapter from a shared `Arc<PostgresRepository>`.
        /// This is the canonical constructor.
        pub fn new(repo: Arc<PostgresRepository>) -> Self {
            Self { repo }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl ReportStore for PostgresReportStore {
        async fn save_report(
            &self,
            workspace_id: &str,
            report: &ReportSummary,
        ) -> Result<(), ReportError> {
            // PHASE 0 stub: write SQL lands once the ingest-state
            // machine grows a publish step (sibling change to
            // `IngestCommit::commit_revision`'s Phase 1 atomicity work).
            //
            // The eventual SQL is:
            //   INSERT INTO graph_reports
            //   (id, workspace_id, created_at, report, symbol_count, edge_count, health_score)
            //   VALUES ($1, $2, $3, $4, $5, $6, $7)
            //   ON CONFLICT (id) DO UPDATE SET
            //     report = EXCLUDED.report,
            //     symbol_count = EXCLUDED.symbol_count,
            //     edge_count = EXCLUDED.edge_count,
            //     health_score = EXCLUDED.health_score
            let _ = (workspace_id, report);
            Ok(())
        }

        async fn latest_report(
            &self,
            workspace_id: &str,
        ) -> Result<Option<ReportSummary>, ReportError> {
            self.repo
                .load_latest_report(workspace_id)
                .await
                .map(|opt| opt.map(ReportSummary::from))
                .map_err(|e| ReportError::Store(e.to_string()))
        }

        async fn reports_for_workspace(
            &self,
            workspace_id: &str,
        ) -> Result<Vec<ReportSummary>, ReportError> {
            // ADR-028: ordered by `created_at DESC`, no time-range
            // filter. The Phase-0 implementation reuses the existing
            // time-bounded helper (`load_report_range(days = 365)`)
            // to keep the SQL surface unchanged; a future PR can
            // lift the `days` filter if unbounded reads become
            // a query pattern that needs to scale.
            self.repo
                .load_report_range(workspace_id, 365)
                .await
                .map(|rows| rows.into_iter().map(ReportSummary::from).collect())
                .map_err(|e| ReportError::Store(e.to_string()))
        }
    }

    impl From<crate::infrastructure::persistence::postgres_repository::GraphReportRow>
        for ReportSummary
    {
        fn from(
            r: crate::infrastructure::persistence::postgres_repository::GraphReportRow,
        ) -> Self {
            Self {
                id: r.id,
                workspace_id: r.workspace_id,
                created_at: r.created_at,
                report: r.report,
                symbol_count: r.symbol_count,
                edge_count: r.edge_count,
                health_score: r.health_score,
            }
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresReportStore;
