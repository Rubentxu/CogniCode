//! E29 S5 PostgreSQL query benchmark — measures latency for Q1-Q4 on PG.
//!
//! Queries mirror the lbug Q1-Q4 from s5_query_lbug.rs:
//!   Q1 — Point read:        SELECT name, kind, file_path FROM graph_nodes WHERE id = $id
//!   Q2 — 1-hop neighborhood: recursive CTE depth ≤ 1
//!   Q3 — BFS depth 3:     recursive CTE depth ≤ 3
//!   Q4 — Aggregation:       SELECT kind, COUNT(*) FROM graph_nodes GROUP BY kind ORDER BY count(*) DESC

use clap::Parser;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::time::Instant;

const ITERATIONS_DEFAULT: usize = 100;
const WARMUP_DEFAULT: usize = 10;
const PROBE_ID: &str = "node_42"; // Text id matching lbug's node_{id} format

// ============================================================================
// CLI
// ============================================================================

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// PostgreSQL connection URL
    #[arg(long, default_value = "postgres://cognicode:cognicode@localhost:5432/cognicode")]
    pg_url: String,

    /// Number of iterations per query
    #[arg(long, default_value_t = ITERATIONS_DEFAULT)]
    iterations: usize,

    /// Number of warmup iterations (discarded)
    #[arg(long, default_value_t = WARMUP_DEFAULT)]
    warmup: usize,
}

// ============================================================================
// Query functions
// ============================================================================

/// Q1: Point read by id
async fn run_q1(pool: &PgPool, id: &str) -> anyhow::Result<usize> {
    let rows = sqlx::query("SELECT label, kind, source_path FROM graph_nodes WHERE id = $1")
        .bind(id)
        .fetch_all(pool)
        .await?;
    Ok(rows.len())
}

/// Q2: 1-hop neighborhood — bidirectional edges via two seeded CTEs
async fn run_q2(pool: &PgPool, seed_id: &str) -> anyhow::Result<usize> {
    let rows = sqlx::query(
        "WITH \
         seed AS (SELECT id FROM graph_nodes WHERE id = $1), \
         outgoing AS ( \
           SELECT nn.id, nn.kind, nn.label, 1 AS depth \
           FROM seed s \
           JOIN graph_edges e ON e.source_id = s.id \
           JOIN graph_nodes nn ON nn.id = e.target_id \
         ), \
         incoming AS ( \
           SELECT nn.id, nn.kind, nn.label, 1 AS depth \
           FROM seed s \
           JOIN graph_edges e ON e.target_id = s.id \
           JOIN graph_nodes nn ON nn.id = e.source_id \
         ) \
         SELECT DISTINCT id, kind, label FROM (SELECT * FROM outgoing UNION ALL SELECT * FROM incoming) AS combined;",
    )
    .bind(seed_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.len())
}

/// Q3: BFS depth 3 — bidirectional with depth tracking
async fn run_q3(pool: &PgPool, seed_id: &str) -> anyhow::Result<usize> {
    let rows = sqlx::query(
        "WITH RECURSIVE nbrs AS ( \
           SELECT id, kind, label, 0 AS depth FROM graph_nodes WHERE id = $1 \
           UNION ALL \
           SELECT nn.id, nn.kind, nn.label, n.depth + 1 \
           FROM nbrs n \
           JOIN graph_edges e ON e.source_id = n.id OR e.target_id = n.id \
           JOIN graph_nodes nn ON (nn.id = e.target_id AND e.source_id = n.id) OR (nn.id = e.source_id AND e.target_id = n.id) \
           WHERE n.depth < 3 AND nn.id != $1 \
         ) \
         SELECT DISTINCT id, kind, label FROM nbrs WHERE depth > 0;",
    )
    .bind(seed_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.len())
}

/// Q4: Aggregation
async fn run_q4(pool: &PgPool) -> anyhow::Result<Vec<(String, i64)>> {
    let rows = sqlx::query(
        "SELECT kind, COUNT(*) FROM graph_nodes GROUP BY kind ORDER BY count(*) DESC;",
    )
    .fetch_all(pool)
    .await?;
    let results = rows
        .iter()
        .map(|row| {
            let kind: String = row.get("kind");
            let cnt: i64 = row.get("count");
            (kind, cnt)
        })
        .collect();
    Ok(results)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let iterations = args.iterations;
    let warmup = args.warmup;

    println!("=== S5 PostgreSQL Query Latency Benchmark ===");
    println!("PG: {}", args.pg_url);
    println!("Iterations: {} ({} warmup)", iterations, warmup);
    println!("Probe id: {}", PROBE_ID);
    println!();

    // Connect to PG
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&args.pg_url)
        .await?;

    // Sanity: count nodes
    let (node_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM graph_nodes")
        .fetch_one(&pool)
        .await?;
    println!("PG has {} graph_nodes", node_count);

    // Run benchmarks
    println!("\n--- Q1: Point read by id ---");
    {
        for _ in 0..warmup { run_q1(&pool, PROBE_ID).await?; }
        let mut latencies = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            run_q1(&pool, PROBE_ID).await?;
            latencies.push(start.elapsed());
        }
        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros() as u64;
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros() as u64;
        println!("Q1 median={}us p95={}us", median, p95);
    }

    println!("\n--- Q2: 1-hop neighborhood ---");
    {
        for _ in 0..warmup { run_q2(&pool, PROBE_ID).await?; }
        let mut latencies = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            run_q2(&pool, PROBE_ID).await?;
            latencies.push(start.elapsed());
        }
        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros() as u64;
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros() as u64;
        println!("Q2 median={}us p95={}us", median, p95);
    }

    println!("\n--- Q3: BFS depth 3 ---");
    {
        for _ in 0..warmup { run_q3(&pool, PROBE_ID).await?; }
        let mut latencies = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            run_q3(&pool, PROBE_ID).await?;
            latencies.push(start.elapsed());
        }
        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros() as u64;
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros() as u64;
        println!("Q3 median={}us p95={}us", median, p95);
    }

    println!("\n--- Q4: Aggregation ---");
    {
        for _ in 0..warmup { run_q4(&pool).await?; }
        let mut latencies = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            run_q4(&pool).await?;
            latencies.push(start.elapsed());
        }
        latencies.sort();
        let median = latencies[latencies.len() / 2].as_micros() as u64;
        let p95_idx = (latencies.len() * 95) / 100;
        let p95 = latencies[p95_idx].as_micros() as u64;
        println!("Q4 median={}us p95={}us", median, p95);
    }

    println!("\n(Q5 COPY FROM latency is measured in the populate step — see s5_populate)");

    Ok(())
}
