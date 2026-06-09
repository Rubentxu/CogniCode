//! CallGraph persistence with SQLite
//!
//! Stores CallGraph as a versioned bincode blob AND populates the
//! `symbols` / `call_edges` tables for queryability.
//!
//! ## Blob format (v2, current)
//!
//! ```text
//! +---------+---------+-------------------------+
//! | CCG1    | version | bincode(CallGraph v2)   |
//! | 4 bytes | 1 byte  | N bytes                |
//! +---------+---------+-------------------------+
//! ```
//!
//! The 4-byte ASCII magic `b"CCG1"` lets us tell apart CallGraph blobs
//! from arbitrary `BLOB` payloads, and the version byte lets us read
//! older payloads if we ever change the on-disk shape again.
//!
//! ## Blob format (v1, legacy)
//!
//! Pre-metadata blobs are **raw bincode of `CallGraphV1`** with **no
//! magic / no version header**. They are detected as "first 4 bytes are
//! not `CCG1`", then decoded via [`CallGraphV1::into_v2`] which assigns
//! `(Provenance::Extracted, 1.0)` to every edge.
//!
//! Any blob whose first 4 bytes equal `CCG1` but whose version byte is
//! not the current version is rejected with [`StoreError::Corrupted`].

// `CallGraphV1` is intentionally deprecated; we import it only to
// decode legacy v1 blobs on the read path. The whole module deals
// with version migration, so silencing the deprecation warning here
// is appropriate.
#![allow(deprecated)]

use cognicode_core::domain::aggregates::call_graph::CallGraphV1;
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::value_objects::file_manifest::FileManifest;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use rusqlite::{Connection, params};
use std::sync::Mutex;

/// Magic header bytes that prefix every versioned CallGraph blob.
const CALLGRAPH_BLOB_MAGIC: [u8; 4] = *b"CCG1";

/// Current blob format version. Bump this whenever the `CallGraph`
/// serde shape changes in a way that is not backward-compatible with
/// the previous version.
const BLOB_VERSION_CURRENT: u8 = 2;

/// Encodes/decodes versioned CallGraph blobs.
///
/// A versioned blob is a small header (`[CCG1, version]`) followed by
/// the bincode-serialized `CallGraph`. Legacy v1 blobs (no header) are
/// still accepted on the read path; they are upgraded to v2 in memory
/// with `(Extracted, 1.0)` defaults via [`CallGraphV1::into_v2`].
pub struct VersionedBlob;

impl VersionedBlob {
    /// Encode a v2 `CallGraph` into the versioned blob format.
    pub fn encode_v2(graph: &CallGraph) -> Result<Vec<u8>, StoreError> {
        let payload =
            bincode::serde::encode_to_vec(graph, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let mut out = Vec::with_capacity(5 + payload.len());
        out.extend_from_slice(&CALLGRAPH_BLOB_MAGIC);
        out.push(BLOB_VERSION_CURRENT);
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Decode a blob produced by [`Self::encode_v2`] (or a legacy v1
    /// blob) into a `CallGraph`.
    ///
    /// # Errors
    ///
    /// * [`StoreError::Corrupted`] if the blob starts with the magic
    ///   bytes but the version byte is unsupported.
    /// * [`StoreError::Serialization`] if bincode decoding fails.
    pub fn decode(bytes: &[u8]) -> Result<CallGraph, StoreError> {
        // v2 path: magic + version + payload.
        if bytes.len() >= 5 && bytes[..4] == CALLGRAPH_BLOB_MAGIC {
            let version = bytes[4];
            if version != BLOB_VERSION_CURRENT {
                return Err(StoreError::Corrupted(format!(
                    "unsupported CallGraph blob version: {version} (expected {})",
                    BLOB_VERSION_CURRENT
                )));
            }
            let payload = &bytes[5..];
            let (graph, _): (CallGraph, usize) =
                bincode::serde::decode_from_slice(payload, bincode::config::standard())
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
            return Ok(graph);
        }
        // v1 fallback: no magic, raw bincode(CallGraphV1).
        #[allow(deprecated)]
        let (v1, _): (CallGraphV1, usize) =
            bincode::serde::decode_from_slice(bytes, bincode::config::standard())
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
        Ok(v1.into_v2())
    }
}

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
        // 1. Save as versioned bincode blob (v2 with magic header).
        let bytes = VersionedBlob::encode_v2(graph).map_err(|e| e.to_string())?;
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
                let graph = VersionedBlob::decode(&bytes).map_err(|e| e.to_string())?;
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

    /// Populate call_edges table from CallGraph.
    ///
    /// Inserted columns:
    /// * `caller_id`    — `SymbolId` of the source symbol
    /// * `caller_name`  — human-readable source symbol name (denormalized
    ///                    for quick lookups; the canonical join key is
    ///                    `caller_id` against `symbols.id`)
    /// * `callee_id`    — `SymbolId` of the target symbol
    /// * `callee_name`  — human-readable target symbol name
    /// * `dependency_type` — [`DependencyType`] Debug representation
    /// * `provenance`   — [`Provenance`] as a string
    /// * `confidence`   — `f64` in `[0.0, 1.0]`
    fn populate_edges(&self, graph: &CallGraph) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        db.execute("DELETE FROM call_edges", []).map_err(|e| e.to_string())?;

        for (source_id, target_id, dep_type, provenance, confidence) in
            graph.edges_with_metadata()
        {
            let source_name = graph
                .get_symbol(&source_id)
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            let target_name = graph
                .get_symbol(&target_id)
                .map(|s| s.name().to_string())
                .unwrap_or_default();
            let dep_str = format!("{:?}", dep_type);
            let prov_str = provenance.to_string();
            // Unreachable without a contract violation, but f64 NaN can
            // still pass through here in degenerate code paths — guard it
            // so we never poison the SQLite REAL column.
            let conf = if confidence.is_finite() {
                confidence
            } else {
                0.0
            };
            db.execute(
                "INSERT INTO call_edges \
                 (caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    source_id.as_str(),
                    source_name,
                    target_id.as_str(),
                    target_name,
                    dep_str,
                    prov_str,
                    conf,
                ],
            ).map_err(|e| e.to_string())?;
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
        SqliteGraphStore::save_graph(self, graph).map_err(StoreError::Database)
    }

    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        SqliteGraphStore::load_graph(self).map_err(StoreError::Database)
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
        SqliteGraphStore::clear(self).map_err(StoreError::Database)
    }

    fn exists(&self) -> Result<bool, StoreError> {
        SqliteGraphStore::exists(self).map_err(StoreError::Database)
    }
}

// Suppress unused-import / dead-code warnings for items the module
// references for documentation purposes only.
#[allow(dead_code)]
const _DEP_TYPES: () = ();

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::services::ExtractionContext;
    use cognicode_core::domain::value_objects::{DependencyType, Provenance};

    fn build_small_graph() -> CallGraph {
        use cognicode_core::domain::aggregates::Symbol;
        use cognicode_core::domain::value_objects::{Location, SymbolKind};
        use cognicode_core::domain::services::ExtractionContext;

        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "beta",
            SymbolKind::Function,
            Location::new("src/b.rs", 1, 0),
        ));
        g.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("add dep");
        g
    }

    #[test]
    fn versioned_blob_encode_v2_roundtrip_preserves_metadata() {
        let g = build_small_graph();
        let bytes = VersionedBlob::encode_v2(&g).expect("encode v2");
        // Magic + version + payload.
        assert_eq!(&bytes[..4], b"CCG1");
        assert_eq!(bytes[4], BLOB_VERSION_CURRENT);

        let decoded = VersionedBlob::decode(&bytes).expect("decode v2");
        assert_eq!(decoded.symbol_count(), 2);
        assert_eq!(decoded.edge_count(), 1);
        // Metadata must roundtrip exactly.
        let metas: Vec<_> = decoded.edges_with_metadata().collect();
        assert_eq!(metas.len(), 1);
        let (_src, _tgt, _dep, prov, conf) = &metas[0];
        assert_eq!(*prov, Provenance::Extracted);
        assert_eq!(*conf, 1.0_f64);
    }

    #[test]
    fn versioned_blob_decode_v1_legacy_assigns_extracted_one() {
        use cognicode_core::domain::aggregates::Symbol;
        use cognicode_core::domain::value_objects::{Location, SymbolKind};
        use cognicode_core::domain::aggregates::SymbolId;

        // Hand-craft a v1 blob (no magic header).
        #[allow(deprecated)]
        let mut v1 = CallGraphV1::new();
        let sa = Symbol::new("alpha", SymbolKind::Function, Location::new("src/a.rs", 1, 0));
        let sb = Symbol::new("beta", SymbolKind::Function, Location::new("src/b.rs", 1, 0));
        let id_a = SymbolId::new(sa.fully_qualified_name());
        let id_b = SymbolId::new(sb.fully_qualified_name());
        v1.symbols.insert(id_a.clone(), sa);
        v1.symbols.insert(id_b.clone(), sb);
        v1.edges.insert(
            id_a.clone(),
            std::iter::once((id_b.clone(), DependencyType::Calls)).collect(),
        );
        let bytes = bincode::serde::encode_to_vec(&v1, bincode::config::standard())
            .expect("encode v1");
        // Sanity: a v1 blob has no `CCG1` prefix.
        assert_ne!(&bytes[..4.min(bytes.len())], b"CCG1");

        let decoded = VersionedBlob::decode(&bytes).expect("decode v1");
        assert_eq!(decoded.symbol_count(), 2);
        assert_eq!(decoded.edge_count(), 1);
        // Every edge must be (Extracted, 1.0).
        for (_src, _tgt, _dep, prov, conf) in decoded.edges_with_metadata() {
            assert_eq!(prov, Provenance::Extracted);
            assert_eq!(conf, 1.0_f64);
        }
    }

    #[test]
    fn versioned_blob_decode_bad_magic_with_unknown_version_is_corrupted() {
        // Magic present, but version = 99 → Corrupted.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"CCG1");
        bytes.push(99);
        bytes.extend_from_slice(&[0u8; 4]);
        let result = VersionedBlob::decode(&bytes);
        match result {
            Err(StoreError::Corrupted(msg)) => {
                assert!(msg.contains("unsupported"), "msg = {msg}");
                assert!(msg.contains("99"), "msg = {msg}");
            }
            other => panic!("expected Corrupted, got {other:?}"),
        }
    }

    #[test]
    fn versioned_blob_decode_empty_is_serialization_error() {
        // Empty payload cannot be a v1 blob nor a v2 blob.
        let result = VersionedBlob::decode(&[]);
        assert!(matches!(result, Err(StoreError::Serialization(_))));
    }

    // -------------------------------------------------------------------------
    // SQLite integration tests — schema v2 roundtrip with metadata (Phase 4)
    // -------------------------------------------------------------------------

    use tempfile::tempdir;

    fn open_temp_graph_store() -> (tempfile::TempDir, SqliteGraphStore) {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("graph.db");
        let store = SqliteGraphStore::open(&path).expect("open store");
        (dir, store)
    }

    #[test]
    fn sqlite_save_load_roundtrip_preserves_provenance_and_confidence() {
        use cognicode_core::domain::aggregates::Symbol;
        use cognicode_core::domain::value_objects::{Location, SymbolKind};

        let (_dir, store) = open_temp_graph_store();

        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "beta",
            SymbolKind::Function,
            Location::new("src/b.rs", 1, 0),
        ));
        let c = g.add_symbol(Symbol::new(
            "gamma",
            SymbolKind::Function,
            Location::new("src/c.rs", 1, 0),
        ));
        g.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("add dep a→b");
        g.add_dependency_with_provenance(
            &a,
            &c,
            DependencyType::Imports,
            ExtractionContext::Heuristic { score: 0.6 },
        )
        .expect("add dep a→c");
        g.add_dependency_with_provenance(
            &b,
            &c,
            DependencyType::References,
            ExtractionContext::Unresolved,
        )
        .expect("add dep b→c");

        store.save_graph(&g).expect("save graph");
        let loaded = store.load_graph().expect("load graph").expect("some graph");

        // All three edges must roundtrip with their metadata intact.
        let mut metas: Vec<_> = loaded
            .edges_with_metadata()
            .map(|(s, t, d, p, c)| (s, t, d, p, c))
            .collect();
        metas.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        assert_eq!(metas.len(), 3);

        // a→b = (Extracted, 1.0)
        let ab = metas
            .iter()
            .find(|(s, t, _, _, _)| s.as_str() < t.as_str() && s == &a && t == &b)
            .expect("a→b edge present");
        assert_eq!(ab.2, DependencyType::Calls);
        assert_eq!(ab.3, Provenance::Extracted);
        assert_eq!(ab.4, 1.0_f64);

        // a→c = (Inferred, 0.6)
        let ac = metas
            .iter()
            .find(|(s, t, _, _, _)| s == &a && t == &c)
            .expect("a→c edge present");
        assert_eq!(ac.2, DependencyType::Imports);
        assert_eq!(ac.3, Provenance::Inferred);
        assert_eq!(ac.4, 0.6_f64);

        // b→c = (Ambiguous, 0.3)
        let bc = metas
            .iter()
            .find(|(s, t, _, _, _)| s == &b && t == &c)
            .expect("b→c edge present");
        assert_eq!(bc.2, DependencyType::References);
        assert_eq!(bc.3, Provenance::Ambiguous);
        assert_eq!(bc.4, 0.3_f64);
    }

    #[test]
    fn sqlite_call_edges_table_contains_metadata_columns() {
        use cognicode_core::domain::aggregates::Symbol;
        use cognicode_core::domain::value_objects::{Location, SymbolKind};

        // We need the on-disk path to open a second handle. Build the
        // store path directly so the test owns it.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("graph.db");
        let store = SqliteGraphStore::open(&path).expect("open store");

        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "beta",
            SymbolKind::Function,
            Location::new("src/b.rs", 1, 0),
        ));
        g.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::Heuristic { score: 0.7 },
        )
        .expect("add dep");
        store.save_graph(&g).expect("save graph");

        // Open a second handle and SELECT the v2 columns directly.
        let db = rusqlite::Connection::open(&path).expect("open db read");
        let mut stmt = db
            .prepare(
                "SELECT caller_id, callee_id, dependency_type, provenance, confidence \
                 FROM call_edges ORDER BY id",
            )
            .expect("prepare");
        let rows: Vec<(String, String, String, String, f64)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("query map")
            .map(|r| r.expect("row"))
            .collect();
        assert_eq!(rows.len(), 1);
        let (caller_id, callee_id, dep, prov, conf) = &rows[0];
        assert_eq!(caller_id, a.as_str());
        assert_eq!(callee_id, b.as_str());
        assert_eq!(dep, "Calls");
        assert_eq!(prov, "Inferred");
        assert!((conf - 0.7).abs() < 1e-9);
    }
}
