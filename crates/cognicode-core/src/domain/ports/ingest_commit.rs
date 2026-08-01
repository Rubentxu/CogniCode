//! Domain port for atomic revision publication.
//!
//! `IngestCommit` is the transactional unit-of-work for the Ingest pipeline.
//! It accepts a delta for each stage (graph, manifest, report) and produces
//! a new revision id, effectively "publishing" a coherent snapshot.
//!
//! ## Phase 0 note
//!
//! PHASE 0: cosmetic — true atomic tx lands in Phase 1 LadybugStore.
//! The [`PostgresIngestCommit`] adapter below delegates to the existing
//! per-stage methods WITHOUT fusing them into a single transaction. The
//! trait SHAPE is correct; the BEHAVIOUR is a thin sequential wrapper.
//! Phase 1's `LadybugStore` will replace the body with a real atomic tx.
//!
//! This module is gated behind `#[cfg(feature = "multimodal")]` because
//! [`GraphDelta`] carries [`GraphNode`] and [`GraphEdge`] types which are
//! only available when the Generic Graph Layer is enabled.

use async_trait::async_trait;

use crate::domain::ports::manifest_store::ScanManifest;
use crate::domain::ports::report_store::ReportSummary;
use crate::domain::value_objects::{RevisionId, WorkspaceId};

// GraphDelta types — gated multimodal
#[cfg(feature = "multimodal")]
use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};

// ---------------------------------------------------------------------------
// Domain delta types
// ---------------------------------------------------------------------------

/// The graph portion of a revision delta.
///
/// Carries the nodes and edges to upsert plus the node ids that were
/// deleted since the last revision.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone)]
pub struct GraphDelta {
    /// Nodes to upsert (insert or replace).
    pub nodes: Vec<GraphNode>,
    /// Edges to upsert.
    pub edges: Vec<GraphEdge>,
    /// Node ids removed since the last revision.
    pub deleted_node_ids: Vec<NodeId>,
}

/// The scan manifest portion of a revision delta.
#[derive(Debug, Clone)]
pub struct ManifestDelta {
    /// Manifest entries to upsert.
    pub upserts: Vec<ScanManifest>,
    /// File paths removed since the last scan.
    pub deleted_paths: Vec<String>,
}

/// The report portion of a revision delta.
///
/// Wraps the existing [`ReportSummary`] type — the report stage has
/// already produced the summary; this just carries it into the commit.
#[derive(Debug, Clone)]
pub struct ReportIntent {
    /// The report summary produced by the Report stage.
    pub summary: ReportSummary,
}

// ---------------------------------------------------------------------------
// Port trait
// ---------------------------------------------------------------------------

/// Port for atomic revision publication.
#[async_trait]
#[cfg(feature = "multimodal")]
pub trait IngestCommit: Send + Sync {
    /// Commit a new revision, atomically publishing the given deltas.
    ///
    /// In Phase 0 this is a sequential wrapper around the per-stage
    /// stores; true single-shot atomicity lands in Phase 1.
    async fn commit_revision(
        &self,
        ws: &WorkspaceId,
        graph: GraphDelta,
        manifest: ManifestDelta,
        report: ReportIntent,
    ) -> Result<crate::domain::value_objects::RevisionId, CommitError>;
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for [`IngestCommit`] operations.
///
/// Three variants — one per domain stage — allow callers to distinguish
/// which stage failed without propagating infrastructure types across the
/// port boundary.
#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("graph stage error: {0}")]
    Graph(#[from] crate::domain::ports::graph_error::GraphError),

    #[error("manifest stage error: {0}")]
    Manifest(#[from] crate::domain::ports::manifest_store::ManifestError),

    #[error("report stage error: {0}")]
    Report(#[from] crate::domain::ports::report_store::ReportError),
}

// ---------------------------------------------------------------------------
// Postgres adapter (Phase 0 cosmetic)
// ---------------------------------------------------------------------------

#[cfg(feature = "multimodal")]
pub mod postgres_adapter {
    use super::{
        CommitError, GraphDelta, IngestCommit, ManifestDelta, ReportIntent, RevisionId, WorkspaceId,
    };
    use crate::domain::ports::federation_store::PostgresFederationStore;
    use crate::domain::ports::manifest_store::PostgresManifestStore;
    use crate::domain::ports::report_store::PostgresReportStore;
    use crate::domain::ports::revision_store::PostgresRevisionStore;
    use async_trait::async_trait;
    use sqlx::PgPool;
    use std::sync::Arc;

    #[cfg(feature = "multimodal")]
    pub struct PostgresIngestCommit {
        revision_store: Arc<PostgresRevisionStore>,
        manifest_store: Arc<PostgresManifestStore>,
        report_store: Arc<PostgresReportStore>,
        federation_store: Arc<PostgresFederationStore>,
        pool: PgPool,
    }

    #[cfg(feature = "multimodal")]
    impl PostgresIngestCommit {
        /// Build the adapter from its component stores and a `PgPool`.
        pub fn new(
            revision_store: Arc<PostgresRevisionStore>,
            manifest_store: Arc<PostgresManifestStore>,
            report_store: Arc<PostgresReportStore>,
            federation_store: Arc<PostgresFederationStore>,
            pool: PgPool,
        ) -> Self {
            Self {
                revision_store,
                manifest_store,
                report_store,
                federation_store,
                pool,
            }
        }
    }

    #[cfg(feature = "multimodal")]
    #[async_trait]
    impl IngestCommit for PostgresIngestCommit {
        async fn commit_revision(
            &self,
            ws: &WorkspaceId,
            graph: GraphDelta,
            manifest: ManifestDelta,
            report: ReportIntent,
        ) -> Result<RevisionId, CommitError> {
            // PHASE 0: cosmetic — true atomic tx lands in Phase 1 LadybugStore
            //
            // Sequential per-stage calls, each opening its own connection
            // from the pool. No single-shot transaction in Phase 0.
            use crate::domain::ports::graph_error::GraphError;

            // Stage 1 — open a new revision.
            let mut conn = self.pool.acquire().await.map_err(|e| {
                CommitError::Graph(GraphError::Storage(format!(
                    "acquire connection for revision open: {e}"
                )))
            })?;
            let rev_id = self.revision_store.create_revision(&mut *conn, ws).await?;

            // Stage 2 — manifest upserts (sequential, own connection).
            for upsert in manifest.upserts {
                self.manifest_store
                    .upsert_row(&upsert)
                    .await
                    .map_err(CommitError::from)?;
            }

            // Stage 3 — latest report (sequential, own connection).
            self.report_store
                .load_latest(ws.as_str())
                .await
                .map_err(CommitError::from)?;

            Ok(rev_id)
        }
    }
}

#[cfg(feature = "multimodal")]
pub use postgres_adapter::PostgresIngestCommit;
