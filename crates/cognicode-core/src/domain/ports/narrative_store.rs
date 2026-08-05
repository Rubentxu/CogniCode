//! `NarrativeStore` — domain port for narrative view snapshots.
//!
//! Stores rendered view snapshots (ContextualView JSON payloads) keyed by
//! `(workspace_id, view_id, object_id)`. The synthetic primary key is
//! `format!("{}::{}::{}", workspace_id, view_id, object_id)` — this
//! matches the `QualityStore` / `ManifestStore` pattern used elsewhere
//! in the ladybug adapter.

use serde::{Deserialize, Serialize};

/// A single narrative view snapshot.
///
/// `id` is the synthetic PK: `format!("{}::{}::{}", workspace_id, view_id, object_id)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeSnapshot {
    /// Synthetic primary key: `{}::{}::{}` of `(workspace_id, view_id, object_id)`.
    pub id: String,
    pub workspace_id: String,
    pub view_id: String,
    pub object_id: String,
    pub view_kind: String,
    /// JSON-serialized `ContextualView` payload.
    pub payload: String,
    pub source_rev: u64,
    /// ISO 8601 timestamp.
    pub created_at: String,
}

/// Errors returned by [`NarrativeStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum NarrativeError {
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("narrative snapshot not found")]
    NotFound,
}

/// Domain port for persisting and retrieving rendered narrative view snapshots.
///
/// Implementations must be `Send + Sync` and `Arc`-friendly.
#[async_trait::async_trait]
pub trait NarrativeStore: Send + Sync {
    /// Persist a narrative snapshot. Upsert by synthetic `id`.
    async fn save_snapshot(&self, snap: &NarrativeSnapshot) -> Result<(), NarrativeError>;

    /// Load a narrative snapshot by `(workspace_id, view_id, object_id)`.
    /// Returns `Ok(None)` when no snapshot exists.
    async fn load_snapshot(
        &self,
        ws: &str,
        view_id: &str,
        object_id: &str,
    ) -> Result<Option<NarrativeSnapshot>, NarrativeError>;

    /// List all narrative snapshots for a workspace.
    /// When `view_kind` is `Some`, filter to that kind only.
    async fn list_for_workspace(
        &self,
        ws: &str,
        view_kind: Option<&str>,
    ) -> Result<Vec<NarrativeSnapshot>, NarrativeError>;

    /// Invalidate (delete) all snapshots for a workspace at or below `source_rev`.
    /// Returns the count of deleted rows.
    async fn invalidate(&self, ws: &str, source_rev: u64) -> Result<u64, NarrativeError>;
}
