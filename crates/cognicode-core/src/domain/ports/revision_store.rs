//! Domain port for the `graph_revisions` table.
//!
//! `graph_revisions` tracks the workspace-scoped revision timeline.
//! Each workspace has a monotonically-increasing integer revision counter,
//! and exactly one row has `head_of = true` at any time (the "current"
//! revision). Opening a new revision is atomic: demote the old head,
//! insert the new row, and bump the counter.
//!
//! This port exists so `save_call_graph_ws` can open revisions without
//! depending on concrete adapter types directly.
//!
//! # Method shape
//!
//! The 3 methods split along the read/write boundary:
//!
//! - [`head_revision`](RevisionStore::head_revision) — **read-only**,
//!   takes only the workspace.
//! - [`create_revision`](RevisionStore::create_revision) — **write**.
//!   Connection-agnostic: no backend-specific transaction handle is
//!   threaded through the trait (the previous connection handle
//!   parameter was a PostgreSQL-typed leak; the lbug adapter ignores it
//!   and opens its own connection from the shared `Database`).
//! - [`set_head`](RevisionStore::set_head) — **write**, same
//!   connection-agnostic shape.

use async_trait::async_trait;

use crate::domain::value_objects::{RevisionId, WorkspaceId};

/// Port for revision lifecycle management.
#[async_trait]
pub trait RevisionStore: Send + Sync {
    /// Return the current head revision id for a workspace, if any.
    ///
    /// **Read-only**: takes only the workspace, returns
    /// `Ok(None)` for an unknown workspace.
    async fn head_revision(&self, ws: &WorkspaceId) -> Result<Option<RevisionId>, RevisionError>;

    /// Open a new revision for `workspace_id`, atomically demoting the
    /// prior head (if any) and inserting the new row with `head_of = true`.
    ///
    /// Connection-agnostic (no `PgConnection` leak): adapters manage
    /// their own transaction/connection from the shared store handle.
    ///
    /// Returns the newly-opened [`RevisionId`].
    async fn create_revision(&self, ws: &WorkspaceId) -> Result<RevisionId, RevisionError>;

    /// Promote an existing revision to be the new head of the workspace.
    /// Demotes any prior head atomically.
    async fn set_head(&self, ws: &WorkspaceId, rev: RevisionId) -> Result<(), RevisionError>;
}

/// Error type for [`RevisionStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum RevisionError {
    #[error("revision store error: {0}")]
    Store(String),

    #[error("revision not found for workspace {0}")]
    NotFound(WorkspaceId),
}
