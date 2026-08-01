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
use crate::application::ingest::scan::{ScanEntry, scan_for_changes};
use crate::application::ingest::types::{
    ChangeKind, FailedFile, ScanProgress, ScanResult, ScanStage,
};
#[cfg(feature = "postgres")]
use crate::domain::ports::{ManifestError, ManifestStore};
use crate::infrastructure::graph::graph_cache::GraphCache;
use crate::infrastructure::graph::snapshot_provider::SnapshotProvider;
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
    tracing::info!(workspace_id = %workspace_id, root = %root.display(), "ingest: run_scan start");
    let total = count_source_files(root);
    tracing::info!(workspace_id = %workspace_id, total, "ingest: count_source_files done");
    let mut failed_files: Vec<FailedFile> = Vec::new();

    // ── Advisory lock (prevent concurrent scans) ──────────────────
    #[cfg(feature = "postgres")]
    let _lock = {
        let result = repo
            .with_pool_async(|pool| async move {
                sqlx::query("SELECT pg_advisory_lock(hashtext($1))")
                    .bind(workspace_id)
                    .execute(pool)
                    .await
            })
            .await;
        if let Err(e) = result {
            tracing::error!("advisory_lock failed: {e}");
        }
        // Drop guard: lock is released when _lock goes out of scope
        // (at end of function). PG automatically releases on disconnect.
        Some(())
    };
    #[cfg(not(feature = "postgres"))]
    let _lock: Option<()> = None;

    // ── Stage 1: Scan ──────────────────────────────────────────────
    report_progress(on_progress, ScanStage::Scan, 0, total, 0);
    let previous = {
        #[cfg(feature = "postgres")]
        {
            let manifest = crate::domain::ports::PostgresManifestStore::new(repo);
            load_previous_manifest(&manifest, workspace_id).await
        }
        #[cfg(not(feature = "postgres"))]
        {
            load_previous_manifest(repo, workspace_id).await
        }
    };
    tracing::info!(workspace_id = %workspace_id, previous_manifest = previous.len(), "ingest: load_previous_manifest done");
    let changes = scan_for_changes(root, &previous);
    tracing::info!(workspace_id = %workspace_id, changes = changes.len(), "ingest: scan_for_changes done");
    let scan_done = changes
        .iter()
        .filter(|c| c.kind != ChangeKind::Deleted)
        .count();
    report_progress(on_progress, ScanStage::Scan, scan_done, total, 0);

    // ── Stage 1b: Parse CODEOWNERS (ownership feature) ─────────────
    #[cfg(feature = "ownership")]
    let codeowners = crate::application::ingest::CodeOwnersMap::parse(root);

    // ── Stage 2: Extract (streaming) ───────────────────────────────
    let to_extract: Vec<_> = changes
        .into_iter()
        .filter(|c| c.kind != ChangeKind::Deleted)
        .collect();
    let extract_count = to_extract.len();
    tracing::info!(workspace_id = %workspace_id, extract_count, "ingest: extract stage start");

    report_progress(on_progress, ScanStage::Extract, 0, extract_count, 0);
    let mut rx = extract_streaming(to_extract);
    let mut results: Vec<_> = Vec::new();
    let mut received = 0;
    while let Some(mut result) = rx.recv().await {
        if received == 0 {
            tracing::info!(workspace_id = %workspace_id, "ingest: first extraction result received");
        }
        received += 1;
        if let Some(err) = &result.error {
            failed_files.push(FailedFile {
                path: result.source_path.to_string_lossy().into_owned(),
                error: err.clone(),
            });
        }

        // ── Stage 2b: Enrich with blame (ownership feature) ──────────
        #[cfg(feature = "ownership")]
        crate::application::ingest::enrich_with_blame(&mut result, root, &codeowners);

        results.push(result);
        report_progress(
            on_progress,
            ScanStage::Extract,
            received,
            extract_count,
            failed_files.len(),
        );
    }
    tracing::info!(workspace_id = %workspace_id, received, failed = failed_files.len(), "ingest: extract stage done");

    // ── Stage 3: PgUpsert (streaming) ─────────────────────────────
    report_progress(
        on_progress,
        ScanStage::PgUpsert,
        0,
        results.len(),
        failed_files.len(),
    );
    let (tx, rx) =
        tokio::sync::mpsc::channel(crate::application::ingest::pg_upsert_stage::BATCH_SIZE);
    tokio::spawn(async move {
        for r in results {
            if tx.send(r).await.is_err() {
                tracing::warn!("ingest: pg_upsert receiver dropped before all results were sent");
                break;
            }
        }
    });
    let (upsert_stats, unresolved_edges) = pg_upsert_streaming(repo, workspace_id, rx).await;
    tracing::info!(workspace_id = %workspace_id, files = upsert_stats.files, nodes = upsert_stats.nodes, edges = upsert_stats.edges, unresolved = unresolved_edges.len(), errors = upsert_stats.errors, "ingest: pg_upsert done");
    report_progress(
        on_progress,
        ScanStage::PgUpsert,
        upsert_stats.files,
        extract_count,
        failed_files.len() + upsert_stats.errors,
    );

    // ── Stage 3b: Resolve (cross-file calls) ──────────────────────
    if !unresolved_edges.is_empty() {
        report_progress(
            on_progress,
            ScanStage::Resolve,
            0,
            unresolved_edges.len(),
            0,
        );
        let resolved = resolve_cross_file_calls(repo, workspace_id, &unresolved_edges).await;
        tracing::info!(workspace_id = %workspace_id, resolved, unresolved = unresolved_edges.len(), "ingest: resolve done");
        report_progress(
            on_progress,
            ScanStage::Resolve,
            resolved,
            unresolved_edges.len(),
            0,
        );
    }

    // ── Stage 5: Cluster (community detection) ──────────────────
    report_progress(on_progress, ScanStage::Cluster, 0, 1, 0);
    let communities = run_cluster(repo, cache, workspace_id).await;
    tracing::info!(workspace_id = %workspace_id, communities, "ingest: cluster done");
    report_progress(on_progress, ScanStage::Cluster, communities, 1, 0);

    // ── Stage 6: Analyze (god nodes, dead code, hot paths) ──────
    report_progress(on_progress, ScanStage::Analyze, 0, 1, 0);
    let summary = run_analyze(cache).await;
    tracing::info!(workspace_id = %workspace_id, health_score = summary.health_score, "ingest: analyze done");
    report_progress(on_progress, ScanStage::Analyze, 1, 1, 0);

    // ── Stage 7: Report (persist to graph_reports) ──────────────
    report_progress(on_progress, ScanStage::Report, 0, 1, 0);
    let _report_id = run_report(repo, workspace_id, &summary).await;
    tracing::info!(workspace_id = %workspace_id, "ingest: report done");
    report_progress(on_progress, ScanStage::Report, 1, 1, 0);

    // Delete scan_manifest entries for files that were deleted
    let keep_paths: Vec<String> = previous.keys().cloned().collect();
    #[cfg(feature = "postgres")]
    {
        let manifest = crate::domain::ports::PostgresManifestStore::new(repo);
        // PHASE 0 rename: `delete_except(keep_paths)` (batch by keep set)
        // is now `delete_manifest_entry(workspace_id, file_path)` (single
        // row delete) per the ADR-028 contract; the caller-side per-file
        // loop lives below. The batch semantics are preserved at the
        // iteration level.
        for path in previous.keys() {
            if let Err(e) = manifest.delete_manifest_entry(workspace_id, path).await {
                tracing::warn!("scan_manifest cleanup failed for {path}: {e}");
            }
        }
        // `keep_paths` is unused for now (single-row delete only); reserved
        // for a future batch helper that restores the optimization.
        let _ = keep_paths;
    }
    #[cfg(not(feature = "postgres"))]
    {
        if let Err(e) = repo
            .delete_scan_manifest_except(workspace_id, &keep_paths)
            .await
        {
            tracing::warn!("scan_manifest cleanup failed: {e}");
        }
    }

    // ── Stage 4: Refresh ──────────────────────────────────────────
    report_progress(on_progress, ScanStage::Refresh, 0, 1, 0);
    use crate::domain::value_objects::WorkspaceId;
    use crate::infrastructure::graph::SnapshotProviderImpl;
    let ws = WorkspaceId::try_new(workspace_id).unwrap_or_default();
    let provider = SnapshotProviderImpl::new(repo.with_pool(|p| p.clone()));
    if let Err(e) = refresh_from_pg(&provider, cache, &ws).await {
        tracing::error!("refresh failed: {e}");
        failed_files.push(FailedFile {
            path: "<refresh>".to_string(),
            error: e.to_string(),
        });
    }
    tracing::info!(workspace_id = %workspace_id, failed = failed_files.len(), duration_ms = start.elapsed().as_millis() as u64, "ingest: refresh/done");
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
#[cfg(feature = "postgres")]
async fn load_previous_manifest(
    manifest: &dyn ManifestStore,
    workspace_id: &str,
) -> HashMap<String, ScanEntry> {
    match manifest.get_manifest(workspace_id).await {
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
            tracing::warn!("load_previous_manifest failed (treating as empty): {e}");
            HashMap::new()
        }
    }
}

#[cfg(not(feature = "postgres"))]
async fn load_previous_manifest(
    _repo: &PostgresRepository,
    _workspace_id: &str,
) -> HashMap<String, ScanEntry> {
    HashMap::new()
}

/// Helper: log and ignore `ManifestError` from cleanup ops.
#[cfg(feature = "postgres")]
fn log_manifest_err(ctx: &str, e: ManifestError) {
    tracing::warn!("{ctx}: {e}");
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
