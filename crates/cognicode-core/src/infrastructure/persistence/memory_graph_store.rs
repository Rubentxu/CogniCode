//! In-memory implementation of GraphStore for testing
//!
//! This implementation stores data in memory, useful for tests and scenarios
//! where persistence is not required.

use std::sync::Mutex;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::aggregates::call_graph::CallGraph;
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
        use bincode::serde::encode_to_vec;
        use bincode::config::standard;
        let bytes = encode_to_vec(graph, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        *self.graph.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        use bincode::serde::decode_from_slice;
        use bincode::config::standard;
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
        use bincode::serde::encode_to_vec;
        use bincode::config::standard;
        let bytes = encode_to_vec(manifest, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        *self.manifest.lock().unwrap() = Some(bytes);
        Ok(())
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        use bincode::serde::decode_from_slice;
        use bincode::config::standard;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};

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
            1000,
            5,
        )]);

        store.save_manifest(&manifest).unwrap();
        let loaded = store.load_manifest().unwrap().unwrap();

        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(
            loaded.get(&PathBuf::from("src/main.rs")).unwrap().content_hash,
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
            1000,
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
        {
            let mut guard = store.graph.lock().unwrap();
            *guard = Some(vec![0xFF, 0x00, 0x01, 0xFE, 0x00]);
        }

        // Loading corrupted data should return Ok(None), not Err
        let result = store.load_graph();
        assert!(result.is_ok(), "Expected Ok, got Err");
        assert!(result.unwrap().is_none(), "Expected None for corrupted data");
    }
}