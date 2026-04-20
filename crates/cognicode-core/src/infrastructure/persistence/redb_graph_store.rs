//! Redb-based implementation of GraphStore
//!
//! This implementation uses the redb embedded database for persistent storage
//! of the call graph and file manifest.

use std::sync::Arc;
use crate::domain::traits::graph_store::{GraphStore, StoreError};
use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::file_manifest::FileManifest;

// Table definition for the data table: &str keys, &[u8] values
const DATA_TABLE: redb::TableDefinition<&str, &[u8]> =
    redb::TableDefinition::new("data");

/// Redb-based implementation of GraphStore for persistent storage
#[derive(Debug)]
pub struct RedbGraphStore {
    db: Arc<redb::Database>,
}

impl RedbGraphStore {
    /// Open or create a RedbGraphStore at the given path
    pub fn open(db_path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        let db = redb::Database::create(db_path)
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(Self { db: Arc::new(db) })
    }

    fn write_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        use bincode::serde::encode_to_vec;
        use bincode::config::standard;
        let bytes = encode_to_vec(graph, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let txn = self.db.begin_write()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        {
            let mut table = txn.open_table(DATA_TABLE)
                .map_err(|e| StoreError::Database(e.to_string()))?;
            table.insert("call_graph", bytes.as_slice())
                .map_err(|e| StoreError::Database(e.to_string()))?;
        }
        txn.commit().map_err(|e| StoreError::Database(e.to_string()))
    }

    fn read_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        use bincode::serde::decode_from_slice;
        use bincode::config::standard;
        let txn = self.db.begin_read()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let table = match txn.open_table(DATA_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None), // Table doesn't exist yet
        };
        match table.get("call_graph") {
            Ok(Some(value)) => {
                let bytes: Vec<u8> = value.value().to_vec();
                // Graceful degradation: if deserialization fails, return None
                match decode_from_slice::<CallGraph, _>(&bytes, standard()) {
                    Ok((graph, _)) => Ok(Some(graph)),
                    Err(_) => Ok(None), // Corrupted data - return None for graceful degradation
                }
            }
            Ok(None) => Ok(None),
            Err(_) => Ok(None), // Graceful degradation
        }
    }

    fn write_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        use bincode::serde::encode_to_vec;
        use bincode::config::standard;
        let bytes = encode_to_vec(manifest, standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let txn = self.db.begin_write()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        {
            let mut table = txn.open_table(DATA_TABLE)
                .map_err(|e| StoreError::Database(e.to_string()))?;
            table.insert("file_manifest", bytes.as_slice())
                .map_err(|e| StoreError::Database(e.to_string()))?;
        }
        txn.commit().map_err(|e| StoreError::Database(e.to_string()))
    }

    fn read_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        use bincode::serde::decode_from_slice;
        use bincode::config::standard;
        let txn = self.db.begin_read()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        let table = match txn.open_table(DATA_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None), // Table doesn't exist yet
        };
        match table.get("file_manifest") {
            Ok(Some(value)) => {
                let bytes: Vec<u8> = value.value().to_vec();
                // Graceful degradation: if deserialization fails, return None
                match decode_from_slice::<FileManifest, _>(&bytes, standard()) {
                    Ok((manifest, _)) => Ok(Some(manifest)),
                    Err(_) => Ok(None), // Corrupted data - return None for graceful degradation
                }
            }
            Ok(None) => Ok(None),
            Err(_) => Ok(None), // Graceful degradation
        }
    }

    fn delete_all(&self) -> Result<(), StoreError> {
        let txn = self.db.begin_write()
            .map_err(|e| StoreError::Database(e.to_string()))?;
        {
            let mut table = txn.open_table(DATA_TABLE)
                .map_err(|e| StoreError::Database(e.to_string()))?;
            let _ = table.remove("call_graph");
            let _ = table.remove("file_manifest");
        }
        txn.commit().map_err(|e| StoreError::Database(e.to_string()))
    }
}

impl GraphStore for RedbGraphStore {
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        self.write_graph(graph)
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        self.read_graph()
    }

    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        self.write_manifest(manifest)
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        self.read_manifest()
    }

    fn clear(&self) -> Result<(), StoreError> {
        self.delete_all()
    }

    fn exists(&self) -> Result<bool, StoreError> {
        // Check if any data exists by attempting to read
        match self.load_graph() {
            Ok(Some(_)) => Ok(true),
            Ok(None) => {
                // Graph doesn't exist, check manifest
                match self.load_manifest() {
                    Ok(Some(_)) => Ok(true),
                    Ok(None) => Ok(false),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "persistence")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;
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
    fn test_save_and_load_graph_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let store = RedbGraphStore::open(&db_path).unwrap();
        let graph = create_test_graph();

        store.save_graph(&graph).unwrap();
        let loaded = store.load_graph().unwrap().unwrap();

        assert_eq!(loaded.symbol_count(), graph.symbol_count());
    }

    #[test]
    fn test_load_from_empty_store_returns_none() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let store = RedbGraphStore::open(&db_path).unwrap();

        let loaded = store.load_graph().unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_save_and_load_manifest_roundtrip() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let store = RedbGraphStore::open(&db_path).unwrap();
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
    fn test_corrupted_graph_returns_none_gracefully() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let store = RedbGraphStore::open(&db_path).unwrap();

        // Write corrupted data directly to the database
        {
            let txn = store.db.begin_write().unwrap();
            {
                let mut table = txn.open_table(DATA_TABLE).unwrap();
                table.insert("call_graph", &b"corrupted data not valid bincode"[..]).unwrap();
            }
            txn.commit().unwrap();
        }

        // Loading should return None, not an error (graceful degradation)
        let result = store.load_graph();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_clear_removes_all_data() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.redb");
        let store = RedbGraphStore::open(&db_path).unwrap();
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