//! `EvidenceStore` adapter implementation for LadybugDB.
//!
//! E1.W1: read-only adapter for the [`EvidenceStore`] port, backed by the
//! `Evidence` node table (DDL in [`crate::init_schema::init_evidence_schema`]).
//!
//! Mirrors the [`crate::NarrativeStore`] pattern: graceful degradation on
//! missing tables (returns `Ok(Vec::new())` for `list_evidence` and
//! `Ok(Vec::new())` for `search_evidence` when the table is absent, so
//! schema-less databases — e.g. tests using `LadybugStore::new` — behave
//! like an empty store instead of erroring).
//!
//! Schema (see `init_schema.rs::evidence_ddls`):
//!   - `id` STRING PRIMARY KEY (synthetic `evidence:{ws}::{evidence_id}`)
//!   - `workspace_id` STRING
//!   - `evidence_id` STRING (suffix of `id`)
//!   - `title` STRING
//!   - `kind` STRING (`log`|`trace`|`measurement`|`external`)
//!   - `source_path` STRING (nullable)
//!   - `excerpt` STRING (nullable)
//!   - `confidence` DOUBLE
//!
//! The trait is read-only; the writer side is the future
//! `ladybug-evidence-writer` port (not in this PR — see ADR-009 §Decisión 2).

use cognicode_core::domain::ports::evidence_store::{
    EvidenceError, EvidenceKind, EvidenceStore, EvidenceSummary,
};

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

/// Map the persisted `kind` STRING back to the [`EvidenceKind`] enum.
///
/// Returns `None` when the persisted value does not match any variant —
/// the caller should filter those rows out (graceful handling of
/// forward-compatible schema additions).
fn parse_kind(s: &str) -> Option<EvidenceKind> {
    match s {
        "log" => Some(EvidenceKind::Log),
        "trace" => Some(EvidenceKind::Trace),
        "measurement" => Some(EvidenceKind::Measurement),
        "external" => Some(EvidenceKind::External),
        _ => None,
    }
}

/// Render an [`EvidenceKind`] enum to its persisted STRING form.
fn kind_to_str(k: EvidenceKind) -> &'static str {
    match k {
        EvidenceKind::Log => "log",
        EvidenceKind::Trace => "trace",
        EvidenceKind::Measurement => "measurement",
        EvidenceKind::External => "external",
    }
}

/// Build the synthetic PK from a workspace + evidence id.
///
/// Format: `evidence:{workspace_id}::{evidence_id}`. Stored verbatim
/// in the `id` column; the `evidence_id` column stores the same
/// suffix redundantly for indexed queries.
fn evidence_pk(workspace_id: &str, evidence_id: &str) -> String {
    format!("evidence:{}::{}", workspace_id, evidence_id)
}

impl EvidenceStore for LadybugStore {
    fn list_evidence(
        &self,
        workspace: &str,
        kind: Option<EvidenceKind>,
    ) -> Result<Vec<EvidenceSummary>, EvidenceError> {
        let conn = self
            .connection()
            .map_err(|e| EvidenceError::Store(format!("list_evidence connection: {e}")))?;

        let (cypher, params): (&str, Vec<(&str, lbug::Value)>) = match kind {
            Some(k) => {
                let ks = kind_to_str(k);
                (
                    "MATCH (n:KnowledgeEvidence) WHERE n.workspace_id = $ws AND n.kind = $kind \
                     RETURN n.evidence_id, n.title, n.kind, n.source_path, \
                            n.excerpt, n.confidence \
                     ORDER BY n.evidence_id;",
                    vec![
                        ("ws", lbug::Value::String(workspace.to_string())),
                        ("kind", lbug::Value::String(ks.to_string())),
                    ],
                )
            }
            None => (
                "MATCH (n:KnowledgeEvidence) WHERE n.workspace_id = $ws \
                 RETURN n.evidence_id, n.title, n.kind, n.source_path, \
                        n.excerpt, n.confidence \
                 ORDER BY n.evidence_id;",
                vec![("ws", lbug::Value::String(workspace.to_string()))],
            ),
        };

        let mut stmt = match conn.prepare(cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(Vec::new()),
            Err(e) => {
                return Err(EvidenceError::Store(format!("list_evidence prepare: {e}")));
            }
        };

        let mut result = conn
            .execute(&mut stmt, params)
            .map_err(|e| EvidenceError::Store(format!("list_evidence execute: {e}")))?;

        let mut out = Vec::new();
        for row in result {
            let evidence_id = match &row[0] {
                lbug::Value::String(s) => s.clone(),
                _ => continue,
            };
            let title = match &row[1] {
                lbug::Value::String(s) => s.clone(),
                _ => continue,
            };
            let kind_str = match &row[2] {
                lbug::Value::String(s) => s.clone(),
                _ => continue,
            };
            let Some(kind) = parse_kind(&kind_str) else {
                // Forward-compatible: unknown kind (e.g. a future variant
                // not yet implemented by this adapter) is filtered out.
                continue;
            };
            let source_path = match &row[3] {
                lbug::Value::String(s) if !s.is_empty() => Some(s.clone()),
                lbug::Value::Null(_) => None,
                _ => None,
            };
            let excerpt = match &row[4] {
                lbug::Value::String(s) if !s.is_empty() => Some(s.clone()),
                lbug::Value::Null(_) => None,
                _ => None,
            };
            let confidence = match &row[5] {
                lbug::Value::Double(d) => *d as f32,
                _ => 0.0,
            };

            out.push(EvidenceSummary {
                id: evidence_pk(workspace, &evidence_id),
                title,
                kind,
                source_path,
                excerpt,
                confidence,
            });
        }
        Ok(out)
    }

    fn search_evidence(
        &self,
        workspace: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceSummary>, EvidenceError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self
            .connection()
            .map_err(|e| EvidenceError::Store(format!("search_evidence connection: {e}")))?;

        // lbug 0.19 has no `to_lower` / `to_upper` functions; its
        // `CONTAINS` is case-sensitive. To preserve the
        // case-insensitive substring contract across title + excerpt
        // we pull all candidate rows for the workspace and filter
        // in Rust with proper Unicode-aware case folding. Acceptable
        // for the current dataset sizes (hundreds of evidence rows
        // per workspace); if profiling shows hot, replace with an
        // FTS-indexed column or a `LOWER(title) STRING` mirror column.
        let q_lower = query.to_lowercase();
        let cypher = "MATCH (n:KnowledgeEvidence) \
             WHERE n.workspace_id = $ws \
             RETURN n.evidence_id, n.title, n.kind, n.source_path, \
                    n.excerpt, n.confidence \
             ORDER BY n.evidence_id;";

        let mut stmt = match conn.prepare(cypher) {
            Ok(stmt) => stmt,
            Err(e) if is_missing_table(&e) => return Ok(Vec::new()),
            Err(e) => {
                return Err(EvidenceError::Store(format!(
                    "search_evidence prepare: {e}"
                )));
            }
        };

        let mut result = conn
            .execute(
                &mut stmt,
                vec![("ws", lbug::Value::String(workspace.to_string()))],
            )
            .map_err(|e| EvidenceError::Store(format!("search_evidence execute: {e}")))?;

        let mut out = Vec::new();
        for row in result {
            let evidence_id = match &row[0] {
                lbug::Value::String(s) => s.clone(),
                _ => continue,
            };
            let title = match &row[1] {
                lbug::Value::String(s) => s.clone(),
                _ => continue,
            };
            // Case-insensitive substring match against title + excerpt.
            let title_lower = title.to_lowercase();
            let excerpt_str = match &row[4] {
                lbug::Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            };
            let excerpt_lower = excerpt_str.as_deref().map(str::to_lowercase);

            let title_hit = title_lower.contains(&q_lower);
            let excerpt_hit = excerpt_lower
                .as_deref()
                .map(|e| e.contains(&q_lower))
                .unwrap_or(false);
            if !(title_hit || excerpt_hit) {
                continue;
            }

            let kind_str = match &row[2] {
                lbug::Value::String(s) => s.clone(),
                _ => continue,
            };
            let Some(kind) = parse_kind(&kind_str) else {
                continue;
            };
            let source_path = match &row[3] {
                lbug::Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            };
            let confidence = match &row[5] {
                lbug::Value::Double(d) => *d as f32,
                _ => 0.0,
            };

            out.push(EvidenceSummary {
                id: evidence_pk(workspace, &evidence_id),
                title,
                kind,
                source_path,
                excerpt: excerpt_str,
                confidence,
            });

            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    // lbug 0.19 is not thread-safe at the connection level; running
    // tests in parallel can corrupt state (occasionally seen with
    // narrative_store + evidence_store running side by side, where
    // a DDL from one test arrives during another's READ). Pinning
    // to a single thread eliminates the race without changing semantics.
    use serial_test::serial;

    /// Helper: open a temporary LadybugDB with evidence schema initialized.
    /// `LadybugStore::open()` already calls `init_evidence_schema()` idempotently.
    fn temp_store() -> (LadybugStore, TempDir) {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let path = tmp_dir.path().join("evidence.lbdb");
        let store = LadybugStore::open(&path).expect("open temp store");
        (store, tmp_dir)
    }

    /// Helper: open a temporary LadybugDB with NO evidence schema applied.
    /// Used to exercise the graceful-degradation contract on missing tables.
    fn raw_store() -> (LadybugStore, TempDir) {
        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let path = tmp_dir.path().join("evidence_raw.lbdb");
        let db = lbug::Database::new(path, lbug::SystemConfig::default()).expect("raw db");
        let store = LadybugStore::new(std::sync::Arc::new(db));
        (store, tmp_dir)
    }

    /// Helper: insert an Evidence row directly via Cypher (bypasses the
    /// writer-side port which is out of scope for E1.W1). Returns the
    /// `evidence_id` used.
    #[allow(clippy::too_many_arguments)]
    fn insert_raw(
        store: &LadybugStore,
        workspace_id: &str,
        evidence_id: &str,
        title: &str,
        kind: &str,
        source_path: Option<&str>,
        excerpt: Option<&str>,
        confidence: f64,
    ) {
        let conn = store.connection().expect("connection");
        let id = evidence_pk(workspace_id, evidence_id);
        let cypher = "CREATE (n:KnowledgeEvidence {\
             id: $id, workspace_id: $ws, evidence_id: $eid, title: $title, \
             kind: $kind, source_path: $sp, excerpt: $ex, confidence: $conf});";
        let mut stmt = conn.prepare(cypher).expect("prepare insert");
        conn.execute(
            &mut stmt,
            vec![
                ("id", lbug::Value::String(id)),
                ("ws", lbug::Value::String(workspace_id.to_string())),
                ("eid", lbug::Value::String(evidence_id.to_string())),
                ("title", lbug::Value::String(title.to_string())),
                ("kind", lbug::Value::String(kind.to_string())),
                (
                    "sp",
                    lbug::Value::String(source_path.unwrap_or("").to_string()),
                ),
                ("ex", lbug::Value::String(excerpt.unwrap_or("").to_string())),
                ("conf", lbug::Value::Double(confidence)),
            ],
        )
        .expect("execute insert");
    }

    // ------------------------------------------------------------------
    // list_evidence scenarios
    // ------------------------------------------------------------------

    #[test]
    #[serial]
    fn test_list_evidence_empty_workspace() {
        let (store, _tmp) = temp_store();
        let list = store
            .list_evidence("ws1", None)
            .expect("list should succeed");
        assert!(list.is_empty(), "empty workspace must yield empty list");
    }

    #[test]
    #[serial]
    fn test_list_evidence_returns_all_for_workspace() {
        let (store, _tmp) = temp_store();

        insert_raw(
            &store,
            "ws1",
            "ev-1",
            "title-1",
            "log",
            Some("/var/log/foo.log"),
            Some("first evidence"),
            0.9,
        );
        insert_raw(&store, "ws1", "ev-2", "title-2", "trace", None, None, 0.5);
        // Different workspace — must NOT appear.
        insert_raw(&store, "ws2", "ev-x", "title-x", "log", None, None, 0.1);

        let list = store
            .list_evidence("ws1", None)
            .expect("list should succeed");
        assert_eq!(list.len(), 2, "ws1 must contain only 2 entries");

        // Ordered by evidence_id alphabetically.
        assert_eq!(list[0].id, "evidence:ws1::ev-1");
        assert_eq!(list[0].title, "title-1");
        assert_eq!(list[0].kind, EvidenceKind::Log);
        assert_eq!(list[0].source_path.as_deref(), Some("/var/log/foo.log"));
        assert_eq!(list[0].excerpt.as_deref(), Some("first evidence"));
        assert!((list[0].confidence - 0.9).abs() < 1e-6);

        assert_eq!(list[1].id, "evidence:ws1::ev-2");
        assert_eq!(list[1].kind, EvidenceKind::Trace);
        assert!(list[1].source_path.is_none());
        assert!(list[1].excerpt.is_none());
        assert!((list[1].confidence - 0.5).abs() < 1e-6);
    }

    #[test]
    #[serial]
    fn test_list_evidence_filtered_by_kind() {
        let (store, _tmp) = temp_store();

        insert_raw(&store, "ws1", "ev-1", "t1", "log", None, None, 1.0);
        insert_raw(&store, "ws1", "ev-2", "t2", "trace", None, None, 1.0);
        insert_raw(&store, "ws1", "ev-3", "t3", "log", None, None, 1.0);
        insert_raw(&store, "ws1", "ev-4", "t4", "measurement", None, None, 1.0);

        let logs = store
            .list_evidence("ws1", Some(EvidenceKind::Log))
            .expect("list logs");
        assert_eq!(logs.len(), 2);
        assert!(logs.iter().all(|e| e.kind == EvidenceKind::Log));

        let measures = store
            .list_evidence("ws1", Some(EvidenceKind::Measurement))
            .expect("list measurements");
        assert_eq!(measures.len(), 1);
        assert_eq!(measures[0].kind, EvidenceKind::Measurement);

        let externals = store
            .list_evidence("ws1", Some(EvidenceKind::External))
            .expect("list external");
        assert!(externals.is_empty(), "no External entries were inserted");
    }

    #[test]
    #[serial]
    fn test_list_evidence_unknown_kind_is_filtered() {
        let (store, _tmp) = temp_store();

        // Insert a row with a kind string the adapter does not know about
        // (forward-compat: future variants are filtered, not errored).
        insert_raw(
            &store,
            "ws1",
            "ev-future",
            "future-kind-evidence",
            "telemetry",
            None,
            None,
            1.0,
        );
        // Plus a normal row.
        insert_raw(&store, "ws1", "ev-known", "known", "log", None, None, 1.0);

        let list = store
            .list_evidence("ws1", None)
            .expect("list should succeed even with unknown kind");
        assert_eq!(
            list.len(),
            1,
            "unknown kind row must be filtered, known one retained"
        );
        assert_eq!(list[0].id, "evidence:ws1::ev-known");
    }

    // ------------------------------------------------------------------
    // search_evidence scenarios
    // ------------------------------------------------------------------

    #[test]
    #[serial]
    fn test_search_evidence_empty_query_returns_empty() {
        let (store, _tmp) = temp_store();
        insert_raw(&store, "ws1", "ev-1", "alpha", "log", None, None, 1.0);

        let res = store
            .search_evidence("ws1", "", 10)
            .expect("search should succeed");
        assert!(res.is_empty(), "empty query must return empty result");
    }

    #[test]
    #[serial]
    fn test_search_evidence_matches_title_substring() {
        let (store, _tmp) = temp_store();

        insert_raw(
            &store,
            "ws1",
            "ev-1",
            "Performance regression",
            "log",
            None,
            Some("baseline 100ms, current 450ms"),
            0.8,
        );
        insert_raw(
            &store,
            "ws1",
            "ev-2",
            "Disk usage warning",
            "measurement",
            None,
            Some("above 90% threshold"),
            0.7,
        );

        let res = store
            .search_evidence("ws1", "perf", 10)
            .expect("search should succeed");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "evidence:ws1::ev-1");
    }

    #[test]
    #[serial]
    fn test_search_evidence_matches_excerpt_substring() {
        let (store, _tmp) = temp_store();

        insert_raw(
            &store,
            "ws1",
            "ev-1",
            "flamegraph",
            "trace",
            None,
            Some("hot loop in crc32 update"),
            0.9,
        );
        insert_raw(
            &store,
            "ws1",
            "ev-2",
            "memory profile",
            "trace",
            None,
            Some("allocations concentrated in arena"),
            0.6,
        );

        let res = store
            .search_evidence("ws1", "crc32", 10)
            .expect("search should succeed");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "evidence:ws1::ev-1");
    }

    #[test]
    #[serial]
    fn test_search_evidence_is_case_insensitive() {
        let (store, _tmp) = temp_store();
        insert_raw(
            &store,
            "ws1",
            "ev-1",
            "PERFORMANCE bottleneck",
            "log",
            None,
            None,
            1.0,
        );

        let res = store
            .search_evidence("ws1", "performance", 10)
            .expect("search should succeed");
        assert_eq!(res.len(), 1);
    }

    #[test]
    #[serial]
    fn test_search_evidence_respects_limit() {
        let (store, _tmp) = temp_store();

        for i in 0..5 {
            insert_raw(
                &store,
                "ws1",
                &format!("ev-{}", i),
                "matching title",
                "log",
                None,
                None,
                1.0,
            );
        }

        let res = store
            .search_evidence("ws1", "matching", 2)
            .expect("search should succeed");
        assert_eq!(res.len(), 2, "limit must cap the result count");
    }

    #[test]
    #[serial]
    fn test_search_evidence_isolates_workspace() {
        let (store, _tmp) = temp_store();

        insert_raw(
            &store,
            "ws1",
            "ev-1",
            "ws1 only evidence",
            "log",
            None,
            None,
            1.0,
        );
        insert_raw(
            &store,
            "ws2",
            "ev-1",
            "ws2 only evidence",
            "log",
            None,
            None,
            1.0,
        );

        let ws1_hits = store
            .search_evidence("ws1", "evidence", 10)
            .expect("ws1 search");
        assert_eq!(ws1_hits.len(), 1);
        assert!(ws1_hits[0].id.starts_with("evidence:ws1::"));

        let ws2_hits = store
            .search_evidence("ws2", "evidence", 10)
            .expect("ws2 search");
        assert_eq!(ws2_hits.len(), 1);
        assert!(ws2_hits[0].id.starts_with("evidence:ws2::"));
    }

    // ------------------------------------------------------------------
    // Graceful degradation on missing table
    // ------------------------------------------------------------------

    #[test]
    #[serial]
    fn test_list_on_missing_table() {
        let (store, _tmp) = raw_store(); // no schema applied
        let list = store
            .list_evidence("ws1", None)
            .expect("missing table must NOT error");
        assert!(list.is_empty());
    }

    #[test]
    #[serial]
    fn test_search_on_missing_table() {
        let (store, _tmp) = raw_store(); // no schema applied
        let res = store
            .search_evidence("ws1", "anything", 10)
            .expect("missing table must NOT error");
        assert!(res.is_empty());
    }
}
