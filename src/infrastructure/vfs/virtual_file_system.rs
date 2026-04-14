//! Virtual File System implementation

use crate::domain::traits::{FileSystem, TextEdit, VfsError, VfsResult};
use lsp_types::Url;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::traits::VfsError;

    #[test]
    fn test_new_creates_empty_vfs() {
        let vfs = VirtualFileSystem::new();
        assert_eq!(vfs.file_count(), 0);
        assert!(vfs.get_all_urls().is_empty());
    }

    #[test]
    fn test_set_content_and_get_content() {
        let mut vfs = VirtualFileSystem::new();
        let url = Url::parse("file:///test.rs").unwrap();

        vfs.set_content(url.clone(), "fn main() {}".to_string());

        let content = vfs.get_content(&url);
        assert!(content.is_some());
        assert_eq!(&*content.unwrap(), "fn main() {}");
    }

    #[test]
    fn test_get_content_nonexistent_file() {
        let vfs = VirtualFileSystem::new();
        let url = Url::parse("file:///missing.rs").unwrap();

        let content = vfs.get_content(&url);
        assert!(content.is_none());
    }

    #[test]
    fn test_apply_edits_to_file() {
        let mut vfs = VirtualFileSystem::new();
        let url = Url::parse("file:///test.rs").unwrap();

        vfs.set_content(url.clone(), "Hello World".to_string());

        let edits = vec![TextEdit {
            range: (6, 11),
            new_text: "Rust".to_string(),
        }];

        let result = vfs.apply_edits_to_file(&url, edits);
        assert!(result.is_ok());
        assert_eq!(&*result.unwrap(), "Hello Rust");
    }

    #[test]
    fn test_apply_edits_multiple() {
        let mut vfs = VirtualFileSystem::new();
        let url = Url::parse("file:///test.rs").unwrap();

        vfs.set_content(url.clone(), "Hello World".to_string());

        let edits = vec![
            TextEdit {
                range: (0, 5),
                new_text: "Hi".to_string(),
            },
            TextEdit {
                range: (5, 11),
                new_text: "Universe".to_string(),
            },
        ];

        let result = vfs.apply_edits_to_file(&url, edits);
        assert!(result.is_ok());
        assert_eq!(&*result.unwrap(), "HiUniverse");
    }

    #[test]
    fn test_empty_content() {
        let mut vfs = VirtualFileSystem::new();
        let url = Url::parse("file:///empty.rs").unwrap();

        vfs.set_content(url.clone(), "".to_string());

        let content = vfs.get_content(&url);
        assert!(content.is_some());
        assert_eq!(&*content.unwrap(), "");
    }

    #[test]
    fn test_apply_edits_to_nonexistent_file() {
        let vfs = VirtualFileSystem::new();
        let url = Url::parse("file:///missing.rs").unwrap();

        let edits = vec![TextEdit {
            range: (0, 5),
            new_text: "test".to_string(),
        }];

        let result = vfs.apply_edits_to_file(&url, edits);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VfsError::FileNotFound(_)));
    }
}

/// In-memory virtual file system
pub struct VirtualFileSystem {
    files: RwLock<HashMap<Url, Arc<str>>>,
}

impl VirtualFileSystem {
    /// Creates a new empty virtual file system
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
        }
    }

    /// Gets the content of a file
    pub fn get_content(&self, url: &Url) -> Option<Arc<str>> {
        self.files.read().ok()?.get(url).cloned()
    }

    /// Sets the content of a file
    pub fn set_content(&mut self, url: Url, content: String) {
        if let Ok(mut guard) = self.files.write() {
            guard.insert(url, Arc::from(content.into_boxed_str()));
        }
    }

    /// Removes a file
    pub fn remove(&mut self, url: &Url) -> bool {
        self.files
            .write()
            .ok()
            .map(|mut guard| guard.remove(url).is_some())
            .unwrap_or(false)
    }

    /// Checks if a file exists
    pub fn exists(&self, url: &Url) -> bool {
        self.files
            .read()
            .ok()
            .map(|guard| guard.contains_key(url))
            .unwrap_or(false)
    }

    /// Gets all file URLs
    pub fn get_all_urls(&self) -> Vec<Url> {
        self.files
            .read()
            .ok()
            .map(|guard| guard.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Gets the number of files
    pub fn file_count(&self) -> usize {
        self.files.read().ok().map(|guard| guard.len()).unwrap_or(0)
    }

    /// Applies text edits to a file
    pub fn apply_edits_to_file(&self, url: &Url, edits: Vec<TextEdit>) -> VfsResult<Arc<str>> {
        let mut files = self
            .files
            .write()
            .map_err(|_| VfsError::EditFailed("Lock poisoned".into()))?;

        let content = files
            .get_mut(url)
            .ok_or_else(|| VfsError::FileNotFound(url.to_string()))?;

        let mut content_str = content.as_ref().to_string();

        let mut sorted_edits = edits;
        sorted_edits.sort_by(|a, b| b.range.cmp(&a.range));

        for edit in sorted_edits {
            let (start, end) = Self::offset_to_position(&content_str, edit.range.0, edit.range.1);
            content_str.replace_range(start..end, &edit.new_text);
        }

        let new_content: Arc<str> = Arc::from(content_str.into_boxed_str());
        *content = new_content.clone();

        Ok(new_content)
    }

    /// Converts byte offsets to string positions
    fn offset_to_position(content: &str, start: u32, end: u32) -> (usize, usize) {
        let start = start.min(content.len() as u32) as usize;
        let end = end.min(content.len() as u32) as usize;
        (start, end)
    }
}

impl FileSystem for VirtualFileSystem {
    fn get_content(&self, url: &Url) -> Option<Arc<str>> {
        self.get_content(url)
    }

    fn set_content(&mut self, url: Url, content: String) {
        self.set_content(url, content);
    }

    fn apply_edits(&mut self, edits: HashMap<Url, Vec<TextEdit>>) -> VfsResult<()> {
        for (url, file_edits) in edits {
            self.apply_edits_to_file(&url, file_edits)?;
        }
        Ok(())
    }

    fn remove(&mut self, url: &Url) -> bool {
        self.remove(url)
    }

    fn exists(&self, url: &Url) -> bool {
        self.exists(url)
    }

    fn get_all_urls(&self) -> Vec<Url> {
        self.get_all_urls()
    }

    fn file_count(&self) -> usize {
        self.file_count()
    }
}

impl Default for VirtualFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
