//! In-memory implementation of GraphStore for testing
//!
//! This implementation stores data in memory, useful for tests and scenarios
//! where persistence is not required.
//!
//! The graph is kept in its serialized form, the way a persistent store keeps
//! it, and decoded at most once per write: every reader of the same content
//! gets the same shared snapshot, so a cache keyed by the identity of that
//! snapshot can hit. It is a **single-version** store: it holds one snapshot,
//! identified by a [`CheckpointId`] that changes with every write and is never
//! handed out twice. Use [`CachedGraphStore`](crate::infrastructure::persistence::CachedGraphStore)
//! for real versioned snapshot reads.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::value_objects::CheckpointId;
use crate::domain::value_objects::file_manifest::FileManifest;

/// The graph the store holds and the identity of the write that put it there.
#[derive(Debug)]
struct Head {
    id: CheckpointId,
    bytes: Vec<u8>,
    /// The decoded graph, once somebody asked for it. Lives as long as the
    /// head, so the snapshot is shared until the next write replaces it.
    shared: Option<Arc<CallGraph>>,
}

#[derive(Debug)]
struct GraphSlot {
    /// The last id handed out. Survives `clear`, so an id never names two
    /// different graphs.
    last_id: CheckpointId,
    head: Option<Head>,
}

impl Default for GraphSlot {
    fn default() -> Self {
        Self {
            last_id: CheckpointId::NONE,
            head: None,
        }
    }
}

impl GraphSlot {
    fn publish(&mut self, bytes: Vec<u8>) {
        self.last_id = self.last_id.next();
        self.head = Some(Head {
            id: self.last_id,
            bytes,
            shared: None,
        });
    }

    /// The snapshot of the head, decoded on first use. Bytes that do not
    /// decode are treated as absent and are not remembered.
    fn shared_head(&mut self) -> Option<Arc<CallGraph>> {
        let head = self.head.as_mut()?;
        if head.shared.is_none() {
            head.shared = decode_graph(&head.bytes).map(Arc::new);
        }
        head.shared.clone()
    }
}

fn decode_graph(bytes: &[u8]) -> Option<CallGraph> {
    bincode::serde::decode_from_slice::<CallGraph, _>(bytes, bincode::config::standard())
        .ok()
        .map(|(graph, _)| graph)
}

/// In-memory implementation of GraphStore for testing
#[derive(Debug, Default)]
pub struct InMemoryGraphStore {
    graph: Mutex<GraphSlot>,
    manifest: Mutex<Option<Vec<u8>>>,
}

impl InMemoryGraphStore {
    /// Create a new in-memory graph store
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&self) -> MutexGuard<'_, GraphSlot> {
        self.graph.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg(test)]
    fn corrupt_graph_bytes(&self) {
        let mut slot = self.slot();
        let head = slot.head.as_mut().expect("a graph is held");
        head.bytes = vec![0xFF, 0x00, 0x01, 0xFE, 0x00];
        head.shared = None;
    }
}

impl GraphStore for InMemoryGraphStore {
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        use bincode::config::standard;
        use bincode::serde::encode_to_vec;
        let bytes = encode_to_vec(graph, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        self.slot().publish(bytes);
        Ok(())
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        // Graceful degradation: if data is corrupted, treat as absent
        Ok(self
            .slot()
            .head
            .as_ref()
            .and_then(|head| decode_graph(&head.bytes)))
    }

    /// The snapshot the store holds, decoded once per write.
    ///
    /// Every call between two writes returns the same `Arc`; a write -even
    /// of an equal graph- replaces it with a new one. Both the snapshot and
    /// the serialized graph it came from stay in memory.
    fn load_graph_shared(&self) -> Result<Option<Arc<CallGraph>>, StoreError> {
        Ok(self.slot().shared_head())
    }

    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        use bincode::config::standard;
        use bincode::serde::encode_to_vec;
        let bytes = encode_to_vec(manifest, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        *self.manifest.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        use bincode::config::standard;
        use bincode::serde::decode_from_slice;
        let guard = self.manifest.lock().unwrap();
        match guard.as_ref() {
            Some(bytes) => {
                match decode_from_slice::<FileManifest, _>(bytes, standard()) {
                    Ok((manifest, _)) => Ok(Some(manifest)),
                    // Graceful degradation: if data is corrupted, treat as absent
                    Err(_) => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.slot().head = None;
        *self.manifest.lock().unwrap() = None;
        Ok(())
    }

    fn exists(&self) -> Result<bool, StoreError> {
        let graph_exists = self.slot().head.is_some();
        let manifest_exists = self.manifest.lock().unwrap().is_some();
        Ok(graph_exists || manifest_exists)
    }

    /// The id of the write that put the held graph there: it changes with
    /// every `save_graph`, is not reused after `clear`, and is `None` while
    /// the store holds no graph. A manifest alone is not a snapshot.
    fn current_checkpoint_id(&self) -> Option<CheckpointId> {
        self.slot().head.as_ref().map(|head| head.id)
    }

    /// Only the current id is available: the store keeps one snapshot.
    fn checkpoint_at(&self, id: CheckpointId) -> Result<Option<Arc<CallGraph>>, StoreError> {
        let mut slot = self.slot();
        match slot.head.as_ref().map(|head| head.id) {
            None => Ok(None),
            Some(current) if current == id => Ok(slot.shared_head()),
            Some(_) => Err(StoreError::CheckpointNotFound(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};
    use std::path::PathBuf;

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

    #[test]
    fn test_corrupted_data_returns_none_gracefully() {
        let store = InMemoryGraphStore::new();

        // First, save valid data
        let graph = create_test_graph();
        store.save_graph(&graph).unwrap();

        // Verify it loads correctly
        assert!(store.load_graph().unwrap().is_some());

        // Now corrupt the internal bytes directly
        store.corrupt_graph_bytes();

        // Loading corrupted data should return Ok(None), not Err
        let result = store.load_graph();
        assert!(result.is_ok(), "Expected Ok, got Err");
        assert!(
            result.unwrap().is_none(),
            "Expected None for corrupted data"
        );
    }

    #[test]
    fn test_checkpoint_id_cold_store_is_none() {
        let store = InMemoryGraphStore::new();
        assert_eq!(store.current_checkpoint_id(), None);
    }

    #[test]
    fn test_checkpoint_id_first_write_is_one() {
        let store = InMemoryGraphStore::new();
        store.save_graph(&create_test_graph()).unwrap();
        assert_eq!(store.current_checkpoint_id(), Some(CheckpointId(1)));
    }

    #[test]
    fn test_checkpoint_at_returns_current_graph() {
        let store = InMemoryGraphStore::new();
        store.save_graph(&create_test_graph()).unwrap();
        let arc = store.checkpoint_at(CheckpointId(1)).unwrap().unwrap();
        assert_eq!(arc.symbol_count(), 1);
    }

    #[test]
    fn test_checkpoint_at_unknown_id_returns_not_found() {
        let store = InMemoryGraphStore::new();
        store.save_graph(&create_test_graph()).unwrap();
        let result = store.checkpoint_at(CheckpointId(42));
        assert!(
            matches!(result, Err(StoreError::CheckpointNotFound(id)) if id == CheckpointId(42))
        );
    }

    #[test]
    fn test_checkpoint_at_cold_store_returns_none() {
        let store = InMemoryGraphStore::new();
        let result = store.checkpoint_at(CheckpointId(1)).unwrap();
        assert!(result.is_none());
    }
}
