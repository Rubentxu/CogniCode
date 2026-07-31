// DDL source of truth: sddk/e29-s2-schema-load/schema-spec.md v0.4.0

//! E29 S2 query results regression test — validates 5 query shapes.
//!
//! Each sub-test creates its own fresh tempdir DB, applies DDL, inserts
//! minimal fixtures, runs the query, and asserts the expected result.
//! Mirrors s2_query_validation example.
//!
//! NOTE: MAP subscript access s['properties']['key'] is parsed as LIST_EXTRACT
//! on nodes in Kuzu's Cypher dialect. Sub-test 3 validates the MAP was
//! stored correctly by checking its string representation contains the
//! expected key, rather than using subscript access.

use lbug::{Connection, Database, SystemConfig, Value};
use tempfile::TempDir;

// ============================================================================
// Node Tables (25)
// ============================================================================
const NODE_DDL: &[&str] = &[
    "CREATE NODE TABLE Workspace (id SERIAL PRIMARY KEY, name STRING, description STRING, created_at INT64, updated_at INT64, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Space (id SERIAL PRIMARY KEY, name STRING, description STRING, owner STRING, visibility STRING, created_at INT64, updated_at INT64, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Revision (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, parent_revision_id INT64, commit_hash STRING, message STRING, author STRING, created_at INT64, is_head BOOLEAN DEFAULT false, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE FileRecord (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, file_path STRING, content_hash STRING, language STRING, scanned_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE SourceFile (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, file_path STRING, content STRING, language STRING, line_count INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Symbol (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, name STRING, qualified_name STRING, kind STRING, file_path STRING, line_number INT64, column_number INT64, signature STRING, doc_comment STRING, visibility STRING, fan_in INT64 DEFAULT 0, fan_out INT64 DEFAULT 0, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Decision (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, adr_number STRING, title STRING, status STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Doc (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, doc_kind STRING, title STRING, content STRING, file_path STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Evidence (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, evidence_kind STRING, content STRING, source STRING, confidence REAL, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Issue (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, issue_id STRING, rule_id STRING, severity STRING, message STRING, file_path STRING, line_number INT64, column_number INT64, status STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Component (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, component_kind STRING, responsibility STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Container (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, container_kind STRING, technology STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE System (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, system_kind STRING, boundaries STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Route (id SERIAL PRIMARY KEY, symbol_id INT64, workspace_id INT64, revision_id INT64, method STRING, path STRING, handler STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Rule (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, rule_id STRING, name STRING, category STRING, severity STRING, description STRING, message_template STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Baseline (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, baseline_id STRING, name STRING, description STRING, baseline_hash STRING, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Investigation (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, title STRING, goal STRING, status STRING, entry_point STRING, narrative STRING DEFAULT '', related_adrs STRING[] DEFAULT [], created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE EvidenceItem (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, investigation_id INT64, object_id STRING, view_id STRING, note STRING, pinned_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE Artifact (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, investigation_id INT64, artifact_kind STRING, title STRING, content STRING, generated_from STRING, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE ExplorationSession (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, title STRING, panes_json STRING, navigation_json STRING, created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE NamedView (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, owner STRING, name STRING, view_kind STRING, description STRING, created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE ViewSpec (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, owner STRING, name STRING, view_kind STRING, renderer_kind STRING, data_source STRING, transform STRING, props_json STRING DEFAULT '{}', created_at INT64, updated_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE GraphReport (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, report_id STRING, report_kind STRING, summary_json STRING, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE AnalyticsRun (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, algorithm_id STRING, mode STRING, status STRING, parameters_json STRING, row_count INT64, truncation_marker STRING, error_kind STRING, error_message STRING, started_at INT64, finished_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE NODE TABLE DescriptorLimits (id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, algorithm_id STRING, version STRING, max_time_ms INT64, max_memory_bytes INT64, max_result_rows INT64, created_at INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
];

// ============================================================================
// Relationship Tables (20)
// ============================================================================
const REL_DDL: &[&str] = &[
    "CREATE REL TABLE Calls (FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, provenance STRING DEFAULT 'extractor', confidence REAL DEFAULT 1.0, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Imports (FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, module_path STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Defines (FROM FileRecord TO Symbol, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Cites (FROM Symbol TO Decision, workspace_id INT64, revision_id INT64, citation_text STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Justifies (FROM Evidence TO Decision, workspace_id INT64, revision_id INT64, justification_text STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Resolves (FROM Issue TO Decision, workspace_id INT64, revision_id INT64, resolution_note STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE PartOf (FROM Symbol TO Component, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE HttpCalls (FROM Route TO Route, workspace_id INT64, revision_id INT64, call_site STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE DefinedIn (FROM Symbol TO SourceFile, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE ScannedIn (FROM FileRecord TO Revision, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE HasIssue (FROM Symbol TO Issue, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE PinnedIn (FROM EvidenceItem TO Investigation, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE SavedAs (FROM ExplorationSession TO NamedView, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE RunsOn (FROM AnalyticsRun TO Revision, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE TrackedBy (FROM Issue TO Baseline, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Generates (FROM Investigation TO Artifact, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE BelongsTo (FROM Space TO Workspace, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Contains (FROM Container TO Component, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE References (FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, reference_kind STRING, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
    "CREATE REL TABLE Annotates (FROM Doc TO Symbol, workspace_id INT64, revision_id INT64, valid_from INT64, valid_to INT64 DEFAULT -1, properties MAP(STRING, STRING));",
];

// ============================================================================
// Sub-test 1: Point read by PK
// ============================================================================
#[test]
fn s2_point_read_returns_row() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2q.lbdb");
    let db = Database::new(db_path.to_str().unwrap(), SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    for stmt in NODE_DDL {
        conn.query(stmt).expect("NODE DDL");
    }
    for stmt in REL_DDL {
        conn.query(stmt).expect("REL DDL");
    }

    // Insert fn_1
    conn.query(
        "CREATE (:Symbol {id: 1, workspace_id: 1, revision_id: 1, \
         name: 'fn_1', qualified_name: 'main', kind: 'function', \
         file_path: 'src/main.rs', line_number: 10, column_number: 1, \
         signature: 'fn main()', doc_comment: '', visibility: 'public', \
         fan_in: 1, fan_out: 2, valid_from: 1, valid_to: -1});",
    ).expect("insert fn_1");

    let name = {
        let mut rows = conn
            .query("MATCH (s:Symbol) WHERE s.id = 1 RETURN s.name;")
            .expect("query");
        let row = rows.next().expect("one row");
        if let Value::String(s) = &row[0] {
            s.clone()
        } else {
            panic!("expected String, got {:?}", row[0])
        }
    };

    // RED: temporarily assert wrong value
    // assert_eq!(name, "wrong_name", "RED check: should fail");
    // GREEN: correct value
    assert_eq!(name, "fn_1", "point read by PK should return fn_1");
}

// ============================================================================
// Sub-test 2: Typed filter counts correctly
// ============================================================================
#[test]
fn s2_typed_filter_counts_correctly() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2q.lbdb");
    let db = Database::new(db_path.to_str().unwrap(), SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    for stmt in NODE_DDL {
        conn.query(stmt).expect("NODE DDL");
    }
    for stmt in REL_DDL {
        conn.query(stmt).expect("REL DDL");
    }

    // Insert: fn_1 (function), fn_a (function), fn_b (function), MyStruct (struct)
    for (id, name, kind) in [
        (1, "fn_1", "function"),
        (2, "fn_a", "function"),
        (3, "fn_b", "function"),
        (4, "MyStruct", "struct"),
    ] {
        conn.query(&format!(
            "CREATE (:Symbol {{id: {}, workspace_id: 1, revision_id: 1, \
             name: '{}', qualified_name: 'lib:{}', kind: '{}', \
             file_path: 'src/lib.rs', line_number: 10, column_number: 1, \
             signature: '', doc_comment: '', visibility: 'public', \
             fan_in: 0, fan_out: 0, valid_from: 1, valid_to: -1}});",
            id, name, name, kind
        )).expect("insert");
    }

    let count = {
        let mut rows = conn
            .query("MATCH (s:Symbol) WHERE s.kind = 'function' RETURN count(s);")
            .expect("query");
        let row = rows.next().expect("one row");
        if let Value::Int64(n) = &row[0] {
            *n
        } else {
            panic!("expected Int64, got {:?}", row[0])
        }
    };

    // RED: temporarily assert wrong count
    // assert_eq!(count, 99, "RED check: should fail");
    // GREEN: correct count (3 functions: fn_1, fn_a, fn_b)
    assert_eq!(count, 3, "typed filter should return 3 functions");
}

// ============================================================================
// Sub-test 3: MAP property access works
// ============================================================================
#[test]
fn s2_map_property_access_works() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2q.lbdb");
    let db = Database::new(db_path.to_str().unwrap(), SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    for stmt in NODE_DDL {
        conn.query(stmt).expect("NODE DDL");
    }
    for stmt in REL_DDL {
        conn.query(stmt).expect("REL DDL");
    }

    // Insert Symbol with MAP property using map() constructor
    conn.query(
        "CREATE (:Symbol {id: 1, workspace_id: 1, revision_id: 1, \
         name: 'with_map', qualified_name: 'src/lib.rs:with_map:1', \
         kind: 'function', file_path: 'src/lib.rs', line_number: 1, \
         column_number: 1, signature: 'fn with_map()', doc_comment: '', \
         visibility: 'public', fan_in: 0, fan_out: 0, valid_from: 1, valid_to: -1, \
         properties: map(['codeowners'], ['team-alpha'])});",
    ).expect("insert with map");

    // NOTE: subscript syntax s['properties']['key'] is parsed as LIST_EXTRACT on nodes.
    // Validate by checking the full MAP string representation instead.
    let map_repr = {
        let mut rows = conn
            .query("MATCH (s:Symbol) WHERE s.id = 1 RETURN s.properties;")
            .expect("query");
        let row = rows.next().expect("one row");
        row[0].to_string()
    };

    // RED: temporarily assert wrong value
    // assert!(map_repr.contains("wrong_key"), "RED check");
    // GREEN: verify map string contains team-alpha
    assert!(
        map_repr.contains("team-alpha"),
        "MAP should contain 'team-alpha', got: {}",
        map_repr
    );
}

// ============================================================================
// Sub-test 4: Temporal filter returns current symbols
// ============================================================================
#[test]
fn s2_temporal_filter_returns_current() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2q.lbdb");
    let db = Database::new(db_path.to_str().unwrap(), SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    for stmt in NODE_DDL {
        conn.query(stmt).expect("NODE DDL");
    }
    for stmt in REL_DDL {
        conn.query(stmt).expect("REL DDL");
    }

    // Insert: 3 with valid_to=-1 (current), 1 with valid_to=99 (superseded)
    for id in 1..=3 {
        conn.query(&format!(
            "CREATE (:Symbol {{id: {}, workspace_id: 1, revision_id: 1, \
             name: 'sym_{}', qualified_name: 'lib:sym_{}', kind: 'function', \
             file_path: 'src/lib.rs', line_number: {}, column_number: 1, \
             signature: '', doc_comment: '', visibility: 'public', \
             fan_in: 0, fan_out: 0, valid_from: 1, valid_to: -1}});",
            id, id, id, id
        )).expect("insert current symbol");
    }
    conn.query(
        "CREATE (:Symbol {id: 99, workspace_id: 1, revision_id: 1, \
         name: 'superseded', qualified_name: 'lib:superseded:1', kind: 'function', \
         file_path: 'src/lib.rs', line_number: 1, column_number: 1, \
         signature: '', doc_comment: '', visibility: 'public', \
         fan_in: 0, fan_out: 0, valid_from: 1, valid_to: 99});",
    ).expect("insert superseded");

    let current_count = {
        let mut rows = conn
            .query("MATCH (s:Symbol) WHERE s.valid_to = -1 RETURN count(s);")
            .expect("query");
        let row = rows.next().expect("one row");
        if let Value::Int64(n) = &row[0] {
            *n
        } else {
            panic!("expected Int64, got {:?}", row[0])
        }
    };

    // RED: temporarily assert wrong count
    // assert_eq!(current_count, 0, "RED check");
    // GREEN: 3 current symbols (valid_to = -1)
    assert_eq!(current_count, 3, "temporal filter valid_to=-1 should return 3");
}

// ============================================================================
// Sub-test 5: Rel traversal returns correct targets
// ============================================================================
#[test]
fn s2_rel_traversal_returns_targets() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2q.lbdb");
    let db = Database::new(db_path.to_str().unwrap(), SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    for stmt in NODE_DDL {
        conn.query(stmt).expect("NODE DDL");
    }
    for stmt in REL_DDL {
        conn.query(stmt).expect("REL DDL");
    }

    // Insert 3 Symbols: main, lib:a, lib:b
    for (id, name, qname) in [(1, "main", "main"), (2, "lib_a", "lib:a"), (3, "lib_b", "lib:b")] {
        conn.query(&format!(
            "CREATE (:Symbol {{id: {}, workspace_id: 1, revision_id: 1, \
             name: '{}', qualified_name: '{}', kind: 'function', \
             file_path: 'src/main.rs', line_number: 1, column_number: 1, \
             signature: '', doc_comment: '', visibility: 'public', \
             fan_in: 0, fan_out: 0, valid_from: 1, valid_to: -1}});",
            id, name, qname
        )).expect("insert symbol");
    }

    // Create 2 Calls edges: main -> lib:a, main -> lib:b
    conn.query(
        "MATCH (a:Symbol), (b:Symbol) \
         WHERE a.id = 1 AND b.id = 2 \
         CREATE (a)-[:Calls {workspace_id: 1, revision_id: 1, provenance: 'test', confidence: 1.0, valid_from: 1, valid_to: -1}]->(b);",
    ).expect("create calls main->a");
    conn.query(
        "MATCH (a:Symbol), (b:Symbol) \
         WHERE a.id = 1 AND b.id = 3 \
         CREATE (a)-[:Calls {workspace_id: 1, revision_id: 1, provenance: 'test', confidence: 1.0, valid_from: 1, valid_to: -1}]->(b);",
    ).expect("create calls main->b");

    // Rel traversal: main calls what?
    let targets: Vec<String> = {
        let mut rows = conn
            .query(
                "MATCH (a:Symbol)-[:Calls]->(b:Symbol) \
                 WHERE a.qualified_name = 'main' \
                 RETURN b.qualified_name ORDER BY b.qualified_name;",
            )
            .expect("query");
        let mut result = Vec::new();
        while let Some(row) = rows.next() {
            if let Value::String(s) = &row[0] {
                result.push(s.clone());
            }
        }
        result
    };

    // RED: temporarily assert wrong count
    // assert_eq!(targets.len(), 99, "RED check");
    // GREEN: exactly 2 targets (lib:a and lib:b)
    assert_eq!(targets.len(), 2, "rel traversal from main should return 2 targets");
    assert_eq!(targets[0], "lib:a");
    assert_eq!(targets[1], "lib:b");
}
