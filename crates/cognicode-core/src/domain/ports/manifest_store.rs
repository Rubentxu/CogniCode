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

/// Domain type returned by [`ManifestStore::get_manifest`] and
/// accepted by [`ManifestStore::upsert_manifest_entry`].
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
    async fn get_manifest(&self, workspace_id: &str) -> Result<Vec<ScanManifest>, ManifestError>;

    /// Upsert a single `scan_manifest` row.
    ///
    /// On conflict (same `(workspace_id, file_path)`), updates every
    /// mutable column and refreshes `scanned_at`.
    async fn upsert_manifest_entry(&self, row: &ScanManifest) -> Result<(), ManifestError>;

    /// Delete every `scan_manifest` row for a workspace whose file path
    /// is NOT in `keep_paths`. Used by the Scan stage to garbage-collect
    /// entries for deleted files.
    ///
    /// Returns the number of rows deleted.
    async fn delete_manifest_entry(
        &self,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<(), ManifestError>;
}

/// Error type for [`ManifestStore`] operations.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("scan manifest store error: {0}")]
    Store(String),
}
