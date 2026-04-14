//! FileSystem trait - Interface for virtual file system implementations

use lsp_types::Url;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during file system operations.
#[derive(Debug, Error)]
pub enum VfsError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Edit application failed: {0}")]
    EditFailed(String),

    #[error("Encoding error: {0}")]
    EncodingError(String),
}

/// Result type for file system operations.
pub type VfsResult<T> = Result<T, VfsError>;

/// Text edit representation for document changes.
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// The range to replace (zero-indexed, start inclusive, end exclusive)
    pub range: (u32, u32), // (start_offset, end_offset)
    /// The new text to insert
    pub new_text: String,
}

/// Trait for virtual file system implementations that manage in-memory file contents.
pub trait FileSystem: Send + Sync {
    /// Gets the content of a file by its URL.
    fn get_content(&self, url: &Url) -> Option<Arc<str>>;

    /// Sets the content of a file.
    fn set_content(&mut self, url: Url, content: String);

    /// Applies a batch of text edits to multiple files.
    fn apply_edits(&mut self, edits: HashMap<Url, Vec<TextEdit>>) -> VfsResult<()>;

    /// Removes a file from the file system.
    fn remove(&mut self, url: &Url) -> bool;

    /// Checks if a file exists in the file system.
    fn exists(&self, url: &Url) -> bool;

    /// Returns all known file URLs.
    fn get_all_urls(&self) -> Vec<Url>;

    /// Returns the number of files in the file system.
    fn file_count(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Url;
    use std::collections::HashMap;

    struct MockFileSystem {
        files: HashMap<Url, String>,
    }

    impl MockFileSystem {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
            }
        }
    }

    impl FileSystem for MockFileSystem {
        fn get_content(&self, url: &Url) -> Option<Arc<str>> {
            self.files.get(url).map(|s| Arc::from(s.as_str()))
        }

        fn set_content(&mut self, url: Url, content: String) {
            self.files.insert(url, content);
        }

        fn apply_edits(&mut self, edits: HashMap<Url, Vec<TextEdit>>) -> VfsResult<()> {
            for (url, text_edits) in edits {
                if let Some(content) = self.files.get_mut(&url) {
                    let mut chars: Vec<char> = content.chars().collect();
                    for edit in text_edits {
                        let (start, end) = edit.range;
                        let start = start as usize;
                        let end = end as usize;
                        if start <= chars.len() && end <= chars.len() && start <= end {
                            chars.splice(start..end, edit.new_text.chars());
                        }
                    }
                    *content = chars.into_iter().collect();
                }
            }
            Ok(())
        }

        fn remove(&mut self, url: &Url) -> bool {
            self.files.remove(url).is_some()
        }

        fn exists(&self, url: &Url) -> bool {
            self.files.contains_key(url)
        }

        fn get_all_urls(&self) -> Vec<Url> {
            self.files.keys().cloned().collect()
        }

        fn file_count(&self) -> usize {
            self.files.len()
        }
    }

    #[test]
    fn test_mock_read_file() {
        let mut fs = MockFileSystem::new();
        let url = Url::parse("file:///test.rs").unwrap();
        fs.set_content(url.clone(), "fn main() {}".to_string());

        let content = fs.get_content(&url);
        assert!(content.is_some());
        assert_eq!(&*content.unwrap(), "fn main() {}");
    }

    #[test]
    fn test_mock_read_missing() {
        let fs = MockFileSystem::new();
        let url = Url::parse("file:///missing.rs").unwrap();

        let content = fs.get_content(&url);
        assert!(content.is_none());
    }

    #[test]
    fn test_mock_write() {
        let mut fs = MockFileSystem::new();
        let url = Url::parse("file:///test.rs").unwrap();

        fs.set_content(url.clone(), "original".to_string());
        assert!(fs.exists(&url));
        assert_eq!(fs.file_count(), 1);

        fs.set_content(url.clone(), "updated".to_string());
        let content = fs.get_content(&url).unwrap();
        assert_eq!(&*content, "updated");
    }

    #[test]
    fn test_mock_exists() {
        let mut fs = MockFileSystem::new();
        let url1 = Url::parse("file:///exists.rs").unwrap();
        let url2 = Url::parse("file:///not_exists.rs").unwrap();

        fs.set_content(url1.clone(), "content".to_string());

        assert!(fs.exists(&url1));
        assert!(!fs.exists(&url2));
    }
}
