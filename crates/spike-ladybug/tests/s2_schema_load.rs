// DDL source of truth: sddk/e29-s2-schema-load/schema-spec.md v0.4.0

//! E29 S2 schema-load regression test — validates all 45 DDL apply without error.
//!
//! Mirrors s2_schema_create as a regression test.

use lbug::{Connection, Database, SystemConfig, Value};
use tempfile::TempDir;

// ============================================================================
// Node Tables (25)
// ============================================================================
const NODE_DDL: &[&str] = &[
    // Workspace
    "CREATE NODE TABLE Workspace (id SERIAL PRIMARY KEY, name STRING, description STRING, created_at INT64, updated_at INT64, properties MAP(STRING, STRING));",
    // Space
    "CREATE NODE TABLE Space (id SERIAL PRIMARY KEY, name STRING, description STRING, owner STRING, visibility STRING, created_at INT64, updated_at INT64, properties MAP(STRING, STRING));",
    // Revision
    "CREATE NODE TABLE Revision (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, parent_revision_id INT64, commit_hash STRING, message STRING, author STRING, created_at INT64, is_head BOOLEAN DEFAULT false, properties MAP(STRING, STRING));",
    // FileRecord
    "CREATE NODE TABLE FileRecord (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, file_path STRING, content_hash STRING, language STRING, scanned_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // SourceFile
    "CREATE NODE TABLE SourceFile (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, file_path STRING, content STRING, language STRING, line_count INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Symbol
    "CREATE NODE TABLE Symbol (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, name STRING, qualified_name STRING, kind STRING, file_path STRING, line_number INT64, column_number INT64, signature STRING, doc_comment STRING, visibility STRING, fan_in INT64 DEFAULT 0, fan_out INT64 DEFAULT 0, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Decision
    "CREATE NODE TABLE Decision (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, adr_number STRING, title STRING, status STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Doc
    "CREATE NODE TABLE Doc (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, doc_kind STRING, title STRING, content STRING, file_path STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Evidence
    "CREATE NODE TABLE Evidence (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, evidence_kind STRING, content STRING, source STRING, confidence REAL, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Issue
    "CREATE NODE TABLE Issue (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, issue_id STRING, rule_id STRING, severity STRING, message STRING, file_path STRING, line_number INT64, column_number INT64, status STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Component
    "CREATE NODE TABLE Component (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, component_kind STRING, responsibility STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Container
    "CREATE NODE TABLE Container (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, container_kind STRING, technology STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // System
    "CREATE NODE TABLE System (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, system_kind STRING, boundaries STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Route
    "CREATE NODE TABLE Route (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, method STRING, path STRING, handler STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Rule
    "CREATE NODE TABLE Rule (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, rule_id STRING, name STRING, category STRING, severity STRING, description STRING, message_template STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Baseline
    "CREATE NODE TABLE Baseline (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, baseline_id STRING, name STRING, description STRING, baseline_hash STRING, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Investigation
    "CREATE NODE TABLE Investigation (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, title STRING, goal STRING, status STRING, entry_point STRING, narrative STRING DEFAULT '', related_adrs STRING[] DEFAULT [], created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // EvidenceItem
    "CREATE NODE TABLE EvidenceItem (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, investigation_id INT64, object_id STRING, view_id STRING, note STRING, pinned_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Artifact
    "CREATE NODE TABLE Artifact (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, investigation_id INT64, artifact_kind STRING, title STRING, content STRING, generated_from STRING, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // ExplorationSession
    "CREATE NODE TABLE ExplorationSession (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, title STRING, panes_json STRING, navigation_json STRING, created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // NamedView
    "CREATE NODE TABLE NamedView (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, owner STRING, name STRING, view_kind STRING, description STRING, created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // ViewSpec
    "CREATE NODE TABLE ViewSpec (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, owner STRING, name STRING, view_kind STRING, renderer_kind STRING, data_source STRING, transform STRING, props_json STRING DEFAULT '{}', created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // GraphReport
    "CREATE NODE TABLE GraphReport (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, report_id STRING, report_kind STRING, summary_json STRING, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // AnalyticsRun
    "CREATE NODE TABLE AnalyticsRun (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, algorithm_id STRING, mode STRING, status STRING, parameters_json STRING, row_count INT64, truncation_marker STRING, error_kind STRING, error_message STRING, started_at INT64, finished_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // DescriptorLimits
    "CREATE NODE TABLE DescriptorLimits (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, algorithm_id STRING, version STRING, max_time_ms INT64, max_memory_bytes INT64, max_result_rows INT64, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
];

// ============================================================================
// Relationship Tables (20)
// ============================================================================
const REL_DDL: &[&str] = &[
    // Calls
    "CREATE REL TABLE Calls (FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, provenance STRING DEFAULT 'extractor', confidence REAL DEFAULT 1.0, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Imports
    "CREATE REL TABLE Imports (FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, module_path STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Defines
    "CREATE REL TABLE Defines (FROM FileRecord TO Symbol, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Cites
    "CREATE REL TABLE Cites (FROM Symbol TO Decision, workspace_id INT64, revision_id INT64, citation_text STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Justifies
    "CREATE REL TABLE Justifies (FROM Evidence TO Decision, workspace_id INT64, revision_id INT64, justification_text STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Resolves
    "CREATE REL TABLE Resolves (FROM Issue TO Decision, workspace_id INT64, revision_id INT64, resolution_note STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // PartOf
    "CREATE REL TABLE PartOf (FROM Symbol TO Component, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // HttpCalls
    "CREATE REL TABLE HttpCalls (FROM Route TO Route, workspace_id INT64, revision_id INT64, call_site STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // DefinedIn
    "CREATE REL TABLE DefinedIn (FROM Symbol TO SourceFile, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // ScannedIn
    "CREATE REL TABLE ScannedIn (FROM FileRecord TO Revision, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // HasIssue
    "CREATE REL TABLE HasIssue (FROM Symbol TO Issue, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // PinnedIn
    "CREATE REL TABLE PinnedIn (FROM EvidenceItem TO Investigation, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // SavedAs
    "CREATE REL TABLE SavedAs (FROM ExplorationSession TO NamedView, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // RunsOn
    "CREATE REL TABLE RunsOn (FROM AnalyticsRun TO Revision, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // TrackedBy
    "CREATE REL TABLE TrackedBy (FROM Issue TO Baseline, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Generates
    "CREATE REL TABLE Generates (FROM Investigation TO Artifact, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // BelongsTo
    "CREATE REL TABLE BelongsTo (FROM Space TO Workspace, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Contains
    "CREATE REL TABLE Contains (FROM Container TO Component, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // References
    "CREATE REL TABLE References (FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, reference_kind STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    // Annotates
    "CREATE REL TABLE Annotates (FROM Doc TO Symbol, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
];

#[test]
fn s2_applies_all_ddl_without_error() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2.lbdb");
    let path_str = db_path.to_str().unwrap();

    let db = Database::new(path_str, SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    // Apply all NODE DDL
    for stmt in NODE_DDL {
        conn.query(stmt).expect(format!("NODE DDL should succeed: {}", stmt).as_str());
    }

    // Apply all REL DDL
    for stmt in REL_DDL {
        conn.query(stmt).expect(format!("REL DDL should succeed: {}", stmt).as_str());
    }

    // Probe: Symbol count == 0 (schema parsed, no data yet)
    let count = {
        let mut rows = conn.query("MATCH (s:Symbol) RETURN count(s);").expect("MATCH query");
        let row = rows
            .next()
            .expect("should return one row");
        if let Value::Int64(n) = &row[0] {
            *n
        } else {
            panic!("expected Int64, got {:?}", row[0]);
        }
    };

    // RED check: temporarily assert wrong value to confirm test fails
    // assert_eq!(count, 99_999, "RED check: should fail with wrong expected value");
    // GREEN: correct expected value
    assert_eq!(count, 0, "expected 0 Symbol rows after DDL, got {count}");
}
