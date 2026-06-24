//! In-memory implementation of GraphStore for testing
//!
//! This implementation stores data in memory, useful for tests and scenarios
//! where persistence is not required.
//!
//! ADR-035: this is a **single-version** store. It exposes the checkpoint
//! methods but reports a fixed id of 1 (or `None` when cold) and always
//! returns the current graph. Use [`CachedGraphStore`](crate::infrastructure::persistence::CachedGraphStore)
//! for real versioned snapshot reads.

use std::sync::{Arc, Mutex};

use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::value_objects::CheckpointId;
use crate::domain::value_objects::file_manifest::FileManifest;

/// In-memory implementation of GraphStore for testing
#[derive(Debug, Default)]
pub struct InMemoryGraphStore {
    #[cfg(test)]
    pub(crate) graph: Mutex<Option<Vec<u8>>>,
    #[cfg(not(test))]
    graph: Mutex<Option<Vec<u8>>>,
    manifest: Mutex<Option<Vec<u8>>>,
}

impl InMemoryGraphStore {
    /// Create a new in-memory graph store
    pub fn new() -> Self {
        Self {
            graph: Mutex::new(None),
            manifest: Mutex::new(None),
        }
    }
}

impl GraphStore for InMemoryGraphStore {
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        use bincode::config::standard;
        use bincode::serde::encode_to_vec;
        let bytes = encode_to_vec(graph, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        *self.graph.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        use bincode::config::standard;
        use bincode::serde::decode_from_slice;
        let guard = self.graph.lock().unwrap();
        match guard.as_ref() {
            Some(bytes) => {
                match decode_from_slice::<CallGraph, _>(bytes, standard()) {
                    Ok((graph, _)) => Ok(Some(graph)),
                    // Graceful degradation: if data is corrupted, treat as absent
                    Err(_) => Ok(None),
                }
            }
            None => Ok(None),
        }
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
        *self.graph.lock().unwrap() = None;
        *self.manifest.lock().unwrap() = None;
        Ok(())
    }

    fn exists(&self) -> Result<bool, StoreError> {
        let graph_exists = self.graph.lock().unwrap().is_some();
        let manifest_exists = self.manifest.lock().unwrap().is_some();
        Ok(graph_exists || manifest_exists)
    }

    // ----- ADR-035: single-version checkpoint stubs -----

    fn current_checkpoint_id(&self) -> Option<CheckpointId> {
        // A single-version store reports id 1 once it holds any data.
        // Cold store => None (mirrors the CachedGraphStore semantics).
        if self.exists().unwrap_or(false) {
            Some(CheckpointId(1))
        } else {
            None
        }
    }

    fn checkpoint_at(&self, id: CheckpointId) -> Result<Option<Arc<CallGraph>>, StoreError> {
        // Only id 1 is valid for a single-version store.
        if id != CheckpointId(1) {
            return Err(StoreError::CheckpointNotFound(id));
        }
        self.load_graph().map(|opt| opt.map(Arc::new))
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
        manifest.update_entries(&[(PathBuf::from("src/main.rs"), "hash123".to_string(), 1000, 5)]);

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
        manifest.update_entries(&[(PathBuf::from("src/main.rs"), "hash".to_string(), 1000, 5)]);
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
        {
            let mut guard = store.graph.lock().unwrap();
            *guard = Some(vec![0xFF, 0x00, 0x01, 0xFE, 0x00]);
        }

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
    fn test_checkpoint_id_warm_store_is_one() {
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
