//! Per-file graph cache for modular graph construction
//!
//! This module provides a cache that stores a CallGraph per file, allowing
//! for modular graph construction and merging. This is useful when you need
//! to analyze individual files and then combine them into larger graphs.

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::value_objects::{DependencyType, Location};
use crate::infrastructure::parser::{Language, TreeSitterParser};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Cache entry for per-file graphs
struct FileGraphCacheEntry {
    /// The call graph for this file
    graph: CallGraph,
    /// Whether this entry is valid
    valid: bool,
}

/// Per-file graph cache that stores CallGraph per file
///
/// This cache allows for modular graph construction where each file's
/// call graph is built independently and can be merged on demand.
pub struct PerFileGraphCache {
    /// Map from file path to cached graph
    cache: RwLock<HashMap<String, FileGraphCacheEntry>>,
    /// Project directory for relative paths
    project_dir: Option<String>,
}

impl PerFileGraphCache {
    /// Creates a new empty PerFileGraphCache
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            project_dir: None,
        }
    }

    /// Creates a new PerFileGraphCache with a project directory
    pub fn with_project_dir(project_dir: impl Into<String>) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            project_dir: Some(project_dir.into()),
        }
    }

    /// Gets the graph for a file, building it if needed
    ///
    /// If the file has already been parsed and cached, returns the cached version.
    /// Otherwise, parses the file and builds the local call graph.
    pub fn get_or_build(&self, file_path: &Path) -> std::io::Result<Arc<CallGraph>> {
        let path_str = file_path.to_string_lossy().to_string();

        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(entry) = cache.get(&path_str) {
                if entry.valid {
                    return Ok(Arc::new(entry.graph.clone()));
                }
            }
        }

        // Build the graph for this file
        let graph = self.build_file_graph(&path_str)?;

        // Cache it
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(
                path_str,
                FileGraphCacheEntry {
                    graph: graph.clone(),
                    valid: true,
                },
            );
        }

        Ok(Arc::new(graph))
    }

    /// Gets a cached graph without building if missing
    pub fn get_cached(&self, file_path: &Path) -> Option<Arc<CallGraph>> {
        let path_str = file_path.to_string_lossy().to_string();
        let cache = self.cache.read().unwrap();
        cache
            .get(&path_str)
            .map(|entry| Arc::new(entry.graph.clone()))
    }

    /// Invalidates the cache entry for a specific file
    pub fn invalidate(&self, file_path: &Path) {
        let path_str = file_path.to_string_lossy().to_string();
        if let Ok(mut cache) = self.cache.write() {
            if let Some(entry) = cache.get_mut(&path_str) {
                entry.valid = false;
            }
        }
    }

    /// Clears all cached entries
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Clears all cached entries and marks them as needing rebuild
    pub fn clear_all(&self) {
        if let Ok(mut cache) = self.cache.write() {
            for entry in cache.values_mut() {
                entry.valid = false;
            }
        }
    }

    /// Merges multiple file graphs into a single graph
    ///
    /// Takes a list of file paths and returns a merged CallGraph containing
    /// all symbols and dependencies from all the files.
    pub fn merge(&self, file_paths: &[&Path]) -> CallGraph {
        let mut merged = CallGraph::new();

        for path in file_paths {
            let path_str = path.to_string_lossy().to_string();

            // Get from cache or build
            let graph = match self.get_cached(path) {
                Some(g) => (*g).clone(),
                None => self
                    .build_file_graph(&path_str)
                    .unwrap_or_else(|_| CallGraph::new()),
            };

            // Merge symbols
            for symbol in graph.symbols() {
                let new_symbol = Symbol::new(
                    symbol.name(),
                    *symbol.kind(),
                    Location::new(
                        symbol.location().file(),
                        symbol.location().line(),
                        symbol.location().column(),
                    ),
                );
                merged.add_symbol(new_symbol);
            }

            // Merge edges
            for (source_id, target_id, dep_type) in graph.all_dependencies() {
                // Re-create IDs with proper format
                let source_symbol = graph.get_symbol(source_id);
                let target_symbol = graph.get_symbol(target_id);

                if let (Some(src), Some(tgt)) = (source_symbol, target_symbol) {
                    let new_source_id = crate::domain::aggregates::call_graph::SymbolId::new(
                        src.fully_qualified_name(),
                    );
                    let new_target_id = crate::domain::aggregates::call_graph::SymbolId::new(
                        tgt.fully_qualified_name(),
                    );
                    let _ = merged.add_dependency(&new_source_id, &new_target_id, *dep_type);
                }
            }
        }

        merged
    }

    /// Merges all cached file graphs into a single graph
    ///
    /// Only includes files that have been cached and are valid.
    pub fn merge_all(&self) -> CallGraph {
        let paths: Vec<String> = {
            let cache = self.cache.read().unwrap();
            cache
                .iter()
                .filter(|(_, entry)| entry.valid)
                .map(|(path, _)| path.clone())
                .collect()
        };

        let path_refs: Vec<&Path> = paths.iter().map(Path::new).collect();
        self.merge(&path_refs)
    }

    /// Builds a call graph for a single file
    fn build_file_graph(&self, file_path: &str) -> std::io::Result<CallGraph> {
        let source = std::fs::read_to_string(file_path)?;

        let language =
            Language::from_extension(Path::new(file_path).extension()).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Unsupported file type: {}", file_path),
                )
            })?;

        let parser = TreeSitterParser::new(language)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let symbols = parser
            .find_all_symbols_with_path(&source, file_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let relationships = parser
            .find_call_relationships(&source, file_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut graph = CallGraph::new();
        let mut name_to_symbol: HashMap<String, crate::domain::aggregates::call_graph::SymbolId> =
            HashMap::new();

        // Add symbols to graph
        for symbol in symbols {
            let symbol_id = graph.add_symbol(symbol.clone());
            name_to_symbol.insert(symbol.name().to_lowercase(), symbol_id);
        }

        // Add call relationships
        for (caller, callee_name) in relationships {
            let caller_id =
                crate::domain::aggregates::call_graph::SymbolId::new(caller.fully_qualified_name());

            if let Some(callee_id) = name_to_symbol.get(&callee_name.to_lowercase()).cloned() {
                let _ = graph.add_dependency(&caller_id, &callee_id, DependencyType::Calls);
            }
        }

        Ok(graph)
    }

    /// Returns the number of cached files
    pub fn cached_file_count(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.len()
    }

    /// Returns the number of valid (non-stale) cached entries
    pub fn valid_entry_count(&self) -> usize {
        let cache = self.cache.read().unwrap();
        cache.values().filter(|e| e.valid).count()
    }
}

impl Default for PerFileGraphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PerFileGraphCache {
    fn clone(&self) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()), // Don't clone cache
            project_dir: self.project_dir.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_per_file_graph_cache_empty() {
        let cache = PerFileGraphCache::new();
        assert_eq!(cache.cached_file_count(), 0);
        assert_eq!(cache.valid_entry_count(), 0);
    }

    #[test]
    fn test_per_file_graph_cache_build() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "class MyClass:").unwrap();
        writeln!(file, "    def method(self):").unwrap();
        writeln!(file, "        pass").unwrap();

        let cache = PerFileGraphCache::new();
        let graph = cache.get_or_build(file.path()).unwrap();

        assert!(graph.symbol_count() >= 2);
    }

    #[test]
    fn test_per_file_graph_cache_cached() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let cache = PerFileGraphCache::new();

        // First call builds
        let graph1 = cache.get_or_build(file.path()).unwrap();

        // Second call should return cached
        let graph2 = cache.get_or_build(file.path()).unwrap();

        assert_eq!(graph1.symbol_count(), graph2.symbol_count());
        assert_eq!(cache.cached_file_count(), 1);
    }

    #[test]
    fn test_per_file_graph_cache_invalidate() {
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();

        let cache = PerFileGraphCache::new();
        cache.get_or_build(file.path()).unwrap();

        assert_eq!(cache.valid_entry_count(), 1);

        cache.invalidate(file.path());

        assert_eq!(cache.valid_entry_count(), 0);
    }

    #[test]
    fn test_per_file_graph_cache_merge() {
        let mut file1 = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file1, "def a():").unwrap();
        writeln!(file1, "    b()").unwrap();
        writeln!(file1).unwrap();
        writeln!(file1, "def b():").unwrap();
        writeln!(file1, "    pass").unwrap();

        let mut file2 = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file2, "def c():").unwrap();
        writeln!(file2, "    pass").unwrap();

        let cache = PerFileGraphCache::new();
        let merged = cache.merge(&[file1.path(), file2.path()]);

        // Should have symbols from both files
        assert!(merged.symbol_count() >= 3);
    }
}
