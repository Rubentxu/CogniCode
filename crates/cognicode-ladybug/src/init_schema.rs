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

/// DDL statements for the `KnowledgeEvidence` node table backing the
/// [`cognicode_core::domain::ports::EvidenceStore`] port (E1.W1).
///
/// PK: synthetic `id` of the form `evidence:{}::{}` of
/// `(workspace_id, evidence_id)` so a workspace can host multiple
/// evidence entries without colliding on numeric ids. The
/// `evidence_id` field stores the same suffix as a redundant
/// indexable string for queries.
///
/// Schema fields mirror [`cognicode_core::domain::ports::evidence_store::EvidenceSummary`]:
///   - `kind` is stored as a STRING (the lbug 0.19 API does not expose a
///     native enum type, and the variant count is small). Valid values:
///     `log`, `trace`, `measurement`, `external`.
///
/// **Naming conflict**: the table is named `KnowledgeEvidence` and
/// NOT `Evidence` because the existing `RunLineageStore` already
/// owns an `Evidence` node table (provenance of runs, with a
/// completely different schema: `id SERIAL`, `revision_id INT64`,
/// `source_kind STRING`, `source_ref STRING`, `valid_from/to INT64`,
/// `properties MAP(STRING, STRING)`). This is exactly the
/// "EvidenceStore en domain::evidence_kernel::ports ≠
/// domain::ports::evidence_store" conflict FINAL-STATE §31 warned
/// about. The namespacing choice (`KnowledgeEvidence`) keeps the
/// two tables coexisting in the same DB without ambiguity; the
/// future ADR-010 will document the namespace split.
///
/// Idempotent — `IF NOT EXISTS` on every statement. No FK, no unique
/// indexes beyond the PK (the trait is read-only; insertion paths
/// are out of scope for E1.W1). The future `ladybug-evidence-writer`
/// port (not in this PR) will own the writer side.
///
/// NOTE: written as a single-line string. A previous multi-line
/// variant (with `\`-continuation) appeared to compile but was silently
/// no-op'd by lbug 0.19 — the binder accepted the multi-spaced text
/// but did not register the table. Single-line avoids that ambiguity.
fn evidence_ddls() -> Vec<&'static str> {
    vec![
        "CREATE NODE TABLE IF NOT EXISTS KnowledgeEvidence(id STRING PRIMARY KEY, workspace_id STRING, evidence_id STRING, title STRING, kind STRING, source_path STRING, excerpt STRING, confidence DOUBLE);",
    ]
}

/// Create the `KnowledgeEvidence` node table backing the
/// [`EvidenceStore`] port.
///
/// Idempotent — every statement uses `IF NOT EXISTS`.
///
/// Called automatically by [`super::LadybugStore::open`]; the raw sharing
/// constructor [`super::LadybugStore::new`] does NOT apply it so tests can
/// exercise the graceful-degradation contract on a schema-less db
/// (see `evidence_store::tests::test_list_on_missing_table`).
pub fn init_evidence_schema(conn: &Connection) -> Result<(), Error> {
    for stmt in evidence_ddls() {
        conn.query(stmt)
            .map_err(|e| Error::Lbug(format!("init_evidence_schema: {e}\nDDL: {stmt}")))?;
    }
    Ok(())
}
