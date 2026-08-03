//! Domain port for the `spaces` table.
//!
//! `spaces` is the federation unit: one row per registered space
//! (Repo / Docs / Issues). The table is seeded with a "default" space
//! on first migration so every pre-federation node has a valid `space_id`
//! to back-reference.
//!
//! This port exists so the federation layer can manage spaces without
//! depending on the `PostgresRepository` concrete type.

use async_trait::async_trait;

use crate::domain::value_objects::{Space, SpaceId};

/// Port for federation space persistence.
#[async_trait]
pub trait FederationStore: Send + Sync {
    /// Register a new space, returning its assigned [`SpaceId`].
    async fn register_space(&self, space: &Space) -> Result<SpaceId, FederationError>;

    /// List every registered space, ordered by `created_at DESC`.
    async fn list_spaces(&self) -> Result<Vec<Space>, FederationError>;

    /// Load a single space by [`SpaceId`], if it exists.
    async fn get_space(&self, id: &SpaceId) -> Result<Option<Space>, FederationError>;
}

/// Error type for [`FederationStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    #[error("federation store error: {0}")]
    Store(String),

    #[error("space already exists: {0}")]
    Conflict(SpaceId),
}


