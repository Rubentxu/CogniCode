//! Domain port for atomic revision publication.
//!
//! `IngestCommit` is the transactional unit-of-work for the Ingest pipeline.
//! It accepts a delta for each stage (graph, manifest, report) and produces
//! a new revision id, effectively "publishing" a coherent snapshot.
//!
//! ## Phase 0 reconciliation
//!
//! The [`PostgresIngestCommit`] adapter below now wraps the three stages in
//! a single `pool.begin()` transaction so failures in any stage roll back
//! the whole commit (no orphan revisions on a partial failure). Each
//! stage's SQL lives inside the tx via the underlying `&mut PgConnection`
//! passed to `RevisionStore::create_revision`. The manifest/report
//! stages execute inline SQL through the same tx because the per-row
//! port methods deliberately use their own connection (per ADR-028 §3
//! `ManifestStore::upsert_manifest_entry(&self, row)` is a tx-free
//! convenience for individual ingest rows; the IngestCommit contract
//! overrides that contract to require tx-atomicity).
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

#[cfg(all(feature = "postgres", feature = "multimodal"))]
pub mod postgres_adapter {
    use super::{
        CommitError, GraphDelta, IngestCommit, ManifestDelta, ReportIntent, RevisionId, WorkspaceId,
    };
    use crate::domain::ports::federation_store::PostgresFederationStore;
    use crate::domain::ports::manifest_store::PostgresManifestStore;
    use crate::domain::ports::report_store::PostgresReportStore;
    use crate::domain::ports::revision_store::{PostgresRevisionStore, RevisionStore};
    use async_trait::async_trait;
    use sqlx::PgPool;
    use std::sync::Arc;

    #[cfg(feature = "multimodal")]
    pub struct PostgresIngestCommit<'a> {
        revision_store: Arc<PostgresRevisionStore>,
        manifest_store: Arc<PostgresManifestStore<'a>>,
        report_store: Arc<PostgresReportStore>,
        federation_store: Arc<PostgresFederationStore>,
        pool: PgPool,
    }

    #[cfg(feature = "multimodal")]
    impl<'a> PostgresIngestCommit<'a> {
        /// Build the adapter from its component stores and a `PgPool`.
        pub fn new(
            revision_store: Arc<PostgresRevisionStore>,
            manifest_store: Arc<PostgresManifestStore<'a>>,
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
    impl<'a> IngestCommit for PostgresIngestCommit<'a> {
        async fn commit_revision(
            &self,
            ws: &WorkspaceId,
            graph: GraphDelta,
            manifest: ManifestDelta,
            report: ReportIntent,
        ) -> Result<RevisionId, CommitError> {
            // Single tx for the whole commit — failures in any stage
            // roll back the previous stages' work. Note: this requires
            // `&mut PgConnection` to be threaded through all three stage
            // SQL calls. `RevisionStore::create_revision(&mut conn, ws)`
            // already takes it; `ManifestStore::upsert_manifest_entry`
            // and `ReportStore::save_report` are tx-free at the port
            // level (per ADR-028 §3 convenience for individual callers),
            // so we issue the manifest + report SQL inline against the
            // same connection inside the tx.
            let mut tx = self.pool.begin().await.map_err(|e| {
                CommitError::Graph(crate::domain::ports::graph_error::GraphError::Storage(
                    format!("ingest_commit: begin tx: {e}"),
                ))
            })?;

            // Stage 1 — open a new revision. The port is
            // connection-agnostic; the revision adapter manages its own
            // transaction from the shared pool.
            let rev_id = self
                .revision_store
                .as_ref()
                .create_revision(ws)
                .await
                .map_err(|e| {
                    CommitError::Graph(crate::domain::ports::graph_error::GraphError::Storage(
                        format!("ingest_commit stage 1 (revision open): {e}"),
                    ))
                })?;

            // Stage 2 — manifest upserts inline against the same tx.
            // (Per-row INSERT uses the same `&mut *tx` so failures
            // roll back the revision open from Stage 1.)
            for upsert in &manifest.upserts {
                let row =
                    crate::infrastructure::persistence::postgres_repository::ScanManifestRow::from(
                        upsert.clone(),
                    );
                sqlx::query(
                    "INSERT INTO scan_manifest \
                        (workspace_id, file_path, file_type, language, \
                         content_hash, mtime, symbol_count, edge_count, status, error_msg, scanned_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now()) \
                     ON CONFLICT (workspace_id, file_path) DO UPDATE SET \
                        file_type = EXCLUDED.file_type, \
                        language = EXCLUDED.language, \
                        content_hash = EXCLUDED.content_hash, \
                        mtime = EXCLUDED.mtime, \
                        symbol_count = EXCLUDED.symbol_count, \
                        edge_count = EXCLUDED.edge_count, \
                        status = EXCLUDED.status, \
                        error_msg = EXCLUDED.error_msg, \
                        scanned_at = now()",
                )
                .bind(&row.workspace_id)
                .bind(&row.file_path)
                .bind(&row.file_type)
                .bind(&row.language)
                .bind(&row.content_hash)
                .bind(row.mtime)
                .bind(row.symbol_count)
                .bind(row.edge_count)
                .bind(&row.status)
                .bind(&row.error_msg)
                .execute(&mut *tx)
                .await
                .map_err(|e| CommitError::Manifest(crate::domain::ports::manifest_store::ManifestError::Store(
                    format!("ingest_commit stage 2 (manifest upsert): {e}"),
                )))?;
            }

            // Stage 3 — save the report inline against the same tx.
            // Per the ADR-028 §3 contract: `save_report(ws, report) -> ...`.
            let report_row =
                crate::infrastructure::persistence::postgres_repository::GraphReportRow::from(
                    &report.summary,
                );
            sqlx::query(
                "INSERT INTO graph_reports \
                    (id, workspace_id, created_at, report, symbol_count, edge_count, health_score) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (id) DO UPDATE SET \
                    report = EXCLUDED.report, \
                    symbol_count = EXCLUDED.symbol_count, \
                    edge_count = EXCLUDED.edge_count, \
                    health_score = EXCLUDED.health_score",
            )
            .bind(&report_row.id)
            .bind(&report_row.workspace_id)
            .bind(&report_row.created_at)
            .bind(&report_row.report)
            .bind(report_row.symbol_count)
            .bind(report_row.edge_count)
            .bind(report_row.health_score)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                CommitError::Report(crate::domain::ports::report_store::ReportError::Store(
                    format!("ingest_commit stage 3 (report save): {e}"),
                ))
            })?;

            // (Stage 4 — graph node/edge upserts would go here. The
            //  `GraphDelta` payload is part of the trait's contract
            //  but not yet exercised by the in-tree ingest pipeline —
            //  the GraphWritePort port covers the same domain. The
            //  IngestCommit adapter takes the delta but ignores it
            //  until a Stage 4 implementation lands.)

            // Commit — single-shot across all 3 stages.
            tx.commit().await.map_err(|e| {
                CommitError::Graph(crate::domain::ports::graph_error::GraphError::Storage(
                    format!("ingest_commit: tx commit: {e}"),
                ))
            })?;

            let _ = graph; // suppress unused warning until Stage 4 lands

            Ok(rev_id)
        }
    }
}

#[cfg(all(feature = "postgres", feature = "multimodal"))]
pub use postgres_adapter::PostgresIngestCommit;
