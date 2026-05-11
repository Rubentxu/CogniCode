//! Layout caching for incremental regeneration using blake3 hash.
//!
//! Caches layouts in `.cognicode/layout.cache` to avoid recomputing
//! layouts for unchanged workspaces.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::layout::types::LayoutedDiagram;
use crate::model::workspace::C4Workspace;

/// Cache entry stored in `.cognicode/layout.cache`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCacheEntry {
    /// Hash of the serialized workspace
    pub workspace_hash: String,
    /// The cached layout
    pub layout: LayoutedDiagram,
    /// Timestamp when the entry was created (milliseconds since epoch)
    pub timestamp: u64,
}

/// Cache manager for layout results
pub struct LayoutCache {
    /// Directory where cache files are stored
    cache_dir: PathBuf,
}

impl LayoutCache {
    /// Create a new cache manager with the given cache directory
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Get the cache file path
    fn cache_file(&self) -> PathBuf {
        self.cache_dir.join("layout.cache")
    }

    /// Get cache entry for a workspace, if valid
    ///
    /// Returns `None` if cache doesn't exist or if the hash doesn't match.
    pub fn get(&self, workspace: &C4Workspace) -> Option<LayoutedDiagram> {
        let entry = self.load()?;
        if self.is_valid(&entry, workspace) {
            Some(entry.layout)
        } else {
            None
        }
    }

    /// Store layout for a workspace
    pub fn put(&self, workspace: &C4Workspace, layout: LayoutedDiagram) -> anyhow::Result<()> {
        let entry = LayoutCacheEntry {
            workspace_hash: Self::compute_hash(workspace),
            layout,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };
        self.save(&entry)
    }

    /// Check if cache is valid for a workspace (hash matches)
    fn is_valid(&self, entry: &LayoutCacheEntry, workspace: &C4Workspace) -> bool {
        entry.workspace_hash == Self::compute_hash(workspace)
    }

    /// Compute hash of workspace for cache key
    pub fn compute_hash(workspace: &C4Workspace) -> String {
        let json = serde_json::to_string(workspace).unwrap_or_default();
        let hash = blake3::hash(json.as_bytes());
        hash.to_hex().to_string()
    }

    /// Invalidate cache for a workspace
    ///
    /// Note: This removes the entire cache since we can't easily determine
    /// which workspace a hash corresponds to without deserializing.
    pub fn invalidate(&self, _workspace: &C4Workspace) -> anyhow::Result<()> {
        let cache_file = self.cache_file();
        if cache_file.exists() {
            std::fs::remove_file(cache_file)?;
        }
        Ok(())
    }

    /// Load cache from disk
    fn load(&self) -> Option<LayoutCacheEntry> {
        let cache_file = self.cache_file();
        if !cache_file.exists() {
            return None;
        }
        let content = std::fs::read_to_string(cache_file).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save cache to disk
    fn save(&self, entry: &LayoutCacheEntry) -> anyhow::Result<()> {
        // Ensure cache directory exists
        std::fs::create_dir_all(&self.cache_dir)?;
        let cache_file = self.cache_file();
        let content = serde_json::to_string_pretty(entry)?;
        std::fs::write(cache_file, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::types::{LayoutConfig, LayoutDirection};

    fn create_test_workspace() -> C4Workspace {
        let mut workspace = C4Workspace::new("TestSystem");
        workspace.model.people.push(crate::model::c4_types::Person {
            id: crate::model::c4_types::ElementId::new("person-1"),
            name: "Test User".to_string(),
            description: "A test user".to_string(),
            location: crate::model::c4_types::ElementLocation::Internal,
        });
        workspace
    }

    fn create_test_layout() -> LayoutedDiagram {
        LayoutedDiagram {
            nodes: vec![],
            edges: vec![],
            bounds: (0.0, 0.0, 0.0, 0.0),
            config: LayoutConfig {
                direction: LayoutDirection::TB,
                node_separation: 50.0,
                rank_separation: 80.0,
                margin: 20.0,
                min_node_width: 120.0,
                min_node_height: 60.0,
                max_node_width: 300.0,
                max_node_height: 200.0,
                orthogonal_routing: true,
                compound_padding: 30.0,
            },
        }
    }

    #[test]
    fn test_compute_hash_deterministic() {
        let workspace = create_test_workspace();
        let hash1 = LayoutCache::compute_hash(&workspace);
        let hash2 = LayoutCache::compute_hash(&workspace);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_hash_different_for_different_workspaces() {
        let workspace1 = create_test_workspace();
        let mut workspace2 = create_test_workspace();
        workspace2.name = "DifferentSystem".to_string();
        let hash1 = LayoutCache::compute_hash(&workspace1);
        let hash2 = LayoutCache::compute_hash(&workspace2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_cache_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = LayoutCache::new(temp_dir.path().to_path_buf());
        let workspace = create_test_workspace();
        let layout = create_test_layout();

        // Initially no cache
        assert!(cache.get(&workspace).is_none());

        // Store layout
        cache.put(&workspace, layout.clone()).unwrap();

        // Now we should get the cached layout
        let cached = cache.get(&workspace);
        assert!(cached.is_some());
    }

    #[test]
    fn test_cache_invalidated_on_workspace_change() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = LayoutCache::new(temp_dir.path().to_path_buf());
        let workspace = create_test_workspace();
        let layout = create_test_layout();

        cache.put(&workspace, layout.clone()).unwrap();
        assert!(cache.get(&workspace).is_some());

        // Modify workspace
        let mut modified_workspace = create_test_workspace();
        modified_workspace.name = "ModifiedSystem".to_string();

        // Cache should be invalid
        assert!(cache.get(&modified_workspace).is_none());
    }

    #[test]
    fn test_invalidate_removes_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = LayoutCache::new(temp_dir.path().to_path_buf());
        let workspace = create_test_workspace();
        let layout = create_test_layout();

        cache.put(&workspace, layout).unwrap();
        assert!(cache.get(&workspace).is_some());

        cache.invalidate(&workspace).unwrap();
        assert!(cache.get(&workspace).is_none());
    }
}
