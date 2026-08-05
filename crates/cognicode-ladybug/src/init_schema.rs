//! Schema initialization helpers for LadybugDB node tables.
//!
//! Each `init_*` function applies the DDL for a specific port's backing
//! tables. All statements use `IF NOT EXISTS` for idempotency.

use lbug::Connection;

use crate::Error;

/// DDL statements for the `NarrativeView` node table backing the
/// [`cognicode_core::domain::ports::NarrativeStore`] port.
///
/// Synthetic PK: `{}::{}::{}` of `(workspace_id, view_id, object_id)`.
fn narrative_view_ddls() -> Vec<&'static str> {
    vec![
        "CREATE NODE TABLE IF NOT EXISTS NarrativeView(\
             id TEXT PRIMARY KEY,\
             workspace_id TEXT NOT NULL,\
             view_id TEXT NOT NULL,\
             object_id TEXT NOT NULL,\
             view_kind TEXT NOT NULL,\
             payload TEXT NOT NULL,\
             source_rev INT64 NOT NULL,\
             created_at TEXT NOT NULL);",
        "CREATE INDEX IF NOT EXISTS idx_narrative_view_ws ON NarrativeView(workspace_id);",
        "CREATE INDEX IF NOT EXISTS idx_narrative_view_ws_kind ON NarrativeView(workspace_id, view_kind);",
    ]
}

/// Create the `NarrativeView` node table and indexes backing the
/// [`NarrativeStore`] port.
///
/// Idempotent — every statement uses `IF NOT EXISTS`.
///
/// Called automatically by [`super::LadybugStore::open`]; the raw sharing
/// constructor [`super::LadybugStore::new`] does NOT apply it so tests can
/// exercise the graceful-degradation contract on a schema-less db.
pub fn init_narrative_view_schema(conn: &Connection) -> Result<(), Error> {
    for stmt in narrative_view_ddls() {
        conn.query(stmt)
            .map_err(|e| Error::Lbug(format!("init_narrative_view_schema: {e}\nDDL: {stmt}")))?;
    }
    Ok(())
}