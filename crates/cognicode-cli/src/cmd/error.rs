//! `cogh::error` — Unified installer error types.
//!
//! Aggregates errors from manifest parsing, network I/O, serialization,
//! rollback, and version mismatches.

use std::path::PathBuf;
use thiserror::Error;

use crate::bundle_manifest::Platform;

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

    #[error("SHA256 mismatch: downloaded file does not match expected hash")]
    Sha256Mismatch,

    #[error("shim install error: {0}")]
    ShimInstall(String),

    /// The host platform is not in the e85 Tier-1 surface (e86 REQ-LR-01).
    #[error("platform {0:?} is not in the e85 Tier-1 surface; supported: {1}")]
    PlatformNotInTier1(Platform, String),

    /// A release was refused because it is still a draft or a prerelease
    /// (e86 REQ-LR-03 / REQ-LDS-01).
    #[error("{0}")]
    DraftRelease(String),

    /// A release exists but lacks the per-platform bundle manifest asset
    /// (e86 REQ-LR-04).
    #[error("no per-platform manifest asset for platform {0:?}; available: {1:?}")]
    NoMatchingManifest(Platform, Vec<String>),

    /// Generic resolver failure (network error, JSON parse error, missing
    /// fixture file, etc.). Distinct from `Network` so the resolver layer
    /// can carry a clear single message (e86 REQ-LR-08 / REQ-LR-09).
    #[error("resolve failed: {0}")]
    ResolveFailed(String),

    /// The selected profile matched zero components in the bundle
    /// manifest (e86.1 REQ-LJ-04). Without this, a typo'd profile
    /// would silently install nothing while still pinning the
    /// tracker and writing the lifecycle journal — the most
    /// insidious masking failure mode in the install pipeline.
    #[error(
        "profile {0:?} matches no components in bundle manifest version {1}; refusing to install nothing"
    )]
    EmptyInstall(String, String),

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
