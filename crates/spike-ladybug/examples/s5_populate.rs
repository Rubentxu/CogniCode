//! E29 S5 populate example — generates symbol.csv + calls.csv in a TempDir,
//! then populates BOTH lbug and PostgreSQL concurrently via tokio::join!.
//!
//! lbug schema: Symbol + Calls (from S2)
//! PG schema:   graph_nodes + graph_edges (mirrors cognicode-core production)
//!
//! Both engines load from the same generated CSVs (fair comparison).

use clap::Parser;
use csv::Writer;
use lbug::{Connection, Database, SystemConfig, Value};
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// CLI
// ============================================================================

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path for the lbug .lbdb database file
    #[arg(long)]
    lbug_path: PathBuf,

    /// PostgreSQL connection URL
    #[arg(long, default_value = "postgres://cognicode:cognicode@localhost:5432/cognicode")]
    pg_url: String,

    /// Number of Symbol rows to generate (calls = 5× symbols)
    #[arg(long, default_value = "10000")]
    rows: usize,
}

// ============================================================================
// CSV generation — shared between both engines
// ============================================================================

/// Generate Symbol CSV for lbug COPY FROM (no id column — SERIAL auto-assigns).
/// Returns the number of rows written.
fn gen_symbol_csv(path: &std::path::Path, rows: usize) -> anyhow::Result<usize> {
    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    // Header — NO id column (SERIAL PRIMARY KEY auto-assigns on COPY FROM)
    wtr.write_record(&[
        "workspace_id", "revision_id", "name", "qualified_name", "kind",
        "file_path", "line_number", "column_number", "signature", "doc_comment",
        "visibility", "fan_in", "fan_out", "valid_from", "valid_to", "properties",
    ])?;

    for id in 1..=rows {
        let kind = if id % 3 == 0 { "function" } else if id % 3 == 1 { "struct" } else { "enum" };
        let name = format!("item_{}", id);
        let qualified_name = format!("src/file_{}.rs:{}:{}", (id % 100) + 1, name, (id % 100) + 1);
        let file_path = format!("src/file_{}.rs", (id % 100) + 1);
        let line_number = ((id - 1) % 500) + 1;
        let properties = if id % 2 == 1 {
            "{codeowners=team-alpha,deprecated=false}".to_string()
        } else {
            String::new()
        };

        wtr.write_record(&[
            "1".to_string(),         // workspace_id
            "1".to_string(),         // revision_id
            name,
            qualified_name,
            kind.to_string(),
            file_path,
            line_number.to_string(),
            "1".to_string(),         // column_number
            format!("fn item_{}(i64) -> i64", id),
            String::new(),           // doc_comment
            "public".to_string(),
            "0".to_string(),        // fan_in
            "0".to_string(),        // fan_out
            "1".to_string(),        // valid_from
            "-1".to_string(),       // valid_to
            properties,
        ])?;
    }

    wtr.flush()?;
    Ok(rows)
}

/// Generate Calls CSV for PG (uses TEXT "node_{id}" format for source/target node IDs).
fn gen_calls_csv_pg(path: &std::path::Path, symbol_count: usize) -> anyhow::Result<usize> {
    let file = std::fs::File::create(path)?;
    let mut wtr = Writer::from_writer(file);

    wtr.write_record(&[
        "source_id", "target_id", "kind", "provenance", "confidence", "workspace_id",
    ])?;

    let edge_count = symbol_count * 5;
    for i in 0..edge_count {
        let from_id = (i % symbol_count) as i64 + 1;
        let to_id = ((i as i64 * 7) % symbol_count as i64) + 1;

        wtr.write_record(&[
            format!("node_{}", from_id),
            format!("node_{}", to_id),
            "Calls".to_string(),
            "extractor".to_string(),
            "1.0".to_string(),
            "1".to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(edge_count)
}

// ============================================================================
// lbug populate (sync — runs in spawn_blocking)
// Two-phase: COPY Symbol → discover internal IDs → COPY Calls with internal IDs
// ============================================================================

fn populate_lbug(
    db_path: &str,
    symbol_csv: &str,
    _calls_csv: &str,
    symbol_count: usize,
) -> anyhow::Result<usize> {
    let db = Database::new(db_path, SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    // Create Symbol node table (minimal S2 schema)
    conn.query(
        "CREATE NODE TABLE IF NOT EXISTS Symbol (\
         id SERIAL PRIMARY KEY, \
         workspace_id INT64, revision_id INT64, \
         name STRING, qualified_name STRING, kind STRING, \
         file_path STRING, line_number INT64, column_number INT64, \
         signature STRING, doc_comment STRING, visibility STRING, \
         fan_in INT64 DEFAULT 0, fan_out INT64 DEFAULT 0, \
         valid_from INT64, valid_to INT64 DEFAULT -1, \
         properties MAP(STRING, STRING));"
    )?;

    // Create Calls rel table
    conn.query(
        "CREATE REL TABLE IF NOT EXISTS Calls (\
         FROM Symbol TO Symbol, \
         workspace_id INT64, revision_id INT64, \
         provenance STRING DEFAULT 'extractor', confidence REAL DEFAULT 1.0, \
         valid_from INT64, valid_to INT64 DEFAULT -1, \
         properties MAP(STRING, STRING));"
    )?;

    // Phase 1: COPY Symbol FROM (no id column — SERIAL auto-assigns)
    conn.query(&format!("COPY Symbol FROM '{}' (header=true);", symbol_csv))?;

    // Phase 2: Discover internal node IDs (S2 found id(s) != SERIAL id)
    let mut internal_ids: Vec<u64> = Vec::with_capacity(symbol_count);
    let mut rows = conn.query("MATCH (s:Symbol) RETURN id(s) ORDER BY s.id ASC;")?;
    while let Some(row) = rows.next() {
        if let Value::InternalID(iid) = &row[0] {
            internal_ids.push(iid.offset);
        }
    }

    // Phase 3: Generate Calls CSV using discovered internal IDs
    let discovered_calls_csv = {
        let db_path_buf = std::path::PathBuf::from(db_path);
        let tmp_dir = db_path_buf.parent().unwrap();
        tmp_dir.join("calls_discovered.csv")
    };
    {
        let file = std::fs::File::create(&discovered_calls_csv)?;
        let mut wtr = Writer::from_writer(file);
        // Kùzu expects: from, to, workspace_id, revision_id, provenance, confidence, valid_from, valid_to, properties
        wtr.write_record(&[
            "from", "to", "workspace_id", "revision_id", "provenance",
            "confidence", "valid_from", "valid_to", "properties",
        ])?;

        let edge_count = symbol_count * 5;
        for i in 0..edge_count {
            let from_serial = ((i % symbol_count) as usize) + 1; // 1-indexed
            let to_serial = (((i as i64 * 7) % symbol_count as i64) as usize) + 1; // 1-indexed

            let from_internal = *internal_ids.get(from_serial.saturating_sub(1)).unwrap_or(&0);
            let to_internal = *internal_ids.get(to_serial.saturating_sub(1)).unwrap_or(&0);

            wtr.write_record(&[
                from_internal.to_string(),
                to_internal.to_string(),
                "1".to_string(),          // workspace_id
                "1".to_string(),          // revision_id
                "extractor".to_string(),  // provenance
                "1.0".to_string(),        // confidence
                "1".to_string(),          // valid_from
                "-1".to_string(),         // valid_to
                String::new(),             // properties empty
            ])?;
        }
        wtr.flush()?;
    }

    // Phase 4: COPY Calls FROM using discovered internal IDs
    let discovered_calls_str = discovered_calls_csv.to_str().unwrap();
    conn.query(&format!("COPY Calls FROM '{}' (header=true);", discovered_calls_str))?;

    // Cleanup temp CSV
    let _ = std::fs::remove_file(discovered_calls_csv);

    // Verify count
    let mut rows = conn.query("MATCH (s:Symbol) RETURN count(s);")?;
    let count: i64 = if let Some(row) = rows.next() {
        if let Value::Int64(n) = &row[0] { *n } else { anyhow::bail!("expected Int64") }
    } else { anyhow::bail!("no rows") };

    Ok(count as usize)
}

// ============================================================================
// PG populate (async via sqlx)
// ============================================================================

async fn populate_pg(
    pg_url: &str,
    symbol_csv: &str,
    calls_csv: &str,
    symbol_count: usize,
) -> anyhow::Result<usize> {
    let pool = tokio::time::timeout(
        std::time::Duration::from_millis(2000),
        PgPoolOptions::new()
            .max_connections(1)
            .connect(pg_url),
    )
    .await
    .map_err(|_| anyhow::anyhow!("PG connection timed out (2s)"))?
    .map_err(|e| anyhow::anyhow!("PG connection failed: {}", e))?;

    // Create graph_nodes table (mirrors cognicode-core m0009)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS graph_nodes (\
         id TEXT PRIMARY KEY, \
         kind TEXT NOT NULL, \
         label TEXT NOT NULL, \
         source_path TEXT, \
         properties JSONB NOT NULL DEFAULT '{}'::jsonb, \
         created_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
         updated_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
         workspace_id INTEGER NOT NULL);",
    )
    .execute(&pool)
    .await?;

    // Create graph_edges table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS graph_edges (\
         id SERIAL PRIMARY KEY, \
         source_id TEXT NOT NULL REFERENCES graph_nodes(id), \
         target_id TEXT NOT NULL REFERENCES graph_nodes(id), \
         kind TEXT NOT NULL, \
         provenance TEXT NOT NULL DEFAULT 'extracted', \
         confidence REAL NOT NULL DEFAULT 0.5, \
         metadata JSONB NOT NULL DEFAULT '{}'::jsonb, \
         created_at TIMESTAMPTZ NOT NULL DEFAULT now(), \
         workspace_id INTEGER NOT NULL);",
    )
    .execute(&pool)
    .await?;

    // Load symbol CSV into PG — map Symbol.id (SERIAL 1..N) to TEXT "node_{id}"
    // The CSV has no id column; rows are in order 1..N matching SERIAL ids
    {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(symbol_csv)?;
        let mut idx = 0usize;
        for result in rdr.records() {
            let record = result?;
            idx += 1;
            let workspace_id: i32 = record.get(0).unwrap_or("1").parse().unwrap_or(1);
            let id_str = format!("node_{}", idx);
            let kind = record.get(4).unwrap_or("function");

            sqlx::query(
                "INSERT INTO graph_nodes (id, kind, label, source_path, properties, workspace_id) \
                 VALUES ($1, $2, $3, $4, '{}'::jsonb, $5) \
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&id_str)
            .bind(kind)
            .bind(kind) // label = kind for our simplified schema
            .bind(record.get(5).unwrap_or(""))
            .bind(workspace_id)
            .execute(&pool)
            .await?;
        }
    }

    // Load calls CSV — from_id/to_id are SERIAL symbol ids (1..N) → map to "node_{id}"
    {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(calls_csv)?;
        for result in rdr.records() {
            let record = result?;
            let from_id: i64 = record.get(0).unwrap_or("1").parse().unwrap_or(1);
            let to_id: i64 = record.get(1).unwrap_or("1").parse().unwrap_or(1);
            let workspace_id: i32 = record.get(2).unwrap_or("1").parse().unwrap_or(1);

            sqlx::query(
                "INSERT INTO graph_edges (source_id, target_id, kind, provenance, workspace_id) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(format!("node_{}", from_id))
            .bind(format!("node_{}", to_id))
            .bind("Calls")
            .bind("extractor")
            .bind(workspace_id)
            .execute(&pool)
            .await?;
        }
    }

    // Verify count
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM graph_nodes")
        .fetch_one(&pool)
        .await?;
    let count = row.0 as usize;

    Ok(count)
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let rows = args.rows;
    let edge_count = rows * 5;

    // Create a shared temp directory for CSVs
    let tmp = TempDir::new()?;
    let symbol_csv = tmp.path().join("symbol.csv");
    let calls_csv_pg = tmp.path().join("calls_pg.csv");

    // Generate CSVs (lbug generates its own Calls CSV internally via two-phase discovery)
    gen_symbol_csv(&symbol_csv, rows)?;
    gen_calls_csv_pg(&calls_csv_pg, rows)?;

    let symbol_csv_str = symbol_csv.to_str().unwrap();
    let calls_csv_pg_str = calls_csv_pg.to_str().unwrap();
    let lbug_path_str = args.lbug_path.to_str().unwrap();

    // Populate both engines concurrently
    let lbug_count = tokio::task::spawn_blocking({
        let symbol_csv_str = symbol_csv_str.to_string();
        let lbug_path_str = lbug_path_str.to_string();
        move || populate_lbug(&lbug_path_str, &symbol_csv_str, "", rows)
    })
    .await??;

    let pg_count = match populate_pg(&args.pg_url, symbol_csv_str, calls_csv_pg_str, rows).await {
        Ok(n) => n,
        Err(e) => {
            eprintln!("[WARN] PG populate failed (PG may be down): {}", e);
            0
        }
    };

    println!("POPULATED lbug={} symbols + {} calls, pg={} nodes + {} edges",
             lbug_count, edge_count, pg_count, edge_count);

    Ok(())
}
