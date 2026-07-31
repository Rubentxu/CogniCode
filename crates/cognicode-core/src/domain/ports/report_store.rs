//! Domain port for the `graph_reports` table.
//!
//! `graph_reports` stores per-revision summaries produced by the
//! pipeline's Report stage. Used by the `graph_diff` and `graph_timeline`
//! MCP tools.
//!
//! This port exists so application code can read graph reports
//! without depending on `PostgresRepository` concrete types.

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

/// Port for graph report persistence (read-only).
#[async_trait]
pub trait ReportStore: Send + Sync {
    /// Load the most recent `graph_reports` row for a workspace.
    ///
    /// Returns `Ok(None)` when no report exists yet.
    async fn load_latest(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ReportSummary>, ReportError>;

    /// Load every `graph_reports` row for a workspace within the last
    /// `days` days, ordered newest-first.
    async fn load_range(
        &self,
        workspace_id: &str,
        days: i32,
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
        async fn load_latest(
            &self,
            workspace_id: &str,
        ) -> Result<Option<ReportSummary>, ReportError> {
            self.repo
                .load_latest_report(workspace_id)
                .await
                .map(|opt| opt.map(ReportSummary::from))
                .map_err(|e| ReportError::Store(e.to_string()))
        }

        async fn load_range(
            &self,
            workspace_id: &str,
            days: i32,
        ) -> Result<Vec<ReportSummary>, ReportError> {
            self.repo
                .load_report_range(workspace_id, days)
                .await
                .map(|rows| rows.into_iter().map(ReportSummary::from).collect())
                .map_err(|e| ReportError::Store(e.to_string()))
        }
    }

    impl From<crate::infrastructure::persistence::postgres_repository::GraphReportRow> for ReportSummary {
        fn from(r: crate::infrastructure::persistence::postgres_repository::GraphReportRow) -> Self {
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