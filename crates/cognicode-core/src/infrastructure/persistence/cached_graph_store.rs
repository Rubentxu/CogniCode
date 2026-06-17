//! Cached `GraphStore` impl that delegates reads to a shared
//! `Arc<GraphCache>` (lock-free `ArcSwap<CallGraph>`) and forwards
//! writes/manifest operations to an inner `InMemoryGraphStore`.
//!
//! This makes `HandlerContext::get_graph_store()` return a `GraphStore`
//! that is fast on the read path (~2× faster than the bincode path)
//! while keeping the `GraphStore` trait contract intact for future
//! persistence adapters. See ADR-032.

use std::sync::Arc;

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::infrastructure::graph::GraphCache;
use crate::infrastructure::persistence::InMemoryGraphStore;

/// `GraphStore` impl that reads from a shared `GraphCache` and
/// forwards writes to an inner `InMemoryGraphStore`.
///
/// **Read path** (`load_graph`): returns the current `CallGraph` from
/// the lock-free `ArcSwap` cache. O(1) — no bincode, no Mutex.
///
/// **Write path** (`save_graph`, `save_manifest`, `clear`, `exists`,
/// `load_manifest`): forwarded to the inner `InMemoryGraphStore`.
/// These methods are kept for trait compatibility and future
/// persistence migration; in the current MCP server architecture
/// only `save_manifest` is called by `build_graph` for staleness
/// detection.
pub struct CachedGraphStore {
    /// Lock-free cache for reads.
    cache: Arc<GraphCache>,
    /// Inner store for write/manifest/clear operations.
    inner: InMemoryGraphStore,
}

impl CachedGraphStore {
    /// Create a new `CachedGraphStore` that reads from `cache` and
    /// forwards writes to a fresh `InMemoryGraphStore`.
    pub fn new(cache: Arc<GraphCache>) -> Self {
        Self {
            cache,
            inner: InMemoryGraphStore::new(),
        }
    }
}

impl GraphStore for CachedGraphStore {
    /// Reads from the shared `GraphCache` (lock-free, no serialization).
    /// Returns `None` if the cache holds an empty graph so callers can
    /// prompt the user to run `build_graph` first (matches legacy
    /// behaviour).
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        let arc_graph = self.cache.get();
        if arc_graph.symbol_count() == 0 && arc_graph.edge_count() == 0 {
            Ok(None)
        } else {
            // Dereference the Arc to get an owned CallGraph.
            // This clones the full graph (5-10 MB for 29K symbols) but
            // avoids the bincode encode/decode roundtrip and Mutex
            // contention of the legacy InMemoryGraphStore path.
            Ok(Some((*arc_graph).clone()))
        }
    }

    /// Forwards to the inner store. Kept for trait contract and future
    /// persistence adapter; no production caller in the MCP server
    /// invokes this after ADR-032.
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        self.inner.save_graph(graph)
    }

    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        self.inner.save_manifest(manifest)
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        self.inner.load_manifest()
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.inner.clear()
    }

    fn exists(&self) -> Result<bool, StoreError> {
        self.inner.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::call_graph::CallGraph;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::file_manifest::FileManifest;
    use crate::domain::value_objects::{Location, SymbolKind};
    use std::path::PathBuf;

    fn build_small_graph() -> CallGraph {
        let mut g = CallGraph::new();
        let s = Symbol::new(
            "test_fn",
            SymbolKind::Function,
            Location::new("test.rs", 1, 0),
        );
        g.add_symbol(s);
        g
    }

    #[test]
    fn cached_graph_store_loads_from_cache() {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache.clone());

        // Empty cache returns None
        let loaded = store.load_graph().unwrap();
        assert!(loaded.is_none());

        // After populating the cache, load returns Some
        let graph = build_small_graph();
        cache.set(graph.clone());
        let loaded = store.load_graph().unwrap().unwrap();
        assert_eq!(loaded.symbol_count(), graph.symbol_count());
    }

    #[test]
    fn cached_graph_store_delegates_manifest() {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache);

        // Empty: no manifest
        assert!(store.load_manifest().unwrap().is_none());
        assert!(!store.exists().unwrap());

        // Save manifest
        let mut manifest = FileManifest::new(PathBuf::from("/proj"));
        manifest.update_entries(&[(PathBuf::from("a.rs"), "h".to_string(), 1, 1)]);
        store.save_manifest(&manifest).unwrap();
        assert!(store.exists().unwrap());

        // Load manifest round-trips through inner store
        let loaded = store.load_manifest().unwrap().unwrap();
        assert_eq!(loaded.entries.len(), 1);
    }

    #[test]
    fn cached_graph_store_clear() {
        let cache = Arc::new(GraphCache::new());
        cache.set(build_small_graph());
        let store = CachedGraphStore::new(cache);
        store.clear().unwrap();
        assert!(!store.exists().unwrap());
        // Cache itself is untouched by clear() — only inner store cleared.
        // (The cache holds the source of truth for reads.)
        assert_eq!(store.load_graph().unwrap().unwrap().symbol_count(), 1);
    }
}
