//! Domain port for the `scan_manifest` table.
//!
//! `scan_manifest` tracks the last-seen state of every file in the
//! workspace: content hash, mtime, extraction stats. The pipeline's
//! Scan stage uses this for incremental change detection (mtime-first,
//! hash-second).
//!
//! This port exists so application code (the Ingest pipeline's
//! service layer) can call into the persistence layer without
//! depending on `PostgresRepository` concrete types.

use async_trait::async_trait;

use crate::infrastructure::persistence::ScanManifestRow;

/// Port for scan manifest persistence.
#[async_trait]
pub trait ManifestStore: Send + Sync {
    /// Load every `scan_manifest` row for a workspace.
    ///
    /// Returns an empty `Vec` if the workspace has never been scanned.
    /// Never an error — empty result is a valid state.
    async fn load_manifest(&self, workspace_id: &str) -> Result<Vec<ScanManifestRow>, ManifestError>;

    /// Upsert a single `scan_manifest` row.
    ///
    /// On conflict (same `(workspace_id, file_path)`), updates every
    /// mutable column and refreshes `scanned_at`.
    async fn upsert_row(&self, row: &ScanManifestRow) -> Result<(), ManifestError>;

    /// Delete every `scan_manifest` row for a workspace whose file path
    /// is NOT in `keep_paths`. Used by the Scan stage to garbage-collect
    /// entries for deleted files.
    ///
    /// Returns the number of rows deleted.
    async fn delete_except(
        &self,
        workspace_id: &str,
        keep_paths: &[String],
    ) -> Result<usize, ManifestError>;
}

/// Error type for [`ManifestStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("scan manifest store error: {0}")]
    Store(String),
}

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use super::{ManifestError, ManifestStore};
    use crate::infrastructure::persistence::{PostgresRepository, ScanManifestRow};
    use async_trait::async_trait;
    use std::sync::Arc;

    /// Adapter that delegates every [`ManifestStore`] method to a
    /// [`PostgresRepository`]. Exists so the Ingest pipeline can talk
    /// to the port rather than the concrete adapter.
    #[cfg(feature = "postgres")]
    pub struct PostgresManifestStore {
        repo: Arc<PostgresRepository>,
    }

    #[cfg(feature = "postgres")]
    impl PostgresManifestStore {
        pub fn new(repo: Arc<PostgresRepository>) -> Self {
            Self { repo }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl ManifestStore for PostgresManifestStore {
        async fn load_manifest(
            &self,
            workspace_id: &str,
        ) -> Result<Vec<ScanManifestRow>, ManifestError> {
            self.repo
                .load_scan_manifest(workspace_id)
                .await
                .map_err(|e| ManifestError::Store(e.to_string()))
        }

        async fn upsert_row(&self, row: &ScanManifestRow) -> Result<(), ManifestError> {
            self.repo
                .upsert_scan_manifest_row(row)
                .await
                .map_err(|e| ManifestError::Store(e.to_string()))
        }

        async fn delete_except(
            &self,
            workspace_id: &str,
            keep_paths: &[String],
        ) -> Result<usize, ManifestError> {
            self.repo
                .delete_scan_manifest_except(workspace_id, keep_paths)
                .await
                .map_err(|e| ManifestError::Store(e.to_string()))
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresManifestStore;