//! E29 S5 lbug query benchmark — measures latency for Q1-Q4 on lbug.
//!
//! Queries:
//!   Q1 — Point read:        MATCH (s:Symbol) WHERE s.id = $id RETURN s.name, s.kind, s.file_path
//!   Q2 — 1-hop neighborhood: MATCH (s:Symbol) WHERE s.id = $id-[:Calls]-(n) RETURN n.name, n.kind
//!   Q3 — BFS depth 3:      MATCH (s:Symbol) WHERE s.id = $id-[:Calls*1..3]-(n) RETURN n.name, n.kind
//!   Q4 — Aggregation:       MATCH (s:Symbol) RETURN s.kind, count(*) ORDER BY count(*) DESC
//!
//! Q5 (COPY FROM) is covered by the populate step; this example focuses on query latency.

use clap::Parser;
use lbug::{Connection, Database, SystemConfig, Value};
use std::time::Instant;

const ITERATIONS_DEFAULT: usize = 100;
const WARMUP_DEFAULT: usize = 10;
const PROBE_ID: i64 = 42; // Middle of the 1..N range

// ============================================================================
// CLI
// ============================================================================

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the lbug .lbdb database file
    #[arg(long)]
    path: std::path::PathBuf,

    /// Number of iterations per query
    #[arg(long, default_value_t = ITERATIONS_DEFAULT)]
    iterations: usize,

    /// Number of warmup iterations (discarded)
    #[arg(long, default_value_t = WARMUP_DEFAULT)]
    warmup: usize,
}

// ============================================================================
// Query execution helpers
// ============================================================================

/// Q4: Aggregation — returns kind counts (already sorted by count DESC by the query)
fn run_q4(conn: &Connection) -> anyhow::Result<Vec<(String, i64)>> {
    let mut results = Vec::new();
    let mut rows = conn.query(
        "MATCH (s:Symbol) \
         WITH s.kind AS kind, count(*) AS cnt \
         RETURN kind, cnt \
         ORDER BY cnt DESC;",
    )?;
    while let Some(row) = rows.next() {
        let kind = row[0].to_string();
        let cnt = if let Value::Int64(n) = &row[1] { *n } else { 0 };
        results.push((kind, cnt));
    }
    Ok(results)
}

// ============================================================================
// Main
// ============================================================================

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let iterations = args.iterations;
    let warmup = args.warmup;
    let path_str = args.path.to_str().unwrap();

    println!("=== S5 lbug Query Latency Benchmark ===");
    println!("DB: {}", path_str);
    println!("Iterations: {} ({} warmup)", iterations, warmup);
    println!("Probe id: {}", PROBE_ID);
    println!();

    // Quick sanity: open DB and count symbols
    {
        let db = Database::new(path_str, SystemConfig::default())?;
        let conn = Connection::new(&db)?;
        let mut rows = conn.query("MATCH (s:Symbol) RETURN count(s);")?;
        let count = if let Some(row) = rows.next() {
            if let Value::Int64(n) = &row[0] { *n } else { 0 }
        } else { 0 };
        println!("DB has {} Symbol nodes", count);
    }

    // Run benchmarks
    let db_path = path_str.to_string();
    let probe_id = PROBE_ID;

    println!("\n--- Q1: Point read by id ---");
    {
        let db_path = db_path.clone();
        let mut latencies = Vec::with_capacity(iterations);

        let db = Database::new(&db_path, SystemConfig::default())?;
        let conn = Connection::new(&db)?;
        let mut stmt = conn.prepare(
            "MATCH (s:Symbol {id: $id}) RETURN s.name, s.kind, s.file_path;",
        )?;

        // Warmup
        for _ in 0..warmup {
            let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(probe_id))])?;
        }

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(probe_id))])?;
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros();
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros();
        println!("Q1 median={}us p95={}us", median, p95);
    }

    println!("\n--- Q2: 1-hop neighborhood ---");
    {
        let db_path = db_path.clone();
        let mut latencies = Vec::with_capacity(iterations);

        let db = Database::new(&db_path, SystemConfig::default())?;
        let conn = Connection::new(&db)?;
        let mut stmt = conn.prepare(
            "MATCH (s:Symbol)-[:Calls]-(n) WHERE s.id = $id RETURN n.name, n.kind;",
        )?;

        for _ in 0..warmup {
            let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(probe_id))])?;
        }

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(probe_id))])?;
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros();
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros();
        println!("Q2 median={}us p95={}us", median, p95);
    }

    println!("\n--- Q3: BFS depth 3 ---");
    {
        let db_path = db_path.clone();
        let mut latencies = Vec::with_capacity(iterations);

        let db = Database::new(&db_path, SystemConfig::default())?;
        let conn = Connection::new(&db)?;
        let mut stmt = conn.prepare(
            "MATCH (s:Symbol)-[:Calls*1..3]-(n) WHERE s.id = $id RETURN n.name, n.kind;",
        )?;

        for _ in 0..warmup {
            let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(probe_id))])?;
        }

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = conn.execute(&mut stmt, vec![("id", Value::Int64(probe_id))])?;
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros();
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros();
        println!("Q3 median={}us p95={}us", median, p95);
    }

    println!("\n--- Q4: Aggregation ---");
    {
        let db_path = db_path.clone();

        let db = Database::new(&db_path, SystemConfig::default())?;
        let conn = Connection::new(&db)?;

        let mut latencies = Vec::with_capacity(iterations);

        for _ in 0..warmup {
            let _ = run_q4(&conn)?;
        }

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = run_q4(&conn)?;
            latencies.push(start.elapsed());
        }

        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros();
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros();
        println!("Q4 median={}us p95={}us", median, p95);
    }

    println!("\n(Q5 COPY FROM latency is measured in the populate step — see s5_populate)");

    Ok(())
}
