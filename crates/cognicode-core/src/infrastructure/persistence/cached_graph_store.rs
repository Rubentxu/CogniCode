//! Cached `GraphStore` impl that delegates reads to a shared
//! `Arc<GraphCache>` (lock-free `ArcSwap<CallGraph>`) and forwards
//! writes/manifest operations to an inner `InMemoryGraphStore`.
//!
//! This makes `HandlerContext::get_graph_store()` return a `GraphStore`
//! that is fast on the read path (~2× faster than the bincode path)
//! while keeping the `GraphStore` trait contract intact for future
//! persistence adapters. See ADR-032.
//!
//! ADR-035: this is the only `GraphStore` impl that exposes
//! **real** versioned snapshot reads. It delegates to
//! [`GraphCache::current_id`] and [`GraphCache::get_at`] which read
//! from the lock-free `VersionedGraphCache` ring inside the cache.
//! The inner `InMemoryGraphStore` is single-version and reports id 1.

use std::sync::Arc;

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::domain::value_objects::CheckpointId;
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
    /// Lock-free cache for reads (real versioned ring per ADR-035).
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

    // ----- ADR-035: real versioned checkpoint reads -----

    /// Returns the [`CheckpointId`] of the current head, or `None` if
    /// no checkpoint has ever been published. Delegates to
    /// [`GraphCache::current_id`].
    fn current_checkpoint_id(&self) -> Option<CheckpointId> {
        self.cache.current_id()
    }

    /// Returns the `CallGraph` snapshot pinned to `id`. `Err(
    /// StoreError::CheckpointNotFound)` if the id is not (or no
    /// longer) in the cache's retention window. Delegates to
    /// [`GraphCache::get_at`].
    ///
    /// Note: the cache's `get_at` returns `None` when the ring is
    /// cold (no inserts yet), which is semantically distinct from
    /// "id evicted". We therefore check `current_id()` first: if the
    /// ring is cold we return `Ok(None)`; otherwise a `None` from
    /// `get_at` is a true "not found" and we lift it to
    /// `Err(CheckpointNotFound)`.
    fn checkpoint_at(
        &self,
        id: CheckpointId,
    ) -> Result<Option<Arc<CallGraph>>, StoreError> {
        if self.cache.current_id().is_none() {
            return Ok(None);
        }
        self.cache
            .get_at(id)
            .map(Some)
            .ok_or(StoreError::CheckpointNotFound(id))
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

    // ----- ADR-035: checkpoint-aware methods -----

    #[test]
    fn cached_graph_store_current_checkpoint_id_cold_is_none() {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache);
        assert_eq!(store.current_checkpoint_id(), None);
    }

    #[test]
    fn cached_graph_store_current_checkpoint_id_after_set() {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache.clone());
        let id = cache.set(build_small_graph());
        assert_eq!(store.current_checkpoint_id(), Some(id));
    }

    #[test]
    fn cached_graph_store_checkpoint_at_returns_head() {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache.clone());
        let id = cache.set(build_small_graph());
        let arc = store.checkpoint_at(id).unwrap().unwrap();
        assert_eq!(arc.symbol_count(), 1);
    }

    #[test]
    fn cached_graph_store_checkpoint_at_cold_is_none() {
        let cache = Arc::new(GraphCache::new());
        let store = CachedGraphStore::new(cache);
        // Cold cache: id 1 doesn't exist yet.
        let result = store.checkpoint_at(CheckpointId(1)).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn cached_graph_store_checkpoint_at_evicted_returns_not_found() {
        // Retention 2: after 3 inserts, id 1 is evicted.
        let cache = Arc::new(GraphCache::with_retention(2));
        let store = CachedGraphStore::new(cache.clone());

        let id1 = cache.set(build_small_graph());
        let _id2 = cache.set(build_small_graph());
        let _id3 = cache.set(build_small_graph());

        let result = store.checkpoint_at(id1);
        assert!(
            matches!(result, Err(StoreError::CheckpointNotFound(id)) if id == id1),
            "expected CheckpointNotFound, got {:?}",
            result
        );
    }
}
