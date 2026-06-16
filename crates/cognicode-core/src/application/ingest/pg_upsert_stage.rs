//! PgUpsert stage — write extraction results to PostgreSQL
//! (ADR-017, ADR-019, ADR-020, ADR-021, ADR-023).
//!
//! Consumes from the Extract stage's mpsc receiver, batches into groups,
//! and performs per-file transactional upserts:
//!
//! For each batch (up to `BATCH_SIZE` files):
//! 1. DELETE: remove old nodes/edges for each file in the batch
//! 2. INSERT: upsert new nodes
//! 3. INSERT: upsert new edges
//! 4. UPSERT: scan_manifest row for each file
//!
//! Batching reduces COMMIT (fsync) overhead by Nx where N = BATCH_SIZE.

use std::collections::HashMap;
use std::path::Path;

use serde_json::json;
use sqlx::{Acquire, Postgres};
use tokio::sync::mpsc;

use crate::application::ingest::types::{
    ExtractionEdge, ExtractionResult, TargetRef,
};
use crate::infrastructure::persistence::{PostgresRepository, ScanManifestRow};

/// Number of files per transaction. Larger = fewer commits, more memory.
/// 10 is a good default — reduces commit overhead 10x without OOM risk.
pub const BATCH_SIZE: usize = 10;

/// Consume extraction results from the channel and upsert to PostgreSQL.
/// Returns stats and all unresolved edges for the Resolve stage.
pub async fn pg_upsert_streaming(
    repo: &PostgresRepository,
    workspace_id: &str,
    mut rx: mpsc::Receiver<ExtractionResult>,
) -> (PgUpsertStats, Vec<ExtractionEdge>) {
    let mut stats = PgUpsertStats::default();
    let mut all_unresolved: Vec<ExtractionEdge> = Vec::new();
    let mut batch: Vec<ExtractionResult> = Vec::with_capacity(BATCH_SIZE);

    while let Some(result) = rx.recv().await {
        batch.push(result);

        if batch.len() >= BATCH_SIZE {
            let (result_stats, unresolved) = upsert_batch(repo, workspace_id, &batch).await;
            stats += result_stats;
            all_unresolved.extend(unresolved);
            batch.clear();
        }
    }

    // Flush remaining
    if !batch.is_empty() {
        let (result_stats, unresolved) = upsert_batch(repo, workspace_id, &batch).await;
        stats += result_stats;
        all_unresolved.extend(unresolved);
    }

    (stats, all_unresolved)
}

/// Upsert a batch of extraction results in a single transaction.
/// Returns stats and all unresolved edges collected from this batch.
async fn upsert_batch(
    repo: &PostgresRepository,
    workspace_id: &str,
    batch: &[ExtractionResult],
) -> (PgUpsertStats, Vec<ExtractionEdge>) {
    let mut stats = PgUpsertStats::default();
    let mut all_unresolved: Vec<ExtractionEdge> = Vec::new();

    let mut conn = match repo.pool().acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("pg_upsert: failed to acquire connection: {e}");
            stats.errors += batch.len();
            return (stats, all_unresolved);
        }
    };

    let mut tx = match conn.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("pg_upsert: failed to begin tx: {e}");
            stats.errors += batch.len();
            return (stats, all_unresolved);
        }
    };

    for result in batch {
        match upsert_one(&mut tx, workspace_id, result).await {
            Ok((file_stats, unresolved)) => {
                stats.merge_file(&file_stats);
                all_unresolved.extend(unresolved);
            }
            Err(e) => {
                tracing::error!(
                    file = %result.source_path.display(),
                    "pg_upsert file failed: {e}"
                );
                stats.errors += 1;
            }
        }
    }

    if let Err(e) = tx.commit().await {
        tracing::error!("pg_upsert: commit failed: {e}");
        stats.errors += batch.len();
    }

    (stats, all_unresolved)
}

/// Upsert a single file's extraction result. Returns the stats and
/// the list of unresolved edges (those with `TargetRef::Unresolved`)
/// that will be processed by the Resolve stage (Sprint 2).
async fn upsert_one<'c>(
    tx: &mut sqlx::Transaction<'c, Postgres>,
    workspace_id: &str,
    result: &ExtractionResult,
) -> Result<(FileStats, Vec<ExtractionEdge>), sqlx::Error> {
    let rel_path = result.source_path.to_string_lossy().into_owned();

    // 1. DELETE: remove old nodes/edges for this file
    sqlx::query("DELETE FROM graph_edges WHERE workspace_id = $1 AND (source_id IN (SELECT id FROM graph_nodes WHERE workspace_id = $1 AND source_path = $2) OR target_id IN (SELECT id FROM graph_nodes WHERE workspace_id = $1 AND source_path = $2))")
        .bind(workspace_id)
        .bind(&rel_path)
        .execute(&mut **tx)
        .await?;

    sqlx::query("DELETE FROM graph_nodes WHERE workspace_id = $1 AND source_path = $2")
        .bind(workspace_id)
        .bind(&rel_path)
        .execute(&mut **tx)
        .await?;

    // 2. INSERT: new nodes
    let mut nodes_written = 0;
    for node in &result.nodes {
        let properties = serde_json::to_value(&node.properties)
            .unwrap_or_else(|_| json!({}));
        sqlx::query(
            "INSERT INTO graph_nodes \
                (id, kind, label, source_path, properties, workspace_id) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (id) DO UPDATE SET \
                kind = EXCLUDED.kind, \
                label = EXCLUDED.label, \
                source_path = EXCLUDED.source_path, \
                properties = EXCLUDED.properties, \
                updated_at = now()",
        )
        .bind(node.id.as_str())
        .bind(node.kind.as_str())
        .bind(&node.label)
        .bind(node.source_path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default())
        .bind(properties)
        .bind(workspace_id)
        .execute(&mut **tx)
        .await?;
        nodes_written += 1;
    }

    // 3. INSERT: new edges (only Resolved — Unresolved go to the Resolve stage)
    let mut edges_written = 0;
    let mut unresolved: Vec<ExtractionEdge> = Vec::new();
    for edge in &result.edges {
        let target_id = match &edge.target_ref {
            TargetRef::Resolved(id) => id.clone(),
            TargetRef::Unresolved(name) => {
                // Sprint 2: these will be resolved by the Resolve stage.
                // Collect them for post-PgUpsert processing.
                unresolved.push(edge.clone());
                continue;
            }
        };

        let metadata = json!({
            "source_file": result.source_path.to_string_lossy(),
            "line": edge.line,
        });

        sqlx::query(
            "INSERT INTO graph_edges \
                (source_id, target_id, kind, provenance, confidence, metadata, workspace_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (source_id, target_id, kind) DO UPDATE SET \
                provenance = EXCLUDED.provenance, \
                confidence = EXCLUDED.confidence, \
                metadata = EXCLUDED.metadata",
        )
        .bind(&edge.source)
        .bind(&target_id)
        .bind(&edge.kind)
        .bind(edge.provenance.to_string())
        .bind(edge.confidence)
        .bind(metadata)
        .bind(workspace_id)
        .execute(&mut **tx)
        .await?;
        edges_written += 1;
    }

    // 4. UPSERT: scan_manifest row
    let row = ScanManifestRow {
        workspace_id: workspace_id.to_string(),
        file_path: rel_path.clone(),
        file_type: file_type_str(&result.source_path),
        language: language_for(&result.source_path),
        content_hash: result.content_hash.clone(),
        mtime: 0.0, // set by caller if known
        symbol_count: nodes_written as i32,
        edge_count: edges_written as i32,
        status: if result.is_ok() { "ok" } else { "error" }.to_string(),
        error_msg: result.error.clone(),
    };
    upsert_manifest_row(tx, &row).await?;

    Ok((FileStats {
        files: 1,
        nodes: nodes_written,
        edges: edges_written,
    }, unresolved))
}

/// Upsert a single scan_manifest row (helper).
async fn upsert_manifest_row<'c>(
    tx: &mut sqlx::Transaction<'c, Postgres>,
    row: &ScanManifestRow,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO scan_manifest \
            (workspace_id, file_path, file_type, language, content_hash, \
             mtime, symbol_count, edge_count, status, error_msg) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (workspace_id, file_path) DO UPDATE SET \
            file_type = EXCLUDED.file_type, \
            language = EXCLUDED.language, \
            content_hash = EXCLUDED.content_hash, \
            mtime = EXCLUDED.mtime, \
            symbol_count = EXCLUDED.symbol_count, \
            edge_count = EXCLUDED.edge_count, \
            status = EXCLUDED.status, \
            error_msg = EXCLUDED.error_msg, \
            scanned_at = now()",
    )
    .bind(&row.workspace_id)
    .bind(&row.file_path)
    .bind(&row.file_type)
    .bind(&row.language)
    .bind(&row.content_hash)
    .bind(row.mtime)
    .bind(row.symbol_count)
    .bind(row.edge_count)
    .bind(&row.status)
    .bind(&row.error_msg)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn file_type_str(path: &Path) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "rs" | "py" | "ts" | "tsx" | "js" | "jsx" | "go" | "java" => "code".into(),
        "md" | "mdx" | "txt" | "rst" => "document".into(),
        "json" | "yaml" | "yml" | "toml" => "config".into(),
        _ => "other".into(),
    }
}

fn language_for(path: &Path) -> Option<String> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();
    match ext.as_str() {
        "rs" => Some("rust".into()),
        "py" | "pyw" => Some("python".into()),
        "ts" | "tsx" => Some("typescript".into()),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript".into()),
        "go" => Some("go".into()),
        "java" => Some("java".into()),
        _ => None,
    }
}

// ============================================================================
// Stats
// ============================================================================

#[derive(Debug, Default, Clone)]
pub struct PgUpsertStats {
    pub files: usize,
    pub nodes: usize,
    pub edges: usize,
    pub errors: usize,
}

#[derive(Debug, Default, Clone)]
struct FileStats {
    files: usize,
    nodes: usize,
    edges: usize,
}

impl PgUpsertStats {
    fn merge_file(&mut self, other: &FileStats) {
        self.files += other.files;
        self.nodes += other.nodes;
        self.edges += other.edges;
    }
}

impl std::ops::AddAssign for PgUpsertStats {
    fn add_assign(&mut self, other: Self) {
        self.files += other.files;
        self.nodes += other.nodes;
        self.edges += other.edges;
        self.errors += other.errors;
    }
}

// ============================================================================
// Tests (require a test database)
// ============================================================================

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use crate::application::ingest::types::ExtractionEdge;
    use crate::domain::aggregates::{GraphNode, NodeId};
    use crate::domain::value_objects::{NodeKind, SymbolKind};
    use crate::infrastructure::persistence::PostgresRepository;

    /// Helper to create a test pool. Mirrors the pattern in
    /// `postgres_repository::tests`.
    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgres://cognicode:cognicode@localhost:5432/cognicode_test".into()
        });
        let pool = sqlx::PgPool::connect(&url)
            .await
            .expect("connect to test db");
        let repo = PostgresRepository::from_pool(pool.clone());
        repo.run_migrations().await.expect("migrations");
        pool
    }

    #[tokio::test]
    async fn test_upsert_one_roundtrip() {
        let pool = test_pool().await;
        let repo = PostgresRepository::from_pool(pool.clone());

        // Clean up
        sqlx::query("DELETE FROM graph_nodes WHERE workspace_id = 'test_upsert'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM graph_edges WHERE workspace_id = 'test_upsert'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM scan_manifest WHERE workspace_id = 'test_upsert'")
            .execute(&pool)
            .await
            .unwrap();

        // Build a minimal ExtractionResult
        let node = GraphNode::builder(
            NodeId::new("src/test.rs:foo:1"),
            NodeKind::Symbol(SymbolKind::Function),
        )
        .label("foo")
        .source_path(std::path::PathBuf::from("src/test.rs"))
        .build();

        let edge = ExtractionEdge {
            source: "src/test.rs:foo:1".into(),
            target_ref: TargetRef::Resolved("src/test.rs:bar:5".into()),
            kind: "dependency.calls".into(),
            provenance: crate::domain::value_objects::Provenance::Extracted,
            confidence: 1.0,
            line: Some(3),
        };

        let result = ExtractionResult::ok(
            std::path::PathBuf::from("src/test.rs"),
            "deadbeef".to_string(),
            vec![node],
            vec![edge],
        );

        let (stats, _unresolved) = pg_upsert_streaming(
            &repo,
            "test_upsert",
            {
                let (tx, rx) = mpsc::channel(10);
                tx.send(result).await.unwrap();
                drop(tx);
                rx
            },
        )
        .await;

        assert_eq!(stats.files, 1);
        assert_eq!(stats.nodes, 1);
        assert_eq!(stats.errors, 0);

        // Verify the row exists
        let row: ScanManifestRow = sqlx::query_as(
            "SELECT workspace_id, file_path, file_type, language, content_hash, mtime, symbol_count, edge_count, status, error_msg FROM scan_manifest WHERE workspace_id = $1",
        )
        .bind("test_upsert")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.file_path, "src/test.rs");
        assert_eq!(row.symbol_count, 1);
    }
}
