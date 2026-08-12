//! `cogh::install` — Top-level install command entry point.
//!
//! Provides `run_install()` which wraps [`InstallerTransaction::run`]
//! with profile selection and error handling.

use crate::error::InstallerError;
use crate::installer_transaction::InstallerTransaction;

/// Run the atomic install transaction.
///
// Loads the bundle manifest, validates the version, and executes the
/// install pipeline (download → verify → extract → shim → manifest).
///
/// Returns the path to the written install manifest on success.
pub fn run_install(profile: &str) -> Result<std::path::PathBuf, InstallerError> {
    InstallerTransaction::run(profile)
}
