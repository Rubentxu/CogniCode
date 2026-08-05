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
             id STRING PRIMARY KEY,\
             workspace_id STRING,\
             view_id STRING,\
             object_id STRING,\
             view_kind STRING,\
             payload STRING,\
             source_rev INT64,\
             created_at STRING);",
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
