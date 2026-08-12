//! `cogh::cache` — Download cache helpers with rollback support.
//!
//! Provides atomic download helpers that integrate with the
//! [`RollbackJournal`] so that partial downloads can be cleaned up
//! on failure or startup.

use std::path::Path;

use crate::error::InstallerError;
use crate::rollback_journal::{RollbackJournal, SideEffect};

/// Download a file with rollback support.
///
/// // Uses a `.part` staging file + atomic rename so that a interrupted
/// download never leaves a partial file at the destination path.
/// The `.part` path is recorded in the journal for automatic cleanup.
pub fn download_with_rollback(
    url: &str,
    dest: &Path,
    journal: &mut RollbackJournal,
) -> Result<(), InstallerError> {
    use std::io::Write;

    let part_path = dest.with_extension("part");

    // Download to .part staging file using reqwest blocking client
    let mut response = reqwest::blocking::get(url)
        .map_err(|e| InstallerError::Network(url.into(), e.to_string()))?;

    let mut file = std::fs::File::create(&part_path)
        .map_err(|e| InstallerError::Io(part_path.clone(), e))?;

    let _bytes_copied = response.copy_to(&mut file)
        .map_err(|e| InstallerError::Network("copying response".into(), e.to_string()))?;

    // Record for rollback cleanup
    journal.record(SideEffect::Downloaded(part_path.clone()));

    // Atomic rename: .part → dest
    // On Unix this is atomic if dest's directory is on the same filesystem.
    std::fs::rename(&part_path, dest)
        .map_err(|e| InstallerError::Io(dest.into(), e))?;

    Ok(())
}

/// Clean up any stale `.part` partial downloads in the cache directory.
///
// Call this on installer startup to ensure no orphaned `.part` files
/// remain from a previous interrupted run.
pub fn partial_download_cleanup(cache_dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "part").unwrap_or(false) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn partial_download_cleanup_removes_part_files() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path();

        // Create a regular file and a .part file
        let regular = cache.join("already-done.tar.gz");
        let partial = cache.join("in-progress.tar.gz.part");

        std::fs::write(&regular, b"done").unwrap();
        std::fs::write(&partial, b"partial").unwrap();

        partial_download_cleanup(cache);

        // Regular file should still exist
        assert!(regular.exists(), "regular file should NOT be removed");
        // .part file should be removed
        assert!(!partial.exists(), ".part file should be removed");
    }

    #[test]
    fn partial_download_cleanup_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path();

        // Call twice — should not panic or error
        partial_download_cleanup(cache);
        partial_download_cleanup(cache);
    }
}
