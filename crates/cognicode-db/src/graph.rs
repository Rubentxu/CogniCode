//! CallGraph persistence with SQLite
//!
//! Stores CallGraph as bincode blob (backward compatible) AND
//! populates symbols/call_edges tables for queryability.

use rusqlite::{Connection, params};
use std::sync::Mutex;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::value_objects::file_manifest::FileManifest;

/// SQLite-based implementation of GraphStore
pub struct SqliteGraphStore {
    db: Mutex<Connection>,
}

impl SqliteGraphStore {
    pub fn open(db_path: &std::path::Path) -> Result<Self, String> {
        let db = Connection::open(db_path).map_err(|e| e.to_string())?;
        crate::schema::initialize_schema(&db);
        Ok(Self { db: Mutex::new(db) })
    }

    /// Save CallGraph: both blob AND normalized tables
    pub fn save_graph(&self, graph: &CallGraph) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        // 1. Save as bincode blob (backward compat, fast load)
        let bytes = bincode::serde::encode_to_vec(graph, bincode::config::standard())
            .map_err(|e| e.to_string())?;
        db.execute(
            "INSERT OR REPLACE INTO call_graphs (id, data) VALUES (1, ?1)",
            params![bytes],
        ).map_err(|e| e.to_string())?;

        // 2. Populate normalized tables (for queries)
        drop(db);
        self.populate_symbols(graph)?;
        self.populate_edges(graph)?;

        Ok(())
    }

    /// Load CallGraph from blob (fast, backward compat)
    pub fn load_graph(&self) -> Result<Option<CallGraph>, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let result: Result<Vec<u8>, _> = db.query_row(
            "SELECT data FROM call_graphs WHERE id = 1", [],
            |row| row.get(0)
        );
        match result {
            Ok(bytes) => {
                let (graph, _) = bincode::serde::decode_from_slice::<CallGraph, _>(
                    &bytes, bincode::config::standard()
                ).map_err(|e| e.to_string())?;
                Ok(Some(graph))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Populate symbols table from CallGraph
    fn populate_symbols(&self, graph: &CallGraph) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        // Clear old symbols for this graph
        db.execute("DELETE FROM symbols", []).map_err(|e| e.to_string())?;

        for (_id, symbol) in graph.symbol_ids() {
            let location = symbol.location();
            db.execute(
                "INSERT INTO symbols (file_path, name, kind, line, column) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    location.file(),
                    symbol.name(),
                    format!("{:?}", symbol.kind()),
                    location.line() as i64,
                    location.column() as i64,
                ],
            ).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Populate call_edges table from CallGraph
    fn populate_edges(&self, graph: &CallGraph) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        db.execute("DELETE FROM call_edges", []).map_err(|e| e.to_string())?;

        for (source_id, symbol) in graph.symbol_ids() {
            let callees = graph.callees(source_id);
            for (target_id, dep_type) in &callees {
                db.execute(
                    "INSERT INTO call_edges (caller_name, callee_name, dependency_type) VALUES (?1, ?2, ?3)",
                    params![symbol.name(),
                        graph.get_symbol(target_id).map(|s| s.name()).unwrap_or("unknown"),
                        format!("{:?}", dep_type),
                    ],
                ).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Clear all graph data
    pub fn clear(&self) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        db.execute_batch("DELETE FROM call_graphs; DELETE FROM symbols; DELETE FROM call_edges;")
            .map_err(|e| e.to_string())
    }

    /// Check if graph data exists
    pub fn exists(&self) -> Result<bool, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let count: i64 = db.query_row("SELECT COUNT(*) FROM call_graphs", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }
}

impl GraphStore for SqliteGraphStore {
    fn save_graph(&self, graph: &CallGraph) -> Result<(), StoreError> {
        SqliteGraphStore::save_graph(self, graph).map_err(|e| StoreError::Database(e))
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        SqliteGraphStore::load_graph(self).map_err(|e| StoreError::Database(e))
    }

    fn save_manifest(&self, manifest: &FileManifest) -> Result<(), StoreError> {
        let bytes = bincode::serde::encode_to_vec(manifest, bincode::config::standard())
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let db = self.db.lock().map_err(|e| StoreError::Database(e.to_string()))?;
        db.execute("INSERT OR REPLACE INTO call_graphs (id, data) VALUES (2, ?1)", params![bytes])
            .map_err(|e| StoreError::Database(e.to_string()))?;
        Ok(())
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        let db = self.db.lock().map_err(|e| StoreError::Database(e.to_string()))?;
        let result: Result<Vec<u8>, _> = db.query_row("SELECT data FROM call_graphs WHERE id = 2", [], |row| row.get(0));
        match result {
            Ok(bytes) => {
                let (manifest, _) = bincode::serde::decode_from_slice(&bytes, bincode::config::standard())
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(manifest))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StoreError::Database(e.to_string())),
        }
    }

    fn clear(&self) -> Result<(), StoreError> {
        SqliteGraphStore::clear(self).map_err(|e| StoreError::Database(e))
    }

    fn exists(&self) -> Result<bool, StoreError> {
        SqliteGraphStore::exists(self).map_err(|e| StoreError::Database(e))
    }
}