//! `cogh::error` — Unified installer error types.
//!
//! Aggregates errors from manifest parsing, network I/O, serialization,
//! rollback, and version mismatches.

use std::path::PathBuf;
use thiserror::Error;

/// Bundle manifest parse or validation error (wraps anyhow::Error).
#[derive(Debug, Error)]
#[error("manifest error: {0}")]
pub struct BundleManifestError(pub anyhow::Error);

/// Unified installer error enum.
#[derive(Debug, Error)]
pub enum InstallerError {
    #[error("manifest parse error: {0}")]
    ManifestParse(#[source] BundleManifestError),

    #[error("version mismatch: {0}")]
    VersionMismatch(#[source] BundleManifestError),

    #[error("network error fetching {0}: {1}")]
    Network(String, String),

    #[error("I/O error on {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error("rollback error: {0}")]
    Rollback(String),

    #[error("unknown error: {0}")]
    Unknown(String),
}

impl From<anyhow::Error> for InstallerError {
    fn from(e: anyhow::Error) -> Self {
        InstallerError::ManifestParse(BundleManifestError(e))
    }
}

impl From<BundleManifestError> for InstallerError {
    fn from(e: BundleManifestError) -> Self {
        InstallerError::ManifestParse(e)
    }
}
