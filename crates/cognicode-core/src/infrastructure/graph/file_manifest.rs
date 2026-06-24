//! File manifest for incremental graph scanning.
//!
//! Tracks file state (mtime, content hash) to detect changes between scans.
//! Uses blake3 for fast content hashing with mtime fastpath: if the size and
//! modification time are unchanged, the content hash is assumed to match and
//! the file is skipped without being re-read. Only when mtime or size changes
//! is the blake3 hash recomputed to confirm the change.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Record for a single file in the manifest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileRecord {
    /// Relative path from project root.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Last modified time as nanoseconds since epoch.
    pub mtime_ns: u64,
    /// Blake3 content hash (hex-encoded).
    pub content_hash: String,
    /// When this record was last scanned (epoch nanoseconds).
    pub scanned_at_ns: u64,
}

/// Manifest tracking file state for incremental scanning.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FileManifest {
    /// Map from relative path to file record.
    files: HashMap<PathBuf, FileRecord>,
}

/// Result of comparing current filesystem state against the manifest.
#[derive(Debug)]
pub struct ScanDelta {
    /// Files that are new or have different content hash.
    pub changed: Vec<PathBuf>,
    /// Files that were deleted since last scan.
    pub deleted: Vec<PathBuf>,
    /// Files that are unchanged (same hash, skipped).
    pub unchanged: Vec<PathBuf>,
}

impl FileManifest {
    /// Create a new empty manifest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a file record by path.
    pub fn get(&self, path: &Path) -> Option<&FileRecord> {
        self.files.get(path)
    }

    /// Upsert a file record.
    pub fn upsert(&mut self, record: FileRecord) {
        self.files.insert(record.path.clone(), record);
    }

    /// Remove a file record.
    pub fn remove(&mut self, path: &Path) -> Option<FileRecord> {
        self.files.remove(path)
    }

    /// Number of tracked files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the manifest is empty.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// All tracked file paths.
    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.keys()
    }

    /// Compute delta between current filesystem state and manifest.
    ///
    /// Uses mtime fastpath: if size + mtime haven't changed, skip the hash.
    /// Only computes blake3 hash when mtime or size changed.
    pub fn compute_delta(&self, project_dir: &Path, current_files: &[PathBuf]) -> ScanDelta {
        let mut changed = Vec::new();
        let mut deleted = Vec::new();
        let mut unchanged = Vec::new();

        // Detect deleted files: anything in the manifest that isn't in the
        // current file list.
        let current_set: HashSet<&PathBuf> = current_files.iter().collect();
        for path in self.files.keys() {
            if !current_set.contains(path) {
                deleted.push(path.clone());
            }
        }

        // Detect changed/unchanged files
        for rel_path in current_files {
            let abs_path = project_dir.join(rel_path);
            let metadata = match std::fs::metadata(&abs_path) {
                Ok(m) => m,
                Err(_) => {
                    // File missing or unreadable — treat as changed so the
                    // caller can decide what to do (the apply step will
                    // re-skip it if it's still unreachable).
                    changed.push(rel_path.clone());
                    continue;
                }
            };

            let size = metadata.len();
            let mtime_ns = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);

            if let Some(record) = self.files.get(rel_path) {
                // Mtime fastpath: if size + mtime match, assume unchanged
                if record.size == size && record.mtime_ns == mtime_ns {
                    unchanged.push(rel_path.clone());
                    continue;
                }

                // Mtime or size changed — verify with content hash
                let content_hash = Self::hash_file(&abs_path).unwrap_or_default();
                if content_hash == record.content_hash {
                    // False positive — content is the same. We don't push to
                    // `unchanged` for the caller's re-parse accounting, but
                    // we DO want the record's mtime refreshed so the next
                    // delta takes the fastpath. The caller can decide.
                    unchanged.push(rel_path.clone());
                } else {
                    changed.push(rel_path.clone());
                }
            } else {
                // New file — never seen in the manifest before.
                changed.push(rel_path.clone());
            }
        }

        ScanDelta {
            changed,
            deleted,
            unchanged,
        }
    }

    /// Update manifest with scan results. Hashes and records metadata for
    /// every path in `changed` and inserts the new `FileRecord`.
    pub fn apply_delta(&mut self, project_dir: &Path, changed: &[PathBuf]) {
        let now_ns = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        for rel_path in changed {
            let abs_path = project_dir.join(rel_path);
            let metadata = match std::fs::metadata(&abs_path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let content_hash = Self::hash_file(&abs_path).unwrap_or_default();
            let mtime_ns = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);

            self.upsert(FileRecord {
                path: rel_path.clone(),
                size: metadata.len(),
                mtime_ns,
                content_hash,
                scanned_at_ns: now_ns,
            });
        }
    }

    /// Refresh mtime/size for files in `unchanged` where the fastpath missed
    /// (false-positive mtime change but content unchanged). The next delta
    /// will then take the fastpath.
    pub fn refresh_unchanged(&mut self, project_dir: &Path, unchanged: &[PathBuf]) {
        for rel_path in unchanged {
            if let Some(record) = self.files.get_mut(rel_path) {
                let abs_path = project_dir.join(rel_path);
                if let Ok(metadata) = std::fs::metadata(&abs_path) {
                    let mtime_ns = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(record.mtime_ns);
                    record.size = metadata.len();
                    record.mtime_ns = mtime_ns;
                }
            }
        }
    }

    /// Remove deleted files from manifest.
    pub fn remove_deleted(&mut self, deleted: &[PathBuf]) {
        for path in deleted {
            self.remove(path);
        }
    }

    /// Hash a file using blake3.
    fn hash_file(path: &Path) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let hash = blake3::hash(&bytes);
        Some(hash.to_hex().to_string())
    }

    /// Serialize manifest to JSON bytes.
    pub fn to_json_bytes(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    /// Deserialize manifest from JSON bytes.
    pub fn from_json_bytes(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    fn create_temp_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_empty_manifest() {
        let manifest = FileManifest::new();
        assert!(manifest.is_empty());
        assert_eq!(manifest.len(), 0);
        assert_eq!(manifest.paths().count(), 0);
    }

    #[test]
    fn test_upsert_and_get() {
        let mut manifest = FileManifest::new();
        let record = FileRecord {
            path: PathBuf::from("src/main.rs"),
            size: 100,
            mtime_ns: 1_000_000,
            content_hash: "abc123".to_string(),
            scanned_at_ns: 2_000_000,
        };
        manifest.upsert(record.clone());
        assert_eq!(manifest.len(), 1);
        assert!(!manifest.is_empty());
        let stored = manifest.get(Path::new("src/main.rs")).unwrap();
        assert_eq!(stored.size, 100);
        assert_eq!(stored.content_hash, "abc123");
    }

    #[test]
    fn test_upsert_overwrites() {
        let mut manifest = FileManifest::new();
        manifest.upsert(FileRecord {
            path: PathBuf::from("a.rs"),
            size: 10,
            mtime_ns: 1,
            content_hash: "h1".to_string(),
            scanned_at_ns: 1,
        });
        manifest.upsert(FileRecord {
            path: PathBuf::from("a.rs"),
            size: 20,
            mtime_ns: 2,
            content_hash: "h2".to_string(),
            scanned_at_ns: 2,
        });
        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest.get(Path::new("a.rs")).unwrap().size, 20);
    }

    #[test]
    fn test_remove() {
        let mut manifest = FileManifest::new();
        manifest.upsert(FileRecord {
            path: PathBuf::from("a.rs"),
            size: 10,
            mtime_ns: 1,
            content_hash: "h".to_string(),
            scanned_at_ns: 1,
        });
        let removed = manifest.remove(Path::new("a.rs"));
        assert!(removed.is_some());
        assert!(manifest.is_empty());
    }

    #[test]
    fn test_compute_delta_all_new() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn main() {}");
        create_temp_file(dir.path(), "b.rs", b"fn foo() {}");

        let manifest = FileManifest::new();
        let files = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        let delta = manifest.compute_delta(dir.path(), &files);

        assert_eq!(delta.changed.len(), 2);
        assert_eq!(delta.deleted.len(), 0);
        assert_eq!(delta.unchanged.len(), 0);
        assert!(delta.changed.contains(&PathBuf::from("a.rs")));
        assert!(delta.changed.contains(&PathBuf::from("b.rs")));
    }

    #[test]
    fn test_compute_delta_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn main() {}");

        // First scan
        let mut manifest = FileManifest::new();
        manifest.apply_delta(dir.path(), &[PathBuf::from("a.rs")]);
        assert_eq!(manifest.len(), 1);

        // Second scan — file unchanged
        let delta = manifest.compute_delta(dir.path(), &[PathBuf::from("a.rs")]);
        assert_eq!(delta.unchanged.len(), 1);
        assert_eq!(delta.changed.len(), 0);
        assert_eq!(delta.deleted.len(), 0);
    }

    #[test]
    fn test_compute_delta_modified() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn main() {}");

        let mut manifest = FileManifest::new();
        manifest.apply_delta(dir.path(), &[PathBuf::from("a.rs")]);

        // Modify file
        create_temp_file(dir.path(), "a.rs", b"fn main() { println!(\"hello\"); }");

        // Wait briefly to ensure mtime changes (filesystem mtime resolution)
        thread::sleep(Duration::from_millis(20));

        let delta = manifest.compute_delta(dir.path(), &[PathBuf::from("a.rs")]);
        assert_eq!(delta.changed.len(), 1);
        assert_eq!(delta.unchanged.len(), 0);
    }

    #[test]
    fn test_compute_delta_deleted() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn main() {}");
        create_temp_file(dir.path(), "b.rs", b"fn foo() {}");

        let mut manifest = FileManifest::new();
        manifest.apply_delta(dir.path(), &[PathBuf::from("a.rs"), PathBuf::from("b.rs")]);

        // Delete b.rs
        fs::remove_file(dir.path().join("b.rs")).unwrap();

        let delta = manifest.compute_delta(dir.path(), &[PathBuf::from("a.rs")]);
        assert_eq!(delta.deleted.len(), 1);
        assert!(delta.deleted.contains(&PathBuf::from("b.rs")));
        assert_eq!(delta.unchanged.len(), 1);
    }

    #[test]
    fn test_compute_delta_mixed() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn a() {}");
        create_temp_file(dir.path(), "b.rs", b"fn b() {}");
        create_temp_file(dir.path(), "c.rs", b"fn c() {}");

        let mut manifest = FileManifest::new();
        manifest.apply_delta(
            dir.path(),
            &[
                PathBuf::from("a.rs"),
                PathBuf::from("b.rs"),
                PathBuf::from("c.rs"),
            ],
        );

        // Modify a.rs, leave b.rs, delete c.rs, add d.rs
        thread::sleep(Duration::from_millis(20));
        create_temp_file(dir.path(), "a.rs", b"fn a() { 1 }");
        fs::remove_file(dir.path().join("c.rs")).unwrap();
        create_temp_file(dir.path(), "d.rs", b"fn d() {}");

        let delta = manifest.compute_delta(
            dir.path(),
            &[
                PathBuf::from("a.rs"),
                PathBuf::from("b.rs"),
                PathBuf::from("d.rs"),
            ],
        );

        assert_eq!(delta.changed.len(), 2); // a.rs (modified) + d.rs (new)
        assert!(delta.changed.contains(&PathBuf::from("a.rs")));
        assert!(delta.changed.contains(&PathBuf::from("d.rs")));
        assert_eq!(delta.unchanged.len(), 1);
        assert!(delta.unchanged.contains(&PathBuf::from("b.rs")));
        assert_eq!(delta.deleted.len(), 1);
        assert!(delta.deleted.contains(&PathBuf::from("c.rs")));
    }

    #[test]
    fn test_apply_delta_records_hash() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn main() {}");

        let mut manifest = FileManifest::new();
        manifest.apply_delta(dir.path(), &[PathBuf::from("a.rs")]);

        let record = manifest.get(Path::new("a.rs")).unwrap();
        // blake3 of "fn main() {}" is deterministic; just check non-empty hex
        assert!(!record.content_hash.is_empty());
        // 64 hex chars (256 bits) for blake3
        assert_eq!(record.content_hash.len(), 64);
    }

    #[test]
    fn test_remove_deleted() {
        let dir = tempfile::tempdir().unwrap();
        create_temp_file(dir.path(), "a.rs", b"fn a() {}");
        create_temp_file(dir.path(), "b.rs", b"fn b() {}");

        let mut manifest = FileManifest::new();
        manifest.apply_delta(dir.path(), &[PathBuf::from("a.rs"), PathBuf::from("b.rs")]);
        assert_eq!(manifest.len(), 2);

        manifest.remove_deleted(&[PathBuf::from("a.rs")]);
        assert_eq!(manifest.len(), 1);
        assert!(manifest.get(Path::new("a.rs")).is_none());
        assert!(manifest.get(Path::new("b.rs")).is_some());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut manifest = FileManifest::new();
        manifest.upsert(FileRecord {
            path: PathBuf::from("src/main.rs"),
            size: 100,
            mtime_ns: 1_000_000,
            content_hash: "abc123".to_string(),
            scanned_at_ns: 2_000_000,
        });
        manifest.upsert(FileRecord {
            path: PathBuf::from("src/lib.rs"),
            size: 200,
            mtime_ns: 1_500_000,
            content_hash: "def456".to_string(),
            scanned_at_ns: 2_500_000,
        });

        let bytes = manifest.to_json_bytes().unwrap();
        let restored = FileManifest::from_json_bytes(&bytes).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(
            restored.get(Path::new("src/main.rs")).unwrap().content_hash,
            "abc123"
        );
        assert_eq!(
            restored.get(Path::new("src/lib.rs")).unwrap().content_hash,
            "def456"
        );
    }

    #[test]
    fn test_serialization_empty_roundtrip() {
        let manifest = FileManifest::new();
        let bytes = manifest.to_json_bytes().unwrap();
        let restored = FileManifest::from_json_bytes(&bytes).unwrap();
        assert!(restored.is_empty());
    }

    #[test]
    fn test_blake3_hash_deterministic_and_unique() {
        // Same content must hash to the same value across calls; different
        // content must hash to different values. This is the core invariant
        // the mtime fastpath relies on after the fallback to content hash.
        let dir = tempfile::tempdir().unwrap();
        let p1 = create_temp_file(dir.path(), "x1.rs", b"fn main() {}");
        let p2 = create_temp_file(dir.path(), "x2.rs", b"fn main() {}");
        let p3 = create_temp_file(dir.path(), "y.rs", b"fn main() { 1 }");

        let h1 = FileManifest::hash_file(&p1).unwrap();
        let h2 = FileManifest::hash_file(&p2).unwrap();
        let h3 = FileManifest::hash_file(&p3).unwrap();

        // Identical content → identical hash
        assert_eq!(h1, h2);
        // Different content → different hash
        assert_ne!(h1, h3);
        // blake3 hex output is 64 chars (256 bits)
        assert_eq!(h1.len(), 64);
        // Hash should only contain hex characters
        assert!(h1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_blake3_changes_when_content_changes() {
        // End-to-end: re-write a file with different content and confirm the
        // manifest detects the change. This is the property the scanner needs
        // to be correct about.
        let dir = tempfile::tempdir().unwrap();
        let path_a = create_temp_file(dir.path(), "a.rs", b"v1");
        let mut manifest = FileManifest::new();
        manifest.apply_delta(dir.path(), &[PathBuf::from("a.rs")]);
        let hash_v1 = manifest
            .get(Path::new("a.rs"))
            .unwrap()
            .content_hash
            .clone();

        // Change content; sleep so mtime resolution advances.
        thread::sleep(Duration::from_millis(20));
        fs::write(&path_a, b"v2").unwrap();

        let delta = manifest.compute_delta(dir.path(), &[PathBuf::from("a.rs")]);
        assert_eq!(delta.changed.len(), 1);

        manifest.apply_delta(dir.path(), &delta.changed);
        let hash_v2 = manifest
            .get(Path::new("a.rs"))
            .unwrap()
            .content_hash
            .clone();
        assert_ne!(hash_v1, hash_v2);
    }
}
