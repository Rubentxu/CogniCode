//! File manifest for tracking indexed files and their content hashes
//!
//! This value object tracks all indexed files in a project, enabling
//! efficient change detection for incremental indexing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

/// Manifest tracking all indexed files and their metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileManifest {
    /// Map from file path (relative to project root) to file entry
    pub entries: HashMap<PathBuf, FileEntry>,
    /// The project root this manifest belongs to
    pub project_root: PathBuf,
}

/// Entry for a single file in the manifest
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Blake3 hash of file content
    pub content_hash: String,
    /// Last modified time (from filesystem)
    pub mtime: u64,
    /// Number of symbols in this file
    pub symbol_count: usize,
}

impl FileManifest {
    /// Create a new empty manifest for a project root
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            entries: HashMap::new(),
            project_root,
        }
    }

    /// Detect changes between this manifest and actual filesystem
    ///
    /// Returns (new_files, modified_files, deleted_files)
    ///
    /// # Arguments
    /// * `files` - Slice of tuples containing (path, content_hash, mtime)
    pub fn detect_changes(
        &self,
        files: &[(PathBuf, String, u64)],
    ) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
        let mut new_files = Vec::new();
        let mut modified_files = Vec::new();

        // Check each file from the filesystem
        for (path, content_hash, _mtime) in files {
            match self.entries.get(path) {
                Some(entry) => {
                    // File exists in manifest - check if modified
                    if entry.content_hash != *content_hash {
                        modified_files.push(path.clone());
                    }
                }
                None => {
                    // File doesn't exist in manifest - it's new
                    new_files.push(path.clone());
                }
            }
        }

        // Find deleted files (in manifest but not on filesystem)
        let filesystem_paths: std::collections::HashSet<&PathBuf> =
            files.iter().map(|(p, _, _)| p).collect();
        let deleted_files: Vec<PathBuf> = self
            .entries
            .keys()
            .filter(|path| !filesystem_paths.contains(path))
            .cloned()
            .collect();

        (new_files, modified_files, deleted_files)
    }

    /// Update entries from a list of files with their hashes
    ///
    /// # Arguments
    /// * `files` - Slice of tuples containing (path, content_hash, mtime, symbol_count)
    pub fn update_entries(&mut self, files: &[(PathBuf, String, u64, usize)]) {
        for (path, content_hash, mtime, symbol_count) in files {
            self.entries.insert(
                path.clone(),
                FileEntry {
                    content_hash: content_hash.clone(),
                    mtime: *mtime,
                    symbol_count: *symbol_count,
                },
            );
        }
    }

    /// Remove entries for deleted files
    pub fn remove_entries(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.entries.remove(path);
        }
    }

    /// Get entry for a file
    pub fn get(&self, path: &Path) -> Option<&FileEntry> {
        self.entries.get(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_manifest_detects_all_files_as_new() {
        let manifest = FileManifest::new(PathBuf::from("/project"));
        let files = vec![
            (PathBuf::from("src/main.rs"), "hash1".to_string(), 1000),
            (PathBuf::from("src/lib.rs"), "hash2".to_string(), 2000),
        ];

        let (new_files, modified_files, deleted_files) = manifest.detect_changes(&files);

        assert_eq!(new_files.len(), 2);
        assert!(new_files.contains(&PathBuf::from("src/main.rs")));
        assert!(new_files.contains(&PathBuf::from("src/lib.rs")));
        assert!(modified_files.is_empty());
        assert!(deleted_files.is_empty());
    }

    #[test]
    fn test_detect_new_modifed_deleted() {
        let mut manifest = FileManifest::new(PathBuf::from("/project"));
        manifest.update_entries(&[
            (PathBuf::from("src/main.rs"), "same_hash".to_string(), 1000, 5),
            (PathBuf::from("src/lib.rs"), "old_hash".to_string(), 1000, 3),
            (PathBuf::from("src/dead.rs"), "dead_hash".to_string(), 1000, 2),
        ]);

        // Now filesystem has: main.rs (same hash), lib.rs (modified), new.rs (new)
        // dead.rs no longer exists
        let files = vec![
            (PathBuf::from("src/main.rs"), "same_hash".to_string(), 1000),
            (PathBuf::from("src/lib.rs"), "new_hash".to_string(), 2000),
            (PathBuf::from("src/new.rs"), "new_file_hash".to_string(), 3000),
        ];

        let (new_files, modified_files, deleted_files) = manifest.detect_changes(&files);

        assert_eq!(new_files, vec![PathBuf::from("src/new.rs")]);
        assert_eq!(modified_files, vec![PathBuf::from("src/lib.rs")]);
        assert_eq!(deleted_files, vec![PathBuf::from("src/dead.rs")]);
    }

    #[test]
    fn test_update_entries_adds_and_updates() {
        let mut manifest = FileManifest::new(PathBuf::from("/project"));

        // Add a new entry
        manifest.update_entries(&[(
            PathBuf::from("src/main.rs"),
            "hash1".to_string(),
            1000,
            5,
        )]);
        assert_eq!(manifest.get(&PathBuf::from("src/main.rs")).unwrap().symbol_count, 5);

        // Update the same entry
        manifest.update_entries(&[(
            PathBuf::from("src/main.rs"),
            "hash2".to_string(),
            2000,
            10,
        )]);
        let entry = manifest.get(&PathBuf::from("src/main.rs")).unwrap();
        assert_eq!(entry.content_hash, "hash2");
        assert_eq!(entry.mtime, 2000);
        assert_eq!(entry.symbol_count, 10);
    }

    #[test]
    fn test_remove_entries() {
        let mut manifest = FileManifest::new(PathBuf::from("/project"));
        manifest.update_entries(&[
            (PathBuf::from("src/a.rs"), "hash_a".to_string(), 1000, 1),
            (PathBuf::from("src/b.rs"), "hash_b".to_string(), 1000, 2),
            (PathBuf::from("src/c.rs"), "hash_c".to_string(), 1000, 3),
        ]);

        manifest.remove_entries(&[PathBuf::from("src/b.rs")]);

        assert!(manifest.get(&PathBuf::from("src/a.rs")).is_some());
        assert!(manifest.get(&PathBuf::from("src/b.rs")).is_none());
        assert!(manifest.get(&PathBuf::from("src/c.rs")).is_some());
    }

    #[test]
    fn test_get_returns_none_for_missing_path() {
        let manifest = FileManifest::new(PathBuf::from("/project"));
        assert!(manifest.get(Path::new("nonexistent.rs")).is_none());
    }
}