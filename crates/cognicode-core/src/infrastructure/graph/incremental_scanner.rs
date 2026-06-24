//! Incremental graph scanner — only re-parses changed files.
//!
//! Uses [`FileManifest`] to track file state (mtime + blake3 content hash)
//! and exposes a [`scan`](IncrementalScanner::scan) entry point that produces
//! a [`ScanDelta`] describing what changed since the last scan.
//!
//! Re-parsing of the `changed` files is the caller's responsibility — this
//! type only tracks *which* files need re-parsing. Once a future phase
//! wires `FileManifest` persistence to PostgreSQL, the same scanner will
//! load/save the manifest across sessions and `build_graph` will be able
//! to call into it for sub-second incremental updates.

use std::path::PathBuf;

use crate::infrastructure::graph::file_manifest::{FileManifest, ScanDelta};

/// Result of an incremental scan.
#[derive(Debug)]
pub struct IncrementalScanResult {
    /// Number of files re-parsed (changed + new).
    pub files_processed: usize,
    /// Number of files skipped (unchanged).
    pub files_skipped: usize,
    /// Number of files removed from the graph (deleted).
    pub files_removed: usize,
    /// Total files in manifest after scan.
    pub total_tracked: usize,
    /// Whether this was the first scan (no prior manifest).
    pub was_first_scan: bool,
}

/// Incremental scanner that tracks file state between scans.
pub struct IncrementalScanner {
    manifest: FileManifest,
    project_dir: PathBuf,
}

impl IncrementalScanner {
    /// Create a new scanner for the given project directory with an empty
    /// manifest. The first call to [`scan`](Self::scan) will treat every
    /// file as "changed" and produce a `was_first_scan = true` result.
    pub fn new(project_dir: PathBuf) -> Self {
        Self {
            manifest: FileManifest::new(),
            project_dir,
        }
    }

    /// Create a scanner with an existing manifest (e.g., loaded from disk
    /// or from the PostgreSQL `file_manifest` table once persistence is
    /// wired in a later phase).
    pub fn with_manifest(project_dir: PathBuf, manifest: FileManifest) -> Self {
        Self {
            manifest,
            project_dir,
        }
    }

    /// Get a reference to the current manifest.
    pub fn manifest(&self) -> &FileManifest {
        &self.manifest
    }

    /// Consume the scanner and return the inner manifest (e.g., to persist
    /// it to PostgreSQL after a scan).
    pub fn into_manifest(self) -> FileManifest {
        self.manifest
    }

    /// Perform an incremental scan. Returns the delta and scan result.
    ///
    /// The caller is responsible for:
    /// 1. Passing the list of current source files in the project
    /// 2. Re-parsing the `changed` files from the delta
    /// 3. Removing symbols from `deleted` files from the graph
    ///
    /// After this returns, the internal manifest is already up-to-date —
    /// subsequent calls will take the mtime fastpath for unchanged files.
    pub fn scan(&mut self, current_files: &[PathBuf]) -> (ScanDelta, IncrementalScanResult) {
        let was_first_scan = self.manifest.is_empty();

        let delta = self
            .manifest
            .compute_delta(&self.project_dir, current_files);

        let files_processed = delta.changed.len();
        let files_skipped = delta.unchanged.len();
        let files_removed = delta.deleted.len();

        // Update manifest with changes
        self.manifest.apply_delta(&self.project_dir, &delta.changed);
        // Refresh mtime/size on unchanged files that had a false-positive
        // mtime change (so the next delta takes the fastpath for them).
        self.manifest
            .refresh_unchanged(&self.project_dir, &delta.unchanged);
        self.manifest.remove_deleted(&delta.deleted);

        let result = IncrementalScanResult {
            files_processed,
            files_skipped,
            files_removed,
            total_tracked: self.manifest.len(),
            was_first_scan,
        };

        (delta, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    fn write(dir: &std::path::Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_first_scan_treats_all_as_changed() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}");
        write(dir.path(), "b.rs", b"fn b() {}");

        let mut scanner = IncrementalScanner::new(dir.path().to_path_buf());
        let (delta, result) = scanner.scan(&[PathBuf::from("a.rs"), PathBuf::from("b.rs")]);

        assert!(result.was_first_scan);
        assert_eq!(result.files_processed, 2);
        assert_eq!(result.files_skipped, 0);
        assert_eq!(result.files_removed, 0);
        assert_eq!(result.total_tracked, 2);
        assert_eq!(delta.changed.len(), 2);
        assert_eq!(delta.deleted.len(), 0);
        assert_eq!(delta.unchanged.len(), 0);
    }

    #[test]
    fn test_second_scan_skips_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}");
        write(dir.path(), "b.rs", b"fn b() {}");

        let mut scanner = IncrementalScanner::new(dir.path().to_path_buf());
        scanner.scan(&[PathBuf::from("a.rs"), PathBuf::from("b.rs")]);

        // Second scan with the same files: everything should be unchanged.
        let (delta, result) = scanner.scan(&[PathBuf::from("a.rs"), PathBuf::from("b.rs")]);
        assert!(!result.was_first_scan);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.files_skipped, 2);
        assert_eq!(result.files_removed, 0);
        assert_eq!(result.total_tracked, 2);
        assert_eq!(delta.unchanged.len(), 2);
        assert_eq!(delta.changed.len(), 0);
    }

    #[test]
    fn test_scan_detects_modification() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}");

        let mut scanner = IncrementalScanner::new(dir.path().to_path_buf());
        scanner.scan(&[PathBuf::from("a.rs")]);

        thread::sleep(Duration::from_millis(20));
        write(dir.path(), "a.rs", b"fn a() { 1 }");

        let (delta, result) = scanner.scan(&[PathBuf::from("a.rs")]);
        assert_eq!(result.files_processed, 1);
        assert_eq!(result.files_skipped, 0);
        assert_eq!(delta.changed.len(), 1);
        assert_eq!(delta.unchanged.len(), 0);
    }

    #[test]
    fn test_scan_detects_deletion() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}");
        write(dir.path(), "b.rs", b"fn b() {}");

        let mut scanner = IncrementalScanner::new(dir.path().to_path_buf());
        scanner.scan(&[PathBuf::from("a.rs"), PathBuf::from("b.rs")]);

        fs::remove_file(dir.path().join("b.rs")).unwrap();

        let (delta, result) = scanner.scan(&[PathBuf::from("a.rs")]);
        assert_eq!(result.files_removed, 1);
        assert_eq!(result.files_skipped, 1);
        assert_eq!(result.total_tracked, 1);
        assert_eq!(delta.deleted.len(), 1);
        assert!(delta.deleted.contains(&PathBuf::from("b.rs")));
    }

    #[test]
    fn test_with_manifest_preserves_state() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}");

        // First scanner populates the manifest
        let mut scanner1 = IncrementalScanner::new(dir.path().to_path_buf());
        scanner1.scan(&[PathBuf::from("a.rs")]);
        let manifest = scanner1.into_manifest();

        // Second scanner constructed from the first's manifest
        let mut scanner2 = IncrementalScanner::with_manifest(dir.path().to_path_buf(), manifest);
        let (_, result) = scanner2.scan(&[PathBuf::from("a.rs")]);
        assert!(!result.was_first_scan);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.files_skipped, 1);
        assert_eq!(result.total_tracked, 1);
    }

    #[test]
    fn test_mixed_scan() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.rs", b"fn a() {}");
        write(dir.path(), "b.rs", b"fn b() {}");
        write(dir.path(), "c.rs", b"fn c() {}");

        let mut scanner = IncrementalScanner::new(dir.path().to_path_buf());
        scanner.scan(&[
            PathBuf::from("a.rs"),
            PathBuf::from("b.rs"),
            PathBuf::from("c.rs"),
        ]);

        thread::sleep(Duration::from_millis(20));
        // Modify a, leave b, delete c, add d
        write(dir.path(), "a.rs", b"fn a() { 1 }");
        fs::remove_file(dir.path().join("c.rs")).unwrap();
        write(dir.path(), "d.rs", b"fn d() {}");

        let (delta, result) = scanner.scan(&[
            PathBuf::from("a.rs"),
            PathBuf::from("b.rs"),
            PathBuf::from("d.rs"),
        ]);
        assert_eq!(result.files_processed, 2); // a modified + d new
        assert_eq!(result.files_skipped, 1); // b unchanged
        assert_eq!(result.files_removed, 1); // c deleted
        assert_eq!(result.total_tracked, 3); // a, b, d

        assert!(delta.changed.contains(&PathBuf::from("a.rs")));
        assert!(delta.changed.contains(&PathBuf::from("d.rs")));
        assert!(delta.unchanged.contains(&PathBuf::from("b.rs")));
        assert!(delta.deleted.contains(&PathBuf::from("c.rs")));
    }
}
