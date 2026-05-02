//! Trait for persisting and loading the call graph
//!
//! This trait abstracts the persistence backend for the call graph,
//! supporting both file-based stores and in-memory stores for testing.

use crate::domain::aggregates::call_graph::CallGraph;
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
}

/// Trait for persisting and loading the call graph
pub trait GraphStore: Send + Sync {
    /// Save the call graph to the store
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError>;

    /// Load the call graph from the store
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError>;

    /// Save the file manifest
    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError>;

    /// Load the file manifest
    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError>;

    /// Clear all stored data
    fn clear(&self) -> Result<(), StoreError>;

    /// Check if data exists in the store
    fn exists(&self) -> Result<bool, StoreError>;
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
            let bytes =
                bincode::serde::encode_to_vec(graph, bincode::config::standard())
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
            let bytes =
                bincode::serde::encode_to_vec(manifest, bincode::config::standard())
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
}