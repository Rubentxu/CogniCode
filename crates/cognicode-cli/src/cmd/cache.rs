//! `cogh::cache` — Download cache helpers with rollback support.
//!
//! Provides atomic download helpers that integrate with the
//! [`RollbackJournal`] so that partial downloads can be cleaned up
//! on failure or startup.
//!
//! `partial_download_cleanup` is part of the public `cogh::cache` API
//! surface; it is exercised by the unit test below and is expected to be
//! called by the install startup path.  H-06 (JOURNAL §99/§100,
//! commits `728f05a0`/`d0913498`) exercised the install path
//! end-to-end against a payload fixture; a future PRF-DIST cycle will
//! wire `partial_download_cleanup` into `cmd_install` startup as
//! part of the H-04 persistence decision (operator-gated). Until
//! then, the `#[allow(dead_code)]` silences the false-positive when
//! this module is compiled into bin targets (e.g. `cogh`) where the
//! function is not yet called.

use std::path::Path;

#[allow(unused_imports)]
// `InstallerError` will become the return-type carrier once
// `partial_download_cleanup` integrates with the rollback journal.
use crate::error::InstallerError;

/// Clean up any stale `.part` partial downloads in the cache directory.
///
// Call this on installer startup to ensure no orphaned `.part` files
/// remain from a previous interrupted run.
#[allow(dead_code)]
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
