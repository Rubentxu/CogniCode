//! `NarrativeStore` adapter implementation for LadybugDB.
//!
//! Mirrors the [`QualityStore`] pattern: synthetic `id`, read-then-conditional-write
//! for upserts, and graceful degradation on missing tables.

#[cfg(test)]
use std::sync::Arc;

use async_trait::async_trait;

use cognicode_core::domain::ports::narrative_store::{
    NarrativeError, NarrativeSnapshot, NarrativeStore,
};

#[cfg(test)]
use tempfile::TempDir;

use crate::LadybugStore;

/// Return `true` when an lbug error is caused by a missing node table.
fn is_missing_table(e: &lbug::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("does not exist")
        || msg.contains("not exist")
        || msg.contains("not found")
        || msg.contains("unknown table")
        || msg.contains("no table")
}

#[async_trait]
impl NarrativeStore for LadybugStore {
    async fn save_snapshot(&self, snap: &NarrativeSnapshot) -> Result<(), NarrativeError> {
        let conn = self
            .connection()
            .map_err(|e| NarrativeError::Database(format!("save_snapshot connection: {e}")))?;

        // Synthetic PK: {}::{}::{} of (workspace_id, view_id, object_id)
        let id = format!(
            "{}::{}::{}",
            snap.workspace_id, snap.view_id, snap.object_id
        );

        let params = vec![
            ("id", lbug::Value::String(id.clone())),
            ("ws", lbug::Value::String(snap.workspace_id.clone())),
            ("vid", lbug::Value::String(snap.view_id.clone())),
            ("oid", lbug::Value::String(snap.object_id.clone())),
            ("kind", lbug::Value::String(snap.view_kind.clone())),
            ("payload", lbug::Value::String(snap.payload.clone())),
            ("rev", lbug::Value::Int64(snap.source_rev as i64)),
            ("ts", lbug::Value::String(snap.created_at.clone())),
        ];

        let update_cypher = "MATCH (n:NarrativeView) WHERE n.id = $id \
             SET n.workspace_id = $ws, n.view_id = $vid, n.object_id = $oid, \
                 n.view_kind = $kind, n.payload = $payload, n.source_rev = $rev, n.created_at = $ts \
             RETURN count(n);";

        // Try UPDATE first (lbug 0.19 has no MERGE).
        let mut upd_stmt = match conn.prepare(update_cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => {
                // Table missing — degrade gracefully by creating the row.
                let ins_cypher = "CREATE (n:NarrativeView {id: $id, workspace_id: $ws, \
                     view_id: $vid, object_id: $oid, view_kind: $kind, payload: $payload, \
                     source_rev: $rev, created_at: $ts});";
                let mut stmt = conn.prepare(ins_cypher).map_err(|e| {
                    NarrativeError::Database(format!("save_snapshot insert prepare: {e}"))
                })?;
                conn.execute(&mut stmt, params).map_err(|e| {
                    NarrativeError::Database(format!("save_snapshot insert exec: {e}"))
                })?;
                return Ok(());
            }
            Err(e) => {
                return Err(NarrativeError::Database(format!(
                    "save_snapshot update prepare: {e}"
                )));
            }
        };

        let mut result = conn
            .execute(&mut upd_stmt, params.clone())
            .map_err(|e| NarrativeError::Database(format!("save_snapshot update execute: {e}")))?;

        let updated: i64 = result
            .next()
            .map(|row| match &row[0] {
                lbug::Value::Int64(n) => *n,
                lbug::Value::Int32(n) => *n as i64,
                _ => 0,
            })
            .unwrap_or(0);

        if updated == 0 {
            // Row doesn't exist — INSERT.
            let ins_cypher = "CREATE (n:NarrativeView {id: $id, workspace_id: $ws, \
                 view_id: $vid, object_id: $oid, view_kind: $kind, payload: $payload, \
                 source_rev: $rev, created_at: $ts});";
            drop(upd_stmt);
            let mut stmt = conn.prepare(ins_cypher).map_err(|e| {
                NarrativeError::Database(format!("save_snapshot insert prepare: {e}"))
            })?;
            conn.execute(&mut stmt, params)
                .map_err(|e| NarrativeError::Database(format!("save_snapshot insert exec: {e}")))?;
        }

        Ok(())
    }

    async fn load_snapshot(
        &self,
        ws: &str,
        view_id: &str,
        object_id: &str,
    ) -> Result<Option<NarrativeSnapshot>, NarrativeError> {
        let conn = self
            .connection()
            .map_err(|e| NarrativeError::Database(format!("load_snapshot connection: {e}")))?;

        let id = format!("{}::{}::{}", ws, view_id, object_id);
        let mut stmt = match conn.prepare(
            "MATCH (n:NarrativeView) WHERE n.id = $id \
             RETURN n.id, n.workspace_id, n.view_id, n.object_id, n.view_kind, \
                    n.payload, n.source_rev, n.created_at;",
        ) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(None),
            Err(e) => {
                return Err(NarrativeError::Database(format!(
                    "load_snapshot prepare: {e}"
                )));
            }
        };

        let mut result = conn
            .execute(&mut stmt, vec![("id", lbug::Value::String(id))])
            .map_err(|e| NarrativeError::Database(format!("load_snapshot execute: {e}")))?;

        match result.next() {
            Some(row) => Ok(Some(narrative_snapshot_from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn list_for_workspace(
        &self,
        ws: &str,
        view_kind: Option<&str>,
    ) -> Result<Vec<NarrativeSnapshot>, NarrativeError> {
        let conn = self
            .connection()
            .map_err(|e| NarrativeError::Database(format!("list_for_workspace connection: {e}")))?;

        let (cypher, params): (&str, Vec<(&str, lbug::Value)>) = match view_kind {
            Some(kind) => (
                "MATCH (n:NarrativeView) WHERE n.workspace_id = $ws AND n.view_kind = $kind \
                 RETURN n.id, n.workspace_id, n.view_id, n.object_id, n.view_kind, \
                        n.payload, n.source_rev, n.created_at \
                 ORDER BY n.created_at DESC;",
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("kind", lbug::Value::String(kind.to_string())),
                ],
            ),
            None => (
                "MATCH (n:NarrativeView) WHERE n.workspace_id = $ws \
                 RETURN n.id, n.workspace_id, n.view_id, n.object_id, n.view_kind, \
                        n.payload, n.source_rev, n.created_at \
                 ORDER BY n.created_at DESC;",
                vec![("ws", lbug::Value::String(ws.to_string()))],
            ),
        };

        let mut stmt = match conn.prepare(cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(Vec::new()),
            Err(e) => {
                return Err(NarrativeError::Database(format!(
                    "list_for_workspace prepare: {e}"
                )));
            }
        };

        let result = conn
            .execute(&mut stmt, params)
            .map_err(|e| NarrativeError::Database(format!("list_for_workspace execute: {e}")))?;

        let mut snapshots = Vec::new();
        for row in result {
            snapshots.push(narrative_snapshot_from_row(&row)?);
        }
        Ok(snapshots)
    }

    async fn invalidate(&self, ws: &str, source_rev: u64) -> Result<u64, NarrativeError> {
        let conn = self
            .connection()
            .map_err(|e| NarrativeError::Database(format!("invalidate connection: {e}")))?;

        let mut stmt = match conn.prepare(
            "MATCH (n:NarrativeView) WHERE n.workspace_id = $ws AND n.source_rev <= $rev \
             DELETE n RETURN count(n);",
        ) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(0),
            Err(e) => {
                return Err(NarrativeError::Database(format!("invalidate prepare: {e}")));
            }
        };

        let mut result = conn
            .execute(
                &mut stmt,
                vec![
                    ("ws", lbug::Value::String(ws.to_string())),
                    ("rev", lbug::Value::Int64(source_rev as i64)),
                ],
            )
            .map_err(|e| NarrativeError::Database(format!("invalidate execute: {e}")))?;

        Ok(match result.next() {
            Some(row) => match &row[0] {
                lbug::Value::Int64(n) => *n as u64,
                lbug::Value::Int32(n) => *n as u64,
                _ => 0,
            },
            None => 0,
        })
    }
}

/// Map a NarrativeView row into a [`NarrativeSnapshot`].
///
/// Row order: `id, workspace_id, view_id, object_id, view_kind, payload, source_rev, created_at`.
fn narrative_snapshot_from_row(row: &[lbug::Value]) -> Result<NarrativeSnapshot, NarrativeError> {
    fn str_at(row: &[lbug::Value], idx: usize) -> String {
        row.get(idx).map(|v| v.to_string()).unwrap_or_default()
    }

    fn req_i64(row: &[lbug::Value], idx: usize) -> i64 {
        match row.get(idx) {
            Some(lbug::Value::Int64(n)) => *n,
            Some(lbug::Value::Int32(n)) => *n as i64,
            _ => 0,
        }
    }

    Ok(NarrativeSnapshot {
        id: str_at(row, 0),
        workspace_id: str_at(row, 1),
        view_id: str_at(row, 2),
        object_id: str_at(row, 3),
        view_kind: str_at(row, 4),
        payload: str_at(row, 5),
        source_rev: req_i64(row, 6) as u64,
        created_at: str_at(row, 7),
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: open a temporary LadybugDB with narrative schema initialized.
    /// LadybugStore::open() already calls init_narrative_view_schema() idempotently.
    fn temp_store() -> (LadybugStore, TempDir) {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let path = tmp_dir.path().join("narrative.lbdb");
        let store = LadybugStore::open(&path).expect("open temp store");
        (store, tmp_dir) // TempDir kept alive to keep path valid
    }

    #[tokio::test]
    async fn test_save_and_load_snapshot() {
        let (store, _tmp) = temp_store();
        let snap = NarrativeSnapshot {
            id: "ws1::view1::obj1".to_string(),
            workspace_id: "ws1".to_string(),
            view_id: "view1".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "file".to_string(),
            payload: r#"{"key":"value"}"#.to_string(),
            source_rev: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        store
            .save_snapshot(&snap)
            .await
            .expect("save_snapshot should succeed");

        let loaded = store
            .load_snapshot("ws1", "view1", "obj1")
            .await
            .expect("load_snapshot should succeed")
            .expect("snapshot should exist");

        assert_eq!(loaded.id, snap.id);
        assert_eq!(loaded.workspace_id, snap.workspace_id);
        assert_eq!(loaded.view_id, snap.view_id);
        assert_eq!(loaded.object_id, snap.object_id);
        assert_eq!(loaded.view_kind, snap.view_kind);
        assert_eq!(loaded.payload, snap.payload);
        assert_eq!(loaded.source_rev, snap.source_rev);
        assert_eq!(loaded.created_at, snap.created_at);
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let (store, _tmp) = temp_store();

        let snap1 = NarrativeSnapshot {
            id: "ws1::view1::obj1".to_string(),
            workspace_id: "ws1".to_string(),
            view_id: "view1".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "file".to_string(),
            payload: r#"{"v":1}"#.to_string(),
            source_rev: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        store.save_snapshot(&snap1).await.expect("first save");

        let snap2 = NarrativeSnapshot {
            id: "ws1::view1::obj1".to_string(),
            workspace_id: "ws1".to_string(),
            view_id: "view1".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "file".to_string(),
            payload: r#"{"v":2}"#.to_string(),
            source_rev: 2,
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        store.save_snapshot(&snap2).await.expect("second save");

        // Should have the updated payload.
        let loaded = store
            .load_snapshot("ws1", "view1", "obj1")
            .await
            .expect("load after upsert")
            .expect("snapshot should exist");
        assert_eq!(loaded.payload, r#"{"v":2}"#);
        assert_eq!(loaded.source_rev, 2);
    }

    #[tokio::test]
    async fn test_load_returns_none_on_miss() {
        let (store, _tmp) = temp_store();
        let result = store
            .load_snapshot("nonexistent", "view", "obj")
            .await
            .expect("load should not error");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_for_workspace() {
        let (store, _tmp) = temp_store();

        for i in 1..=3 {
            let snap = NarrativeSnapshot {
                id: format!("ws1::view{}::obj", i),
                workspace_id: "ws1".to_string(),
                view_id: format!("view{}", i),
                object_id: "obj".to_string(),
                view_kind: "file".to_string(),
                payload: format!(r#"{{"n":{}}}"#, i),
                source_rev: 1,
                created_at: format!("2024-01-{:02}T00:00:00Z", i),
            };
            store.save_snapshot(&snap).await.expect("save");
        }

        // Add one for a different workspace.
        let other = NarrativeSnapshot {
            id: "ws2::view1::obj1".to_string(),
            workspace_id: "ws2".to_string(),
            view_id: "view1".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "file".to_string(),
            payload: r#"{"other":true}"#.to_string(),
            source_rev: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        store.save_snapshot(&other).await.expect("save other ws");

        let list = store
            .list_for_workspace("ws1", None)
            .await
            .expect("list should succeed");
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_list_filtered_by_view_kind() {
        let (store, _tmp) = temp_store();

        let file_snap = NarrativeSnapshot {
            id: "ws1::view_file::obj1".to_string(),
            workspace_id: "ws1".to_string(),
            view_id: "view_file".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "file".to_string(),
            payload: r#"{"type":"file"}"#.to_string(),
            source_rev: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        store
            .save_snapshot(&file_snap)
            .await
            .expect("save file snap");

        let scope_snap = NarrativeSnapshot {
            id: "ws1::view_scope::obj1".to_string(),
            workspace_id: "ws1".to_string(),
            view_id: "view_scope".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "scope".to_string(),
            payload: r#"{"type":"scope"}"#.to_string(),
            source_rev: 1,
            created_at: "2024-01-02T00:00:00Z".to_string(),
        };
        store
            .save_snapshot(&scope_snap)
            .await
            .expect("save scope snap");

        let file_list = store
            .list_for_workspace("ws1", Some("file"))
            .await
            .expect("list file kind");
        assert_eq!(file_list.len(), 1);
        assert_eq!(file_list[0].view_kind, "file");

        let scope_list = store
            .list_for_workspace("ws1", Some("scope"))
            .await
            .expect("list scope kind");
        assert_eq!(scope_list.len(), 1);
        assert_eq!(scope_list[0].view_kind, "scope");
    }

    #[tokio::test]
    async fn test_invalidate_deletes_by_source_rev() {
        let (store, _tmp) = temp_store();

        // Save 3 snapshots with different source_rev.
        for rev in [1u64, 2, 3] {
            let snap = NarrativeSnapshot {
                id: format!("ws1::view1::obj{}", rev),
                workspace_id: "ws1".to_string(),
                view_id: "view1".to_string(),
                object_id: format!("obj{}", rev),
                view_kind: "file".to_string(),
                payload: format!(r#"{{"rev":{}}}"#, rev),
                source_rev: rev,
                created_at: format!("2024-01-{:02}T00:00:00Z", rev),
            };
            store.save_snapshot(&snap).await.expect("save");
        }

        // Invalidate at source_rev=2 — should delete rev 1 and 2.
        let deleted = store
            .invalidate("ws1", 2)
            .await
            .expect("invalidate should succeed");
        assert_eq!(deleted, 2);

        // rev=3 should still exist.
        let remaining = store
            .list_for_workspace("ws1", None)
            .await
            .expect("list after invalidate");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].source_rev, 3);
    }

    /// Scenario 8: invalidate returns 0 when no snapshots match.
    #[tokio::test]
    async fn test_invalidate_returns_zero_when_no_match() {
        let (store, _tmp) = temp_store();

        let deleted = store
            .invalidate("ws_nonexistent", 99)
            .await
            .expect("invalidate should succeed");
        assert_eq!(deleted, 0);
    }

    /// Scenario 9: save_snapshot gracefully creates row on missing table
    /// (follows QualityStore pattern — table auto-creation via CREATE).
    #[tokio::test]
    async fn test_save_snapshot_on_missing_table() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let path = tmp_dir.path().join("narrative_missing.lbdb");
        let store = LadybugStore::open(&path).expect("open store");

        let snap = NarrativeSnapshot {
            id: "ws1::view1::obj1".to_string(),
            workspace_id: "ws1".to_string(),
            view_id: "view1".to_string(),
            object_id: "obj1".to_string(),
            view_kind: "file".to_string(),
            payload: r#"{"key":"value"}"#.to_string(),
            source_rev: 1,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };

        // Must not error — table doesn't exist but save degrades gracefully.
        store
            .save_snapshot(&snap)
            .await
            .expect("save_snapshot should degrade gracefully on missing table");

        // Verify it was persisted.
        let loaded = store
            .load_snapshot("ws1", "view1", "obj1")
            .await
            .expect("load after graceful save")
            .expect("snapshot should exist after graceful save");
        assert_eq!(loaded.payload, r#"{"key":"value"}"#);
    }

    /// Scenario 10: load_snapshot returns None on missing table
    /// (follows QualityStore pattern — missing table means empty).
    #[tokio::test]
    async fn test_load_snapshot_on_missing_table() {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let path = tmp_dir.path().join("narrative_missing_load.lbdb");
        let store = LadybugStore::open(&path).expect("open store");

        let result = store
            .load_snapshot("ws1", "view1", "obj1")
            .await
            .expect("load_snapshot should not error on missing table");
        assert!(
            result.is_none(),
            "missing table should return None, not error"
        );
    }
}
