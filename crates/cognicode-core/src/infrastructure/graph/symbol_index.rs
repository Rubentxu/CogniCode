//! Symbol index service with cache management
//!
//! This module provides a higher-level service that wraps the LightweightIndex
//! and adds cache management, invalidation, and query optimization.

use super::lightweight_index::{LightweightIndex, SymbolLocation};
use indexmap::IndexMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache entry with timestamp for invalidation
struct CacheEntry<T> {
    value: T,
    created_at: Instant,
}

/// Configuration for cache behavior
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum time a cache entry is valid
    pub max_age: Duration,
    /// Maximum number of entries in the cache
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_age: Duration::from_secs(300), // 5 minutes
            max_entries: 1000,
        }
    }
}

/// Symbol index service with caching capabilities
///
/// Wraps LightweightIndex with additional features:
/// - Cache management with TTL support
/// - Query result caching
/// - Cache invalidation on file changes
pub struct SymbolIndex {
    /// The underlying lightweight index
    index: LightweightIndex,
    /// Query result cache
    query_cache: RwLock<IndexMap<String, CacheEntry<Arc<Vec<SymbolLocation>>>>>,
    /// Cache configuration
    config: CacheConfig,
    /// Whether the index has been built
    built: bool,
}

impl SymbolIndex {
    /// Creates a new empty SymbolIndex with default configuration
    pub fn new() -> Self {
        Self {
            index: LightweightIndex::new(),
            query_cache: RwLock::new(IndexMap::new()),
            config: CacheConfig::default(),
            built: false,
        }
    }

    /// Creates a new SymbolIndex with custom cache configuration
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            index: LightweightIndex::new(),
            query_cache: RwLock::new(IndexMap::new()),
            config,
            built: false,
        }
    }

    /// Builds the index from a directory
    ///
    /// Scans all source files in the directory and builds the index.
    /// Returns the number of symbols indexed.
    pub fn build<P: AsRef<Path>>(&mut self, project_dir: P) -> std::io::Result<usize> {
        let project_path = project_dir.as_ref().to_path_buf();

        // Build the underlying index
        self.index.build_index(&project_path)?;
        self.built = true;

        // Clear query cache since index changed
        self.clear_query_cache();

        let count = self.index.symbol_count();
        Ok(count)
    }

    /// Builds the index from in-memory sources (useful for testing)
    pub fn build_from_sources<'a, I>(&mut self, sources: I) -> usize
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        self.index.build_from_sources(sources);
        self.built = true;
        self.clear_query_cache();
        self.index.symbol_count()
    }

    /// Queries the index for symbol locations
    ///
    /// Results are cached based on the configured TTL.
    pub fn query(&self, symbol_name: &str) -> Vec<SymbolLocation> {
        // Check cache first
        if let Some(cached) = self.get_cached_query(symbol_name) {
            return cached;
        }

        // Query the index
        let results = self.index.find_symbol(symbol_name).to_vec();

        // Cache the result
        self.cache_query_result(symbol_name.to_lowercase(), results.clone());

        results
    }

    /// Finds all symbols defined in a specific file
    ///
    /// This does NOT use query caching since it's file-based.
    pub fn find_in_file(&self, file_path: &str) -> Vec<&SymbolLocation> {
        self.index.find_in_file(file_path)
    }

    /// Returns whether the index has been built
    pub fn is_built(&self) -> bool {
        self.built
    }

    /// Returns the number of symbols in the index
    pub fn symbol_count(&self) -> usize {
        self.index.symbol_count()
    }

    /// Returns the total number of locations in the index
    pub fn location_count(&self) -> usize {
        self.index.location_count()
    }

    /// Invalidates all cached query results
    pub fn clear_query_cache(&self) {
        if let Ok(mut cache) = self.query_cache.write() {
            cache.clear();
        }
    }

    /// Invalidates a specific cached query result
    pub fn invalidate_symbol(&self, symbol_name: &str) {
        if let Ok(mut cache) = self.query_cache.write() {
            cache.shift_remove(&symbol_name.to_lowercase());
        }
    }

    /// Gets a cached query result if it exists and is not expired
    fn get_cached_query(&self, symbol_name: &str) -> Option<Vec<SymbolLocation>> {
        let cache = self.query_cache.read().ok()?;
        let key = symbol_name.to_lowercase();
        let entry = cache.get(&key)?;

        if entry.created_at.elapsed() > self.config.max_age {
            return None;
        }

        Some((*entry.value.clone()).clone())
    }

    /// Caches a query result
    fn cache_query_result(&self, key: String, value: Vec<SymbolLocation>) {
        if let Ok(mut cache) = self.query_cache.write() {
            if cache.len() >= self.config.max_entries {
                self.evict_expired_entries(&mut cache);

                if cache.len() >= self.config.max_entries {
                    if let Some(oldest_key) = cache.keys().next().cloned() {
                        cache.shift_remove(&oldest_key);
                    }
                }
            }

            cache.insert(
                key,
                CacheEntry {
                    value: Arc::new(value),
                    created_at: Instant::now(),
                },
            );
        }
    }

    /// Removes expired entries from the cache
    fn evict_expired_entries(
        &self,
        cache: &mut IndexMap<String, CacheEntry<Arc<Vec<SymbolLocation>>>>,
    ) {
        let now = Instant::now();
        cache.retain(|_, entry| now.duration_since(entry.created_at) <= self.config.max_age);
    }

    /// Returns a reference to the underlying LightweightIndex
    pub fn underlying_index(&self) -> &LightweightIndex {
        &self.index
    }

    /// Gets an Arc reference to the index (thread-safe access)
    pub fn get_arc(&self) -> Arc<LightweightIndex> {
        Arc::new(self.index.clone())
    }
}

impl Default for SymbolIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SymbolIndex {
    fn clone(&self) -> Self {
        Self {
            index: self.index.clone(),
            query_cache: RwLock::new(IndexMap::new()), // Don't clone cache
            config: self.config.clone(),
            built: self.built,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::SymbolKind;

    #[test]
    fn test_symbol_index_new() {
        let index = SymbolIndex::new();
        assert!(!index.is_built());
        assert_eq!(index.symbol_count(), 0);
    }

    #[test]
    fn test_symbol_index_build_from_sources() {
        let mut index = SymbolIndex::new();
        let count = index.build_from_sources([(
            "test.py",
            "def hello():\n    pass\n\nclass MyClass:\n    pass\n",
        )]);

        assert_eq!(count, 2);
        assert!(index.is_built());
        assert_eq!(index.symbol_count(), 2);
    }

    #[test]
    fn test_symbol_index_query() {
        let mut index = SymbolIndex::new();
        index.build_from_sources([("test.py", "def hello():\n    pass\n")]);

        let results = index.query("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol_kind, SymbolKind::Function);
    }

    #[test]
    fn test_symbol_index_query_not_found() {
        let index = SymbolIndex::new();
        let results = index.query("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_symbol_index_find_in_file() {
        let mut index = SymbolIndex::new();
        index.build_from_sources([("test.py", "def a():\n    pass\ndef b():\n    pass\n")]);

        let results = index.find_in_file("test.py");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_symbol_index_cache_invalidation() {
        let mut index = SymbolIndex::new();
        index.build_from_sources([("test.py", "def hello():\n    pass\n")]);

        // First query populates cache
        let results1 = index.query("hello");
        assert_eq!(results1.len(), 1);

        // Invalidate and query again
        index.invalidate_symbol("hello");
        let results2 = index.query("hello");
        assert_eq!(results2.len(), 1);
    }
}
