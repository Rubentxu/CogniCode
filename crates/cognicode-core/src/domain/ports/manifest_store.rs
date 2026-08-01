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

/// Domain type returned by [`ManifestStore::load_manifest`] and
/// accepted by [`ManifestStore::upsert_row`].
///
/// Mirrors the columns of the `scan_manifest` table but lives in the
/// domain layer. The Postgres adapter performs the row-to-domain
/// translation internally; callers never see the row type.
#[derive(Debug, Clone)]
pub struct ScanManifest {
    pub workspace_id: String,
    pub file_path: String,
    pub file_type: String,
    pub language: Option<String>,
    pub content_hash: String,
    pub mtime: f64,
    pub symbol_count: i32,
    pub edge_count: i32,
    pub status: String,
    pub error_msg: Option<String>,
}

/// Port for scan manifest persistence.
#[async_trait]
pub trait ManifestStore: Send + Sync {
    /// Load every `scan_manifest` row for a workspace.
    ///
    /// Returns an empty `Vec` if the workspace has never been scanned.
    /// Never an error — empty result is a valid state.
    async fn load_manifest(&self, workspace_id: &str) -> Result<Vec<ScanManifest>, ManifestError>;

    /// Upsert a single `scan_manifest` row.
    ///
    /// On conflict (same `(workspace_id, file_path)`), updates every
    /// mutable column and refreshes `scanned_at`.
    async fn upsert_row(&self, row: &ScanManifest) -> Result<(), ManifestError>;

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
    use super::{ManifestError, ManifestStore, ScanManifest};
    use crate::infrastructure::persistence::{PostgresRepository, ScanManifestRow};
    use async_trait::async_trait;
    use std::sync::Arc;

    /// Row ↔ domain translations. Keep the row type inside the adapter
    /// so the port surface stays free of infrastructure imports.
    impl From<ScanManifestRow> for ScanManifest {
        fn from(r: ScanManifestRow) -> Self {
            Self {
                workspace_id: r.workspace_id,
                file_path: r.file_path,
                file_type: r.file_type,
                language: r.language,
                content_hash: r.content_hash,
                mtime: r.mtime,
                symbol_count: r.symbol_count,
                edge_count: r.edge_count,
                status: r.status,
                error_msg: r.error_msg,
            }
        }
    }

    impl From<ScanManifest> for ScanManifestRow {
        fn from(m: ScanManifest) -> Self {
            Self {
                workspace_id: m.workspace_id,
                file_path: m.file_path,
                file_type: m.file_type,
                language: m.language,
                content_hash: m.content_hash,
                mtime: m.mtime,
                symbol_count: m.symbol_count,
                edge_count: m.edge_count,
                status: m.status,
                error_msg: m.error_msg,
            }
        }
    }

    /// Adapter that delegates every [`ManifestStore`] method to a
    /// [`PostgresRepository`]. Exists so the Ingest pipeline can talk
    /// to the port rather than the concrete adapter.
    #[cfg(feature = "postgres")]
    pub struct PostgresManifestStore<'a> {
        repo: &'a PostgresRepository,
    }

    #[cfg(feature = "postgres")]
    impl<'a> PostgresManifestStore<'a> {
        pub fn new(repo: &'a PostgresRepository) -> Self {
            Self { repo }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl<'a> ManifestStore for PostgresManifestStore<'a> {
        async fn load_manifest(
            &self,
            workspace_id: &str,
        ) -> Result<Vec<ScanManifest>, ManifestError> {
            self.repo
                .load_scan_manifest(workspace_id)
                .await
                .map(|rows| rows.into_iter().map(ScanManifest::from).collect())
                .map_err(|e| ManifestError::Store(e.to_string()))
        }

        async fn upsert_row(&self, row: &ScanManifest) -> Result<(), ManifestError> {
            self.repo
                .upsert_scan_manifest_row(&ScanManifestRow::from(row.clone()))
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
