//! `CallGraphStore` — domain port for the canonical call-graph WS round-trip.
//!
//! Surfaces `save_call_graph_ws` and `load_call_graph_ws` behind a
//! `Send + Sync` trait shape so consumers (snapshot provider, graph
//! executor, postgres bridge, ingest service) can depend on the
//! port instead of concrete adapter types.
//!
//! # Why a dedicated port (not `RevisionStore`)
//!
//! - `RevisionStore` is the workspace-scoped revision head *counter*
//!   (3 methods: `create_revision`, `set_head`, `head_revision`).
//!   It manipulates the `graph_revisions` table; no `CallGraph`
//!   aggregate is involved.
//! - The generic graph layer (the
//!   `graph_nodes` + `graph_edges` tables populated by the docs
//!   extractor). It carries no workspace/revision concept.
//!
//! The canonical call-graph WS round-trip is its own aggregate
//! domain concept — a workspace's edges + nodes at a specific
//! revision — and warrants its own port.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::aggregates::CallGraph;
use crate::domain::value_objects::{RevisionId, WorkspaceId};

/// Port for the canonical call-graph WS round-trip.
#[async_trait]
pub trait CallGraphStore: Send + Sync {
    /// Persist the `CallGraph` aggregate for `workspace_id` at a new
    /// revision. Returns the newly-opened [`RevisionId`].
    ///
    /// The concrete PG implementation delegates to
    /// the canonical adapter's `save_call_graph_ws`, which atomically
    /// demotes the prior head, opens the new revision (via
    /// `PostgresRevisionStore::create_revision`), and writes the
    /// graph edges + nodes.
    async fn save_call_graph_ws(
        &self,
        graph: &CallGraph,
        ws: &WorkspaceId,
    ) -> Result<RevisionId, CallGraphError>;

    /// Load the `CallGraph` aggregate for `workspace_id` at
    /// `revision_id`. Returns `Ok(None)` when the workspace has no
    /// call graph at that revision (the aggregate is workspace+revision
    /// keyed, not single-row global).
    async fn load_call_graph_ws(
        &self,
        ws: &WorkspaceId,
        revision: RevisionId,
    ) -> Result<Option<CallGraph>, CallGraphError>;

    /// Load the *current head* `CallGraph` aggregate for `workspace_id`.
    /// Equivalent to `load_call_graph_ws(ws, current_head(ws))` but
    /// resolved atomically in a single SQL call.
    async fn load_call_graph_current(
        &self,
        ws: &WorkspaceId,
    ) -> Result<Option<CallGraph>, CallGraphError>;
}

/// Error type for [`CallGraphStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum CallGraphError {
    #[error("call graph store error: {0}")]
    Store(String),
    #[error("call graph not found for workspace {0} at revision {1:?}")]
    NotFound(WorkspaceId, RevisionId),
    #[error("call graph store conflict: {0}")]
    Conflict(String),
}
