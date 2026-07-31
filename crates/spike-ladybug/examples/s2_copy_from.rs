// DDL source of truth: sddk/e29-s2-schema-load/schema-spec.md v0.4.0

//! E29 S2 copy_from example — generates symbol.csv (10K rows) + calls.csv
//! (50K rows) in a TempDir, probes Kùzu internal node-ID assignment,
//! then runs COPY FROM and measures throughput.
//!
//! Key verification:
//! - `id(s)` (Kùzu internal node ID) MUST equal the supplied SERIAL `id`
//!   value — if not, the Calls CSV `from`/`to` columns break.
//! - Symbol CSV columns: id,workspace_id,revision_id,name,qualified_name,
//!   kind,file_path,line_number,column_number,signature,doc_comment,
//!   visibility,fan_in,fan_out,valid_from,valid_to,properties
//! - Calls CSV columns: from,to,workspace_id,revision_id,provenance,
//!   confidence,valid_from,valid_to,properties

use csv::Writer;
use lbug::{Connection, Database, SystemConfig, Value};
use std::time::Instant;
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

fn gen_symbol_csv(path: &std::path::Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    // Header — NOTE: no "id" column — COPY FROM auto-populates SERIAL PRIMARY KEY
    wtr.write_record(&[
        "workspace_id", "revision_id", "name", "qualified_name", "kind",
        "file_path", "line_number", "column_number", "signature", "doc_comment",
        "visibility", "fan_in", "fan_out", "valid_from", "valid_to", "properties",
    ])?;

    // 10,000 Symbol rows — deterministic: SERIAL auto-assigns id=1..10000
    for id in 1..=10_000 {
        let kind = if id % 3 == 0 { "function" } else if id % 3 == 1 { "struct" } else { "enum" };
        let name = format!("item_{}", id);
        let qualified_name = format!("src/file_{}.rs:{}:{}", (id % 100) + 1, name, (id % 100) + 1);
        let file_path = format!("src/file_{}.rs", (id % 100) + 1);
        let line_number = ((id - 1) % 500) + 1;
        let column_number = 1;
        let signature = format!("fn {}(i64) -> i64", name);
        let visibility = "public";
        let fan_in = 0;
        let fan_out = 0;
        let valid_from = 1;
        let valid_to = -1;
        // Odd rows: non-empty MAP; even rows: empty (NULL MAP)
        let properties = if id % 2 == 1 {
            "{codeowners=team-alpha,deprecated=false}".to_string()
        } else {
            String::new()
        };

        wtr.write_record(&[
            "1".to_string(),         // workspace_id
            "1".to_string(),         // revision_id
            name.clone(),
            qualified_name.clone(),
            kind.to_string(),
            file_path.clone(),
            line_number.to_string(),
            column_number.to_string(),
            signature.clone(),
            String::new(),           // doc_comment (empty)
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

fn gen_calls_csv(path: &std::path::Path) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    // Header — first 2 columns MUST be Kùzu internal node IDs (= Symbol.id)
    wtr.write_record(&[
        "from", "to", "workspace_id", "revision_id", "provenance",
        "confidence", "valid_from", "valid_to", "properties",
    ])?;

    // 50,000 Calls edges — deterministic: from = (i % 10000) + 1, to = ((i * 7) % 10000) + 1
    for i in 0..50_000 {
        let from_id = (i % 10_000) as i64 + 1;
        let to_id = ((i as i64 * 7) % 10_000) + 1;

        wtr.write_record(&[
            from_id.to_string(),
            to_id.to_string(),
            "1".to_string(),          // workspace_id
            "1".to_string(),          // revision_id
            "extractor".to_string(),  // provenance
            "1.0".to_string(),        // confidence
            "1".to_string(),          // valid_from
            "-1".to_string(),         // valid_to
            String::new(),            // properties empty = NULL MAP
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

/// Generate Calls CSV using discovered Kùzu internal node IDs.
/// Used when the SERIAL id assumption is broken (id(s) != SERIAL id).
fn gen_calls_csv_from_internal_ids(
    path: &std::path::Path,
    symbol_internal_ids: &[u64],
) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    // Header — first 2 columns MUST be Kùzu internal node IDs
    wtr.write_record(&[
        "from", "to", "workspace_id", "revision_id", "provenance",
        "confidence", "valid_from", "valid_to", "properties",
    ])?;

    // 50,000 Calls edges — same arithmetic as gen_calls_csv but using
    // discovered internal IDs indexed by (SERIAL id - 1)
    for i in 0..50_000 {
        let from_serial = ((i % 10_000) as usize) + 1; // 1-indexed SERIAL id
        let to_serial = (((i as i64 * 7) % 10_000) as usize) + 1; // 1-indexed SERIAL id

        let from_internal = symbol_internal_ids
            .get(from_serial.saturating_sub(1))
            .copied()
            .unwrap_or(0);
        let to_internal = symbol_internal_ids
            .get(to_serial.saturating_sub(1))
            .copied()
            .unwrap_or(0);

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

fn main() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("s2.lbdb");
    let path_str = db_path.to_str().unwrap();

    let db = Database::new(path_str, SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    // Apply all DDL
    for stmt in NODE_DDL {
        conn.query(stmt)?;
    }
    for stmt in REL_DDL {
        conn.query(stmt)?;
    }

    // Generate Symbol CSV in tempdir
    let symbol_csv = tmp.path().join("symbol.csv");
    gen_symbol_csv(&symbol_csv)?;

    // -------------------------------------------------------------------------
    // COPY Symbol FROM and probe: does id(s) == SERIAL id after COPY FROM?
    // -------------------------------------------------------------------------
    let symbol_csv_str = symbol_csv.to_str().unwrap();
    conn.query(&format!("COPY Symbol FROM '{}' (header=true);", symbol_csv_str))?;

    // CRITICAL PROBE: after COPY FROM with explicit id column, does id(s) == the SERIAL id?
    // Check node with SERIAL id = 1 (first row in symbol.csv)
    let probe_result = {
        let mut rows = conn.query("MATCH (s:Symbol) WHERE s.id = 1 RETURN id(s) LIMIT 1;")?;
        let row = rows
            .next()
            .ok_or_else(|| anyhow::anyhow!("probe: no Symbol with id=1 found"))?;
        if let Value::InternalID(iid) = &row[0] {
            iid.offset
        } else {
            anyhow::bail!("expected InternalID for id(s), got {:?}", row[0]);
        }
    };

    let serial_id_assumption_holds = probe_result == 1;
    if serial_id_assumption_holds {
        println!(
            "OK: id(s) offset = {} == SERIAL id 1 — SERIAL id assumption confirmed",
            probe_result
        );
    } else {
        println!(
            "NOTE: id(s) offset = {} != SERIAL id 1. \
             SERIAL id assumption broken — using two-phase load for Calls.",
            probe_result
        );
    }

    // -------------------------------------------------------------------------
    // Two-phase Calls load (only if SERIAL assumption is broken)
    // -------------------------------------------------------------------------
    let calls_csv = tmp.path().join("calls.csv");

    if serial_id_assumption_holds {
        // Fast path: generate Calls CSV using pre-computed SERIAL ids
        gen_calls_csv(&calls_csv)?;
    } else {
        // Slow path: query all Symbol internal ids, build mapping, then generate
        let mut symbol_internal_ids: Vec<u64> = Vec::with_capacity(10_000);
        let mut rows = conn.query("MATCH (s:Symbol) RETURN id(s) ORDER BY s.id;")?;
        while let Some(row) = rows.next() {
            if let Value::InternalID(iid) = &row[0] {
                symbol_internal_ids.push(iid.offset);
            }
        }
        gen_calls_csv_from_internal_ids(&calls_csv, &symbol_internal_ids)?;
    }

    let start = Instant::now();

    let calls_csv_str = calls_csv.to_str().unwrap();
    conn.query(&format!("COPY Calls FROM '{}' (header=true);", calls_csv_str))?;

    let elapsed = start.elapsed();

    // Verify counts
    let symbol_count = {
        let mut rows = conn.query("MATCH (s:Symbol) RETURN count(s);")?;
        let row = rows.next().unwrap();
        if let Value::Int64(n) = &row[0] { *n } else { panic!("expected Int64") }
    };
    assert_eq!(symbol_count, 10_000, "expected 10,000 Symbol rows");

    let calls_count = {
        let mut rows = conn.query("MATCH ()-[r:Calls]->() RETURN count(r);")?;
        let row = rows.next().unwrap();
        if let Value::Int64(n) = &row[0] { *n } else { panic!("expected Int64") }
    };
    assert_eq!(calls_count, 50_000, "expected 50,000 Calls edges");

    let throughput = (60_000.0 / elapsed.as_secs_f64()).round();

    println!(
        "COPY FROM: {} symbols + {} calls in {:.2}s — throughput: {} rows/sec",
        symbol_count, calls_count, elapsed.as_secs_f64(), throughput
    );

    // TempDir auto-cleans .lbdb files on drop

    Ok(())
}
