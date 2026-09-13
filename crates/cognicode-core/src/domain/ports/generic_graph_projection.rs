//! Generic graph projection port (E37 design D5) — the minimal seam that
//! derives a generic graph from the canonical facts of ONE pinned snapshot.
//!
//! The emitted output consists solely of the existing
//! [`GraphNode`]/[`GraphEdge`] values (spec `generic-graph-projection`,
//! requirement "Consumers remain FactStore-ignorant"): a consumer such as an
//! Explorer graph view can adopt the projection without any `FactStore`
//! dependency and without types beyond the existing graph model.
//!
//! Rebuildability (umbrella R2 specialized to this port): rebuilding after
//! clearing the projection, from the same pinned facts, MUST yield an
//! equivalent projection — the in-memory adapter is deterministic by
//! construction (ordered maps + sorted output).
//!
//! ## Gating
//!
//! The module is DUAL-gated behind `all(feature = "evidence-kernel",
//! feature = "multimodal")` (design D5): the facts come from the evidence
//! kernel, and the `GraphNode`/`GraphEdge`/`NodeKind`/`EdgeKind` graph
//! model is the multimodal Generic Graph Layer. The adapter implementing
//! this port lives in `infrastructure::graph::generic_graph_projection`.

use async_trait::async_trait;

use crate::domain::aggregates::{GraphEdge, GraphNode};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::evidence_kernel::ports::KernelError;
use crate::domain::value_objects::WorkspaceId;

/// The output of a generic graph projection: existing graph-model values
/// only, no kernel types (FactStore-ignorance contract).
#[derive(Debug, Clone)]
pub struct GenericProjection {
    /// Emitted nodes, deterministically ordered by node id.
    pub nodes: Vec<GraphNode>,
    /// Emitted edges, deterministically ordered by `(source, target, kind)`.
    pub edges: Vec<GraphEdge>,
}

/// Builds a generic graph from the facts of one pinned snapshot (E37
/// design D5).
///
/// Implementations read the pinned snapshot's facts through a store port
/// and map them onto [`GraphNode`]/[`GraphEdge`] values. The output MUST
/// contain no dangling node references and no kernel types.
#[async_trait]
pub trait GenericGraphProjectionPort: Send + Sync {
    /// Projects the generic graph for `snap` of `ws`.
    async fn project(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> Result<GenericProjection, KernelError>;
}
