//! Domain port for atomic revision publication.
//!
//! `IngestCommitPort` is the transactional unit-of-work for the Ingest pipeline.
//! It accepts a delta for each stage (graph, manifest, report) and produces
//! a new revision id, effectively "publishing" a coherent snapshot.
//!
//! ## Phase 0 reconciliation
//!
//! The [`PostgresIngestCommitPort`] adapter below now wraps the three stages in
//! a single `pool.begin()` transaction so failures in any stage roll back
//! the whole commit (no orphan revisions on a partial failure). Each
//! stage's SQL lives inside the tx via the underlying `&mut PgConnection`
//! passed to `RevisionStore::create_revision`. The manifest/report
//! stages execute inline SQL through the same tx because the per-row
//! port methods deliberately use their own connection (per ADR-028 §3
//! `ManifestStore::upsert_manifest_entry(&self, row)` is a tx-free
//! convenience for individual ingest rows; the IngestCommitPort contract
//! overrides that contract to require tx-atomicity).
//!
//! This module is gated behind `#[cfg(feature = "multimodal")]` because
//! [`GraphDelta`] carries [`GraphNode`] and [`GraphEdge`] types which are
//! only available when the Generic Graph Layer is enabled.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(unused_imports)]

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
pub trait IngestCommitPort: Send + Sync {
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

/// Error type for [`IngestCommitPort`] operations.
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
