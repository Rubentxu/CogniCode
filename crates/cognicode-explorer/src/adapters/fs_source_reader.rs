//! Filesystem-backed [`SourceReader`] adapter.
//!
//! Reads files relative to a configured root path. Out-of-range line
//! requests are clamped to whatever the file actually contains.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{ExplorerError, ExplorerResult};
use crate::ports::source_reader::SourceReader;

/// Adapter that reads source files under a root directory.
pub struct FsSourceReader {
    root: PathBuf,
}

impl FsSourceReader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Borrow the root — useful for tests and for future mtime logic.
    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl SourceReader for FsSourceReader {
    fn read_source(&self, file: &str) -> ExplorerResult<String> {
        let path = self.root.join(file);
        fs::read_to_string(&path).map_err(|e| ExplorerError::SourceUnavailable {
            file: path.display().to_string(),
            object_id: e.to_string(),
        })
    }

    fn read_lines(
        &self,
        file: &str,
        start: u32,
        end: u32,
    ) -> ExplorerResult<Vec<(u32, String)>> {
        let content = self.read_source(file)?;
        let mut out = Vec::new();
        if start == 0 || end == 0 || end < start {
            return Ok(out);
        }
        for (idx, line) in content.lines().enumerate() {
            let one_based = (idx + 1) as u32;
            if one_based < start {
                continue;
            }
            if one_based > end {
                break;
            }
            out.push((one_based, line.to_string()));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::source_reader::SourceReader;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn read_source_returns_full_contents() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("foo.rs");
        fs::write(&path, "alpha\nbeta\ngamma\n").expect("write");

        let reader = FsSourceReader::new(dir.path().to_path_buf());
        let content = reader.read_source("foo.rs").expect("read");
        assert_eq!(content, "alpha\nbeta\ngamma\n");
    }

    #[test]
    fn read_lines_returns_numbered_window() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("foo.rs");
        let body: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
        fs::write(&path, body.join("\n")).expect("write");

        let reader = FsSourceReader::new(dir.path().to_path_buf());
        let lines = reader.read_lines("foo.rs", 5, 8).expect("read");
        assert_eq!(lines, vec![(5, "line 5".into()), (6, "line 6".into()), (7, "line 7".into()), (8, "line 8".into())]);
    }

    #[test]
    fn read_lines_clamps_to_available_lines() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("foo.rs");
        fs::write(&path, "a\nb\nc\n").expect("write");

        let reader = FsSourceReader::new(dir.path().to_path_buf());
        // End is past the end of the file — only 3 lines should be returned.
        let lines = reader.read_lines("foo.rs", 1, 100).expect("read");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn read_lines_zero_range_returns_empty() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("foo.rs");
        fs::write(&path, "a\nb\n").expect("write");

        let reader = FsSourceReader::new(dir.path().to_path_buf());
        let lines = reader.read_lines("foo.rs", 0, 0).expect("read");
        assert!(lines.is_empty());
    }

    #[test]
    fn missing_file_returns_source_unavailable() {
        let dir = tempdir().expect("tempdir");
        let reader = FsSourceReader::new(dir.path().to_path_buf());
        let err = reader.read_source("does_not_exist.rs").unwrap_err();
        assert!(matches!(err, ExplorerError::SourceUnavailable { .. }));
    }
}
