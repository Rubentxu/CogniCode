//! Trait for persisting and loading the call graph
//!
//! This trait abstracts the persistence backend for the call graph,
//! supporting both file-based stores and in-memory stores for testing.
//!
//! ADR-035 adds versioned checkpoint reads via [`GraphStore::current_checkpoint_id`]
//! and [`GraphStore::checkpoint_at`]. These let callers pin to a specific
//! snapshot while writers publish new heads. [`GraphStore::load_graph_shared`]
//! hands out the snapshot itself, one pointer per unchanged graph.

use std::sync::Arc;

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::CheckpointId;
use crate::domain::value_objects::file_manifest::FileManifest;

/// Error type for graph store operations
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Corrupted data: {0}")]
    Corrupted(String),
    /// The requested checkpoint id is not (or no longer) available in the
    /// store's retention window. Returned by
    /// [`GraphStore::checkpoint_at`].
    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(CheckpointId),
}

/// Trait for persisting and loading the call graph
///
/// The default of [`GraphStore::checkpoint_at`] panics with a message naming
/// the concrete type — "loud failure > silent ignore" (ADR-035 D7) — because
/// a caller can only hold an id the store gave it through
/// [`GraphStore::current_checkpoint_id`], whose default is `None`: a store
/// that does not version its snapshots simply never hands one out.
pub trait GraphStore: Send + Sync {
    /// Save the call graph to the store
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError>;

    /// Load the call graph from the store
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError>;

    /// The call graph of the store, shared instead of copied.
    ///
    /// `load_graph` hands back an owned graph, so whoever asks for it
    /// pays for a copy of the whole graph —and a copy of a big graph is
    /// expensive on the thread that serves the request. This method hands
    /// back a pointer to the graph the store already holds, so reading it
    /// costs nothing.
    ///
    /// **Stable snapshot.** A store that persists a graph should return
    /// the same `Arc` on every call for as long as the graph is unchanged,
    /// and a different one after a write. That pointer is the identity of
    /// the snapshot: derived state that depends only on the graph (ranking
    /// statistics, projections) can be cached against it, and a reader that
    /// finds the same pointer knows the graph did not change. Both stores
    /// shipped with the crate honour this.
    ///
    /// The default implementation copies and so returns a new `Arc` per
    /// call: correct, but nothing keyed by the snapshot ever hits. A store
    /// that holds the graph in memory overrides it.
    fn load_graph_shared(&self) -> Result<Option<Arc<CallGraph>>, StoreError> {
        Ok(self.load_graph()?.map(Arc::new))
    }

    /// Save the file manifest
    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError>;

    /// Load the file manifest
    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError>;

    /// Clear all stored data
    fn clear(&self) -> Result<(), StoreError>;

    /// Check if data exists in the store
    fn exists(&self) -> Result<bool, StoreError>;

    /// Returns the [`CheckpointId`] of the current head, or `None` if the
    /// store holds no identified snapshot.
    ///
    /// An id names one graph for good: it is never handed out again for
    /// different content, not after a write and not after `clear`. `None`
    /// means there is nothing to name: the store is cold, or it does not
    /// version its snapshots at all, which is what the default says. It
    /// never panics. A caller that needs the identity of a snapshot from a
    /// store of any kind should use the pointer of
    /// [`GraphStore::load_graph_shared`] instead.
    ///
    /// Stores that maintain a versioned ring (e.g. `CachedGraphStore`)
    /// return the live head id; single-version stores (e.g.
    /// `InMemoryGraphStore`) return the id of the write that put the held
    /// graph there.
    fn current_checkpoint_id(&self) -> Option<CheckpointId> {
        None
    }

    /// Returns the `CallGraph` snapshot for a given [`CheckpointId`].
    ///
    /// * `Ok(Some(graph))` — checkpoint is in the store's retention window.
    /// * `Ok(None)` — store has no checkpoints at all (cold store).
    /// * `Err(StoreError::CheckpointNotFound(id))` — store has at least
    ///   one checkpoint but `id` is not (or no longer) in the window.
    ///
    /// Default impl panics: a store that does not return an id from
    /// [`GraphStore::current_checkpoint_id`] has none to look up. Stores
    /// that do return one override it.
    fn checkpoint_at(&self, id: CheckpointId) -> Result<Option<Arc<CallGraph>>, StoreError> {
        panic!(
            "checkpoint_at not implemented for {} (id={})",
            std::any::type_name::<Self>(),
            id
        );
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
    use std::sync::Mutex;

    struct InMemoryGraphStore {
        graph: Mutex<Option<Vec<u8>>>,
        manifest: Mutex<Option<Vec<u8>>>,
    }

    impl InMemoryGraphStore {
        fn new() -> Self {
            Self {
                graph: Mutex::new(None),
                manifest: Mutex::new(None),
            }
        }
    }

    impl GraphStore for InMemoryGraphStore {
        fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
            let bytes = bincode::serde::encode_to_vec(graph, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            *self.graph.lock().unwrap() = Some(bytes);
            Ok(())
        }

        fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
            let guard = self.graph.lock().unwrap();
            match guard.as_ref() {
                Some(bytes) => {
                    let (graph, _) = bincode::serde::decode_from_slice::<CallGraph, _>(
                        bytes,
                        bincode::config::standard(),
                    )
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                    Ok(Some(graph))
                }
                None => Ok(None),
            }
        }

        fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
            let bytes = bincode::serde::encode_to_vec(manifest, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            *self.manifest.lock().unwrap() = Some(bytes);
            Ok(())
        }

        fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
            let guard = self.manifest.lock().unwrap();
            match guard.as_ref() {
                Some(bytes) => {
                    let (manifest, _) = bincode::serde::decode_from_slice::<FileManifest, _>(
                        bytes,
                        bincode::config::standard(),
                    )
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                    Ok(Some(manifest))
                }
                None => Ok(None),
            }
        }

        fn clear(&self) -> Result<(), StoreError> {
            *self.graph.lock().unwrap() = None;
            *self.manifest.lock().unwrap() = None;
            Ok(())
        }

        fn exists(&self) -> Result<bool, StoreError> {
            let graph_exists = self.graph.lock().unwrap().is_some();
            let manifest_exists = self.manifest.lock().unwrap().is_some();
            Ok(graph_exists || manifest_exists)
        }
    }

    fn create_test_graph() -> CallGraph {
        let mut graph = CallGraph::new();
        let symbol = Symbol::new(
            "test_function",
            SymbolKind::Function,
            Location::new("test_file.rs", 0, 0),
        );
        graph.add_symbol(symbol);
        graph
    }

    #[test]
    fn test_save_and_load_graph() {
        let store = InMemoryGraphStore::new();
        let graph = create_test_graph();

        store.save_graph(&graph).unwrap();
        let loaded = store.load_graph().unwrap().unwrap();

        assert_eq!(loaded.symbol_count(), graph.symbol_count());
    }

    #[test]
    fn test_load_empty_returns_none() {
        let store = InMemoryGraphStore::new();

        let loaded = store.load_graph().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_save_and_load_manifest() {
        let store = InMemoryGraphStore::new();
        let mut manifest = FileManifest::new(PathBuf::from("/project"));
        manifest.update_entries(&[(
            PathBuf::from("src/main.rs"),
            "hash123".to_string(),
            Some(1000),
            5,
        )]);

        store.save_manifest(&manifest).unwrap();
        let loaded = store.load_manifest().unwrap().unwrap();

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(
            loaded
                .get(&PathBuf::from("src/main.rs"))
                .unwrap()
                .content_hash,
            "hash123"
        );
    }

    #[test]
    fn test_clear_removes_all() {
        let store = InMemoryGraphStore::new();
        let graph = create_test_graph();

        store.save_graph(&graph).unwrap();
        let mut manifest = FileManifest::new(PathBuf::from("/project"));
        manifest.update_entries(&[(
            PathBuf::from("src/main.rs"),
            "hash".to_string(),
            Some(1000),
            5,
        )]);
        store.save_manifest(&manifest).unwrap();

        store.clear().unwrap();

        assert!(store.load_graph().unwrap().is_none());
        assert!(store.load_manifest().unwrap().is_none());
        assert!(!store.exists().unwrap());
    }
}
