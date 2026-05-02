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
