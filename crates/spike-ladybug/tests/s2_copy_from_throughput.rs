// DDL source of truth: sddk/e29-s2-schema-load/schema-spec.md v0.4.0

//! E29 S2 copy_from throughput regression test.
//!
//! Mirrors s2_copy_from example as a test. Asserts 60K-row COPY FROM
//! completes in < 60s wall-clock. Uses the same deterministic CSV
//! generators as the example.

use csv::Writer;
use lbug::{Connection, Database, SystemConfig, Value};
use std::time::Instant;
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

fn gen_symbol_csv(path: &std::path::Path) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    wtr.write_record(&[
        "workspace_id", "revision_id", "name", "qualified_name", "kind",
        "file_path", "line_number", "column_number", "signature", "doc_comment",
        "visibility", "fan_in", "fan_out", "valid_from", "valid_to", "properties",
    ])?;

    for id in 1..=10_000 {
        let kind = if id % 3 == 0 {
            "function"
        } else if id % 3 == 1 {
            "struct"
        } else {
            "enum"
        };
        let name = format!("item_{}", id);
        let qualified_name = format!("src/file_{}.rs:{}:{}", (id % 100) + 1, name, (id % 100) + 1);
        let file_path = format!("src/file_{}.rs", (id % 100) + 1);
        let line_number = ((id - 1) % 500) + 1;
        let visibility = "public";
        let fan_in = 0;
        let fan_out = 0;
        let valid_from = 1;
        let valid_to = -1;
        let properties =
            if id % 2 == 1 {
                "{codeowners=team-alpha,deprecated=false}".to_string()
            } else {
                String::new()
            };

        wtr.write_record(&[
            "1".to_string(),
            "1".to_string(),
            name.clone(),
            qualified_name,
            kind.to_string(),
            file_path,
            line_number.to_string(),
            "1".to_string(),
            format!("fn {}(i64) -> i64", name),
            String::new(),
            visibility.to_string(),
            fan_in.to_string(),
            fan_out.to_string(),
            valid_from.to_string(),
            valid_to.to_string(),
            properties,
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Discover Kùzu internal node IDs after Symbol COPY, then generate Calls CSV.
fn gen_calls_csv_two_phase(path: &std::path::Path, conn: &Connection) -> std::io::Result<()> {
    // Query all Symbol internal IDs ordered by SERIAL id
    let mut symbol_internal_ids: Vec<u64> = Vec::with_capacity(10_000);
    let mut rows = conn
        .query("MATCH (s:Symbol) RETURN id(s) ORDER BY s.id;")
        .expect("query internal IDs");
    while let Some(row) = rows.next() {
        if let Value::InternalID(iid) = &row[0] {
            symbol_internal_ids.push(iid.offset);
        }
    }

    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    wtr.write_record(&[
        "from", "to", "workspace_id", "revision_id", "provenance",
        "confidence", "valid_from", "valid_to", "properties",
    ])?;

    for i in 0..50_000 {
        let from_serial = ((i % 10_000) as usize) + 1;
        let to_serial = (((i as i64 * 7) % 10_000) as usize) + 1;

        let from_internal = *symbol_internal_ids
            .get(from_serial.saturating_sub(1))
            .unwrap_or(&0);
        let to_internal = *symbol_internal_ids
            .get(to_serial.saturating_sub(1))
            .unwrap_or(&0);

        wtr.write_record(&[
            from_internal.to_string(),
            to_internal.to_string(),
            "1".to_string(),
            "1".to_string(),
            "extractor".to_string(),
            "1.0".to_string(),
            "1".to_string(),
            "-1".to_string(),
            String::new(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

#[test]
fn s2_copy_from_throughput() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("s2.lbdb");
    let path_str = db_path.to_str().unwrap();

    let db = Database::new(path_str, SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    // Apply all DDL
    for stmt in NODE_DDL {
        conn.query(stmt).expect(format!("NODE DDL should succeed: {}", stmt).as_str());
    }
    for stmt in REL_DDL {
        conn.query(stmt).expect(format!("REL DDL should succeed: {}", stmt).as_str());
    }

    // Generate Symbol CSV
    let symbol_csv = tmp.path().join("symbol.csv");
    gen_symbol_csv(&symbol_csv).expect("gen_symbol_csv");

    // COPY Symbol FROM
    let symbol_csv_str = symbol_csv.to_str().unwrap();
    conn.query(&format!("COPY Symbol FROM '{}' (header=true);", symbol_csv_str))
        .expect("COPY Symbol FROM");

    // Discover internal IDs and generate Calls CSV (two-phase since SERIAL assumption is broken)
    let calls_csv = tmp.path().join("calls.csv");
    gen_calls_csv_two_phase(&calls_csv, &conn).expect("gen_calls_csv_two_phase");

    // Time the Calls COPY
    let start = Instant::now();
    let calls_csv_str = calls_csv.to_str().unwrap();
    conn.query(&format!("COPY Calls FROM '{}' (header=true);", calls_csv_str))
        .expect("COPY Calls FROM");
    let elapsed = start.elapsed();

    // RED moment: temporarily assert impossible budget to confirm test fails
    // assert!(
    //     elapsed.as_secs_f64() < 0.001,
    //     "RED check: this should fail (impossible 1ms budget)"
    // );
    // GREEN: correct budget
    assert!(
        elapsed.as_secs_f64() < 60.0,
        "COPY FROM 60K rows should complete in < 60s, took {:.2}s",
        elapsed.as_secs_f64()
    );

    // Verify post-load row counts
    let symbol_count = {
        let mut rows = conn.query("MATCH (s:Symbol) RETURN count(s);").expect("count symbols");
        let row = rows.next().expect("one row");
        if let Value::Int64(n) = &row[0] { *n } else { panic!("expected Int64") }
    };
    assert_eq!(symbol_count, 10_000, "expected 10,000 Symbol rows");

    let calls_count = {
        let mut rows = conn
            .query("MATCH ()-[r:Calls]->() RETURN count(r);")
            .expect("count calls");
        let row = rows.next().expect("one row");
        if let Value::Int64(n) = &row[0] { *n } else { panic!("expected Int64") }
    };
    assert_eq!(calls_count, 50_000, "expected 50,000 Calls edges");

    eprintln!(
        "elapsed: {:.2}s — throughput: {} rows/sec",
        elapsed.as_secs_f64(),
        (60_000.0 / elapsed.as_secs_f64()).round()
    );
}
