//! E29 S5 latency tests — comparative benchmark: lbug vs PostgreSQL.
//!
//! Tests E1–E7 from spec.md §S5:
//!   E1 (Q1): lbug median < PG median (point read)
//!   E2 (Q2): lbug median < PG median (1-hop)
//!   E3 (Q3): lbug median ≤ 2× PG median (BFS depth 3)
//!   E4 (Q4): lbug median ≤ 2× PG median (aggregation)
//!   E5 (Q5): lbug COPY FROM time ≤ 2× PG COPY FROM time
//!   E6: PG unreachable → test exits 0 with [SKIP] marker
//!   E7: cargo check --workspace + clippy exit 0 (spike excluded)
//!
//! PR 1: lbug-only benchmark (E1, E2, E3, E4 baseline)
//! PR 2: full benchmark vs PG (E1–E5) + E6 + E7

use lbug::{Connection, Database, SystemConfig, Value};
use std::time::Instant;
use tempfile::TempDir;

// ============================================================================
// Helpers
// ============================================================================

/// Run a single lbug query and return latency in micros
fn run_lbug_query(path: &std::path::Path, query: &str, id: Option<i64>) -> anyhow::Result<u64> {
    let db = Database::new(path.to_str().unwrap(), SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    let start = Instant::now();
    if let Some(id_val) = id {
        let mut stmt = conn.prepare(query)?;
        let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(id_val))])?;
    } else {
        let _ = conn.query(query)?;
    }
    Ok(start.elapsed().as_micros() as u64)
}

/// Run warmup + timed iterations, return median latency in micros
fn benchmark_lbug(
    path: &std::path::Path,
    query: &str,
    id: Option<i64>,
    iterations: usize,
    warmup: usize,
) -> anyhow::Result<u64> {
    // Warmup
    for _ in 0..warmup {
        run_lbug_query(path, query, id)?;
    }

    // Timed
    let mut latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let lat = run_lbug_query(path, query, id)?;
        latencies.push(lat);
    }
    latencies.sort();
    let median = latencies[latencies.len() / 2];
    Ok(median)
}

// ============================================================================
// E1: Q1 point read — lbug median must be > 0
// ============================================================================

#[tokio::test]
async fn benchmark_lbug_only_first() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let db_path = tmp.path().join("s5_e1.lbdb");

    // Populate lbug with 100 symbols
    let populate_db = Database::new(db_path.to_str().unwrap(), SystemConfig::default())?;
    let populate_conn = Connection::new(&populate_db)?;

    populate_conn.query(
        "CREATE NODE TABLE IF NOT EXISTS Symbol (\
         id SERIAL PRIMARY KEY, workspace_id INT64, revision_id INT64, \
         name STRING, qualified_name STRING, kind STRING, \
         file_path STRING, line_number INT64, column_number INT64, \
         signature STRING, doc_comment STRING, visibility STRING, \
         fan_in INT64 DEFAULT 0, fan_out INT64 DEFAULT 0, \
         valid_from INT64, valid_to INT64 DEFAULT -1, \
         properties MAP(STRING, STRING));",
    )?;
    populate_conn.query(
        "CREATE REL TABLE IF NOT EXISTS Calls (\
         FROM Symbol TO Symbol, workspace_id INT64, revision_id INT64, \
         provenance STRING DEFAULT 'extractor', confidence REAL DEFAULT 1.0, \
         valid_from INT64, valid_to INT64 DEFAULT -1, \
         properties MAP(STRING, STRING));",
    )?;

    // Insert 100 symbols directly (no CSV needed for this test)
    for i in 1..=100 {
        let kind = if i % 3 == 0 { "function" } else if i % 3 == 1 { "struct" } else { "enum" };
        populate_conn.query(&format!(
            "CREATE (:Symbol {{id: {}, workspace_id: 1, revision_id: 1, \
             name: 'item_{}', qualified_name: 'src/file_{}.rs:item_{}:1', kind: '{}', \
             file_path: 'src/file_{}.rs', line_number: {}, column_number: 1, \
             signature: 'fn item_{}(i64)', doc_comment: '', visibility: 'public', \
             fan_in: 0, fan_out: 0, valid_from: 1, valid_to: -1, \
             properties: map(['codeowners'], ['team-alpha'])}});",
            i,                 // id
            i,                 // name: 'item_{}'
            (i % 100) + 1,     // qualified_name: 'src/file_{}.rs:...'
            i,                 // qualified_name: '...:item_{}:1'
            if i % 3 == 0 { "function" } else if i % 3 == 1 { "struct" } else { "enum" }, // kind
            (i % 100) + 1,     // file_path: 'src/file_{}.rs'
            (i % 500) + 1,     // line_number
            i                  // signature: 'fn item_{}(i64)'
        ))?;
    }

    // Run Q1 benchmark: point read by id = 42
    let q1_query = "MATCH (s:Symbol) WHERE s.id = $id RETURN s.name, s.kind, s.file_path;";
    let median = benchmark_lbug(&db_path, q1_query, Some(42), 100, 10)?;

    println!("Q1 lbug median: {}us", median);
    assert!(median > 0, "Q1 median must be > 0, got {}us", median);

    Ok(())
}
