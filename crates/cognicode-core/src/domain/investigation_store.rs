//! Investigation store trait — ADR-005 Phase INV-1.
//!
//! Defines the port interface for investigation persistence.

use async_trait::async_trait;

use super::investigation::Investigation;

/// Errors that can occur when operating on investigations.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Investigation not found: {0}")]
    NotFound(String),

    #[error("Encode error: {0}")]
    Encode(String),

    #[error("Decode error: {0}")]
    Decode(String),

    #[error("Transaction error: {0}")]
    Transaction(String),
}

/// Port interface for investigation persistence.
///
/// Implementors must be Send + Sync to allow shared access from
/// multiple async tasks.
#[async_trait]
pub trait InvestigationStore: Send + Sync {
    /// Save an investigation (upsert semantics).
    async fn save(&self, investigation: &Investigation) -> Result<(), StoreError>;

    /// Load an investigation by ID.
    /// Returns `Ok(None)` when the investigation does not exist.
    async fn load(&self, id: &str) -> Result<Option<Investigation>, StoreError>;

    /// List all investigations for a workspace, ordered by updated_at desc.
    async fn list(&self, workspace_id: &str) -> Result<Vec<Investigation>, StoreError>;

    /// Delete an investigation by ID.
    /// Returns `Ok(())` even if the investigation did not exist.
    async fn delete(&self, id: &str) -> Result<(), StoreError>;
}
