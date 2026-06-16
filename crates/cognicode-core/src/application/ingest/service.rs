//! IngestService — orchestrator that runs the full pipeline
//! (Scan → Extract → PgUpsert → Refresh) for a workspace.
//!
//! For Sprint 1, this is a synchronous function. Sprint 2 adds the
//! Resolve/Cluster/Analyze/Report stages. Sprint 3 (S1-10) wraps it
//! in an async job for the Explorer API.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use crate::application::ingest::analyzer::run_analyze;
use crate::application::ingest::cluster::run_cluster;
use crate::application::ingest::extract_stage::extract_streaming;
use crate::application::ingest::pg_upsert_stage::pg_upsert_streaming;
use crate::application::ingest::refresh::refresh_from_pg;
use crate::application::ingest::report_stage::run_report;
use crate::application::ingest::resolve::resolve_cross_file_calls;
use crate::application::ingest::scan::{scan_for_changes, ScanEntry};
use crate::application::ingest::types::{
    ChangeKind, FailedFile, ScanProgress, ScanResult, ScanStage,
};
use crate::infrastructure::graph::graph_cache::GraphCache;
use crate::infrastructure::persistence::PostgresRepository;

/// Run the full ingest pipeline for a workspace root.
///
/// Stages (Sprint 1):
/// 1. **Scan** — walk FS, detect Changed|New|Deleted files (ADR-017)
/// 2. **Extract** — tree-sitter parse, produce `ExtractionResult` (ADR-018)
/// 3. **PgUpsert** — write to PG in batches (ADR-017/021)
/// 4. **Refresh** — reload `GraphCache` from PG (ADR-017)
///
/// Returns a `ScanResult` with timing, counts, and any failed files.
pub async fn run_scan(
    repo: &PostgresRepository,
    cache: &GraphCache,
    workspace_id: &str,
    root: &Path,
    on_progress: Option<&(dyn Fn(ScanProgress) + Send + Sync)>,
) -> ScanResult {
    let start = Instant::now();
    let total = count_source_files(root);
    let mut failed_files: Vec<FailedFile> = Vec::new();

    // ── Stage 1: Scan ──────────────────────────────────────────────
    report_progress(on_progress, ScanStage::Scan, 0, total, 0);
    let previous = load_previous_manifest(repo, workspace_id).await;
    let changes = scan_for_changes(root, &previous);
    let scan_done = changes
        .iter()
        .filter(|c| c.kind != ChangeKind::Deleted)
        .count();
    report_progress(on_progress, ScanStage::Scan, scan_done, total, 0);

    // ── Stage 2: Extract (streaming) ───────────────────────────────
    let to_extract: Vec<_> = changes
        .into_iter()
        .filter(|c| c.kind != ChangeKind::Deleted)
        .collect();
    let extract_count = to_extract.len();

    report_progress(on_progress, ScanStage::Extract, 0, extract_count, 0);
    let mut rx = extract_streaming(to_extract);
    let mut results: Vec<_> = Vec::new();
    let mut received = 0;
    while let Some(result) = rx.recv().await {
        received += 1;
        if let Some(err) = &result.error {
            failed_files.push(FailedFile {
                path: result.source_path.to_string_lossy().into_owned(),
                error: err.clone(),
            });
        }
        results.push(result);
        report_progress(
            on_progress,
            ScanStage::Extract,
            received,
            extract_count,
            failed_files.len(),
        );
    }

    // ── Stage 3: PgUpsert (streaming) ─────────────────────────────
    report_progress(on_progress, ScanStage::PgUpsert, 0, results.len(), failed_files.len());
    let (upsert_stats, unresolved_edges) = pg_upsert_streaming(repo, workspace_id, {
        let (tx, rx) = tokio::sync::mpsc::channel(crate::application::ingest::pg_upsert_stage::BATCH_SIZE);
        for r in results {
            let _ = tx.send(r).await;
        }
        drop(tx);
        rx
    })
    .await;
    report_progress(
        on_progress,
        ScanStage::PgUpsert,
        upsert_stats.files,
        extract_count,
        failed_files.len() + upsert_stats.errors,
    );

    // ── Stage 3b: Resolve (cross-file calls) ──────────────────────
    if !unresolved_edges.is_empty() {
        report_progress(on_progress, ScanStage::Resolve, 0, unresolved_edges.len(), 0);
        let resolved = resolve_cross_file_calls(repo, workspace_id, &unresolved_edges).await;
        report_progress(on_progress, ScanStage::Resolve, resolved, unresolved_edges.len(), 0);
    }

    // ── Stage 5: Cluster (community detection) ──────────────────
    report_progress(on_progress, ScanStage::Cluster, 0, 1, 0);
    let communities = run_cluster(repo, cache, workspace_id).await;
    report_progress(on_progress, ScanStage::Cluster, communities, 1, 0);

    // ── Stage 6: Analyze (god nodes, dead code, hot paths) ──────
    report_progress(on_progress, ScanStage::Analyze, 0, 1, 0);
    let summary = run_analyze(cache).await;
    report_progress(on_progress, ScanStage::Analyze, 1, 1, 0);

    // ── Stage 7: Report (persist to graph_reports) ──────────────
    report_progress(on_progress, ScanStage::Report, 0, 1, 0);
    let _report_id = run_report(repo, workspace_id, &summary).await;
    report_progress(on_progress, ScanStage::Report, 1, 1, 0);

    // Delete scan_manifest entries for files that were deleted
    let keep_paths: Vec<String> = previous.keys().cloned().collect();
    if let Err(e) = repo.delete_scan_manifest_except(workspace_id, &keep_paths).await {
        tracing::warn!("scan_manifest cleanup failed: {e}");
    }

    // ── Stage 4: Refresh ──────────────────────────────────────────
    report_progress(on_progress, ScanStage::Refresh, 0, 1, 0);
    if let Err(e) = refresh_from_pg(repo, cache).await {
        tracing::error!("refresh failed: {e}");
        failed_files.push(FailedFile {
            path: "<refresh>".to_string(),
            error: e.to_string(),
        });
    }
    report_progress(on_progress, ScanStage::Done, 1, 1, failed_files.len());

    let total_nodes = upsert_stats.nodes;
    let total_edges = upsert_stats.edges;
    ScanResult {
        symbols: total_nodes,
        edges: total_edges,
        duration_ms: start.elapsed().as_millis() as u64,
        failed_files,
        community_count: communities,
        health_score: summary.health_score,
    }
}

/// Count source files in the workspace root (for progress reporting).
fn count_source_files(root: &Path) -> usize {
    crate::application::ingest::scan::walk_files(root).len()
}

/// Load the previous scan manifest from PG, converting to the
/// lightweight `ScanEntry` map used by the Scan stage.
async fn load_previous_manifest(
    repo: &PostgresRepository,
    workspace_id: &str,
) -> HashMap<String, ScanEntry> {
    match repo.load_scan_manifest(workspace_id).await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| {
                let entry = ScanEntry {
                    content_hash: r.content_hash,
                    mtime: r.mtime,
                };
                (r.file_path, entry)
            })
            .collect(),
        Err(e) => {
            tracing::warn!("load_scan_manifest failed (treating as empty): {e}");
            HashMap::new()
        }
    }
}

/// Report progress to the optional callback.
fn report_progress(
    callback: Option<&(dyn Fn(ScanProgress) + Send + Sync)>,
    stage: ScanStage,
    processed: usize,
    total: usize,
    failed: usize,
) {
    if let Some(cb) = callback {
        cb(ScanProgress {
            stage,
            processed,
            total,
            failed,
        });
    }
}

#[cfg(all(test, feature = "postgres"))]
mod tests {
    // Integration tests require a live PG database with TEST_DATABASE_URL.
    // Run with: TEST_DATABASE_URL=postgres://... cargo test
    // The unit-level tests for individual stages are in their own modules.
}
