//! The incremental reindex, and what it costs.
//!
//! `reparse_on_edit` does five things, and this module measures each of
//! them where it happens: it reads the state the operation needs
//! (`scan`), it establishes the identity of every candidate file against
//! the manifest (`hash`), it interprets the files whose content changed
//! (`parse`), it writes them into the index (`index`) and it rebuilds
//! the graph (`graph`).
//!
//! The cost travels with the operation, and it says which kind of
//! rebuild it was: a cache hit (nothing was interpreted again), a full
//! rebuild (everything was), a partial one (some was, the rest was
//! reused) or an error (the rebuild did not finish). The word is
//! decided by the product's own numbers, never by a label written next
//! to the call.
//!
//! Nothing here publishes a path or a content: the cost is a fact about
//! the work, not about the tree it was done on.

use std::path::PathBuf;
use std::time::Instant;

use super::*;
use crate::application::graph_snapshot::content_matches_manifest;
use crate::application::reparse_cost::{PhaseCost, ReparseCost, ReparsePhase, rebuild_kind_of};
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::domain::value_objects::file_time::observed_mtime;
use crate::domain::value_objects::symbol_delta::{SymbolCount, symbol_delta};
use crate::infrastructure::process_memory::read_peak_memory_bytes;
use crate::infrastructure::telemetry::get_global_metrics;
use crate::interface::mcp::schemas::ReparseOnEditInput;

/// What the operation will do with one candidate file.
enum Decision {
    /// The file is gone: the symbols it had are the ones the graph
    /// holds of it.
    Removed { full: PathBuf, rel: PathBuf },
    /// The file is the one the manifest recorded.
    Unchanged,
    /// The file has to be read again.
    Changed { full: PathBuf, rel: PathBuf },
}

/// What one incremental reindex did, and what it cost.
pub(crate) struct ReparseOutcome {
    /// Number of files that were parsed
    pub files_parsed: usize,
    /// Number of files that were skipped (unchanged)
    pub files_skipped: usize,
    /// Number of files that were removed from the graph
    pub files_removed: usize,
    /// Symbols the changed files gained, counted in the graph
    pub symbols_added: usize,
    /// Symbols the changed files lost, counted in the graph
    pub symbols_removed: usize,
    /// Files whose symbol change could not be counted.
    pub symbols_unmeasured: usize,
    /// Whether the graph was actually updated
    pub graph_updated: bool,
    /// What the operation cost
    pub cost: ReparseCost,
    /// The time the operation took before the rebuild, in milliseconds.
    ///
    /// The rebuild is not part of it: this is the time the analysis
    /// took, and the rebuild is a fact about the graph, which the cost
    /// publishes on its own.
    pub analysis_time_ms: u64,
}

/// Runs one incremental reindex over the given files, and says what it
/// did and what it cost.
pub(crate) async fn run(
    ctx: &HandlerContext,
    input: &ReparseOnEditInput,
) -> HandlerResult<ReparseOutcome> {
    let total_start = Instant::now();

    // Validate input file paths
    for path in &input.file_paths {
        ctx.validator.validate_file_path(path)?;
    }

    // --- scan: the state the operation needs ---
    let scan_start = Instant::now();
    let store = ctx.get_graph_store();
    let existing_manifest = store
        .load_manifest()
        .map_err(|e| HandlerError::Internal(format!("Failed to load manifest: {}", e)))?
        .unwrap_or_else(|| FileManifest::new(ctx.working_dir.clone()));

    let observed = input
        .file_paths
        .iter()
        .map(|file_path| {
            // `join` keeps an absolute path as it is.
            let full_path = ctx.working_dir.join(file_path);
            let (exists, mtime) = observed_mtime(std::fs::metadata(&full_path)).map_err(|e| {
                HandlerError::Internal(format!("Cannot observe file metadata: {e}"))
            })?;
            let rel_path = full_path
                .strip_prefix(&ctx.working_dir)
                .unwrap_or(&full_path)
                .to_path_buf();
            Ok((full_path, rel_path, exists, mtime))
        })
        .collect::<HandlerResult<Vec<_>>>()?;
    let scan_ms = scan_start.elapsed().as_millis() as u64;

    // --- hash: the identity of every candidate against the manifest ---
    let hash_start = Instant::now();
    let mut decisions = Vec::with_capacity(observed.len());
    for (full_path, rel_path, exists, mtime) in observed {
        let entry = existing_manifest.entries.get(&rel_path);
        if !exists {
            // A file that is gone and that the manifest knew is a
            // removal. A file that was never there is not a removal:
            // there was nothing to remove.
            if entry.is_some() {
                decisions.push(Decision::Removed {
                    full: full_path,
                    rel: rel_path,
                });
            }
            continue;
        }
        // The manifest decides by the content, and the clock is only a
        // hint — the same rule the freshness of the snapshot follows.
        // A file whose bytes changed under an unchanged clock is a
        // changed file, and a file that was only touched is an
        // unchanged one.
        let clock_agrees =
            matches!((entry.and_then(|e| e.mtime), mtime), (Some(a), Some(b)) if a == b);
        let unchanged =
            clock_agrees && content_matches_manifest(&existing_manifest, &rel_path, &full_path);
        if unchanged {
            decisions.push(Decision::Unchanged);
        } else {
            decisions.push(Decision::Changed {
                full: full_path,
                rel: rel_path,
            });
        }
    }
    let hash_ms = hash_start.elapsed().as_millis() as u64;

    // The graph as it stands before this call: the baseline the symbol
    // counts are measured against. A graph that holds no symbols at all
    // measured nothing, and the counts it would produce are unknown
    // rather than zero.
    let baseline_graph = ctx.analysis_service.get_project_graph();

    // --- parse + index: the files whose content changed ---
    let mut parse_ms = 0u64;
    let mut index_ms = 0u64;
    let mut indexed_any = false;
    let mut files_parsed = 0usize;
    let mut files_skipped = 0usize;
    let mut files_removed = 0usize;
    let mut symbols_added = 0usize;
    let mut symbols_removed = 0usize;
    let mut symbols_unmeasured = 0usize;
    // Track changed absolute paths for graph rebuild
    let mut changed_abs_paths: Vec<PathBuf> = Vec::new();
    // The changed files whose symbols have to be counted in the rebuilt
    // graph. A file that was deleted is not here: it has no symbols left
    // to count, and its count is settled against the graph as it was.
    let mut changed_rel_paths: Vec<PathBuf> = Vec::new();

    for decision in decisions {
        match decision {
            Decision::Removed { full, rel } => {
                files_removed += 1;
                let delta = symbol_delta(
                    baseline_graph.symbol_count_for_file(&rel),
                    SymbolCount::Measured(0),
                );
                symbols_removed += delta.removed();
                if !delta.is_measured() {
                    symbols_unmeasured += 1;
                }
                changed_abs_paths.push(full);
            }
            Decision::Unchanged => files_skipped += 1,
            Decision::Changed { full, rel } => {
                files_parsed += 1;
                match ctx.semantic_search.index_file_from_path_timed(&full) {
                    Ok(timings) => {
                        parse_ms += timings.parse_ms;
                        index_ms += timings.index_ms;
                        indexed_any = true;
                        // Successfully indexed - symbols will be in
                        // DashMap and FTS5. How many they are is not
                        // published here: it is counted in the graph,
                        // once it has been rebuilt.
                        changed_abs_paths.push(full);
                        changed_rel_paths.push(rel);
                    }
                    Err(e) => {
                        // Parse failure - log at WARN and count as skipped
                        tracing::warn!("Failed to index modified file {:?}: {}", full, e);
                        files_skipped += 1;
                        files_parsed -= 1; // Don't count this as parsed
                    }
                }
            }
        }
    }

    let analysis_time_ms = total_start.elapsed().as_millis() as u64;

    // --- graph: the rebuild ---
    let graph_start = Instant::now();
    let mut rebuild_ran = false;
    let mut rebuild_failed = false;
    let mut graph_updated = false;
    if !changed_abs_paths.is_empty() {
        rebuild_ran = true;
        tracing::debug!(
            "Rebuilding call graph for {} changed files in reparse_on_edit",
            changed_abs_paths.len()
        );

        // Invalidate file cache so changed files are re-parsed
        ctx.analysis_service
            .invalidate_file_cache_for(&changed_abs_paths);

        // Rebuild the project graph to reflect the changes
        match ctx
            .analysis_service
            .build_project_graph_async(&ctx.working_dir)
            .await
        {
            Ok(()) => graph_updated = true,
            Err(e) => {
                rebuild_failed = true;
                tracing::warn!("Failed to rebuild call graph in reparse_on_edit: {}", e);
                // Don't fail the whole operation - FTS5/DashMap are already updated
            }
        }
    }
    let graph_ms = graph_start.elapsed().as_millis() as u64;

    // The symbols of the changed files, counted in the rebuilt graph. When
    // the graph could not be rebuilt there is no "after" to count against,
    // and the change is unknown rather than zero.
    if graph_updated {
        let rebuilt_graph = ctx.analysis_service.get_project_graph();
        for rel_path in &changed_rel_paths {
            let delta = symbol_delta(
                baseline_graph.symbol_count_for_file(rel_path),
                rebuilt_graph.symbol_count_for_file(rel_path),
            );
            symbols_added += delta.added();
            symbols_removed += delta.removed();
            if !delta.is_measured() {
                symbols_unmeasured += 1;
            }
        }
    } else {
        symbols_unmeasured += changed_rel_paths.len();
    }

    // What kind of rebuild this was. The coverage of the build says how
    // many files were interpreted again and how many were reused, and
    // those numbers decide the word.
    let coverage = ctx.analysis_service.get_coverage_metrics();
    let (parsed_files, total_files) = coverage
        .map(|metrics| (metrics.parsed_files, metrics.total_source_files))
        .unwrap_or((0, 0));
    let rebuild_kind = rebuild_kind_of(rebuild_ran, rebuild_failed, parsed_files, total_files);

    let mut phases = vec![
        PhaseCost {
            phase: ReparsePhase::Scan,
            duration_ms: scan_ms,
        },
        PhaseCost {
            phase: ReparsePhase::Hash,
            duration_ms: hash_ms,
        },
    ];
    if indexed_any {
        phases.push(PhaseCost {
            phase: ReparsePhase::Parse,
            duration_ms: parse_ms,
        });
        phases.push(PhaseCost {
            phase: ReparsePhase::Index,
            duration_ms: index_ms,
        });
    }
    if rebuild_ran {
        phases.push(PhaseCost {
            phase: ReparsePhase::Graph,
            duration_ms: graph_ms,
        });
    }

    let total_ms = total_start.elapsed().as_millis() as u64;
    let summary = ctx.analysis_service.record_reparse_cost(total_ms);
    let cost = ReparseCost {
        rebuild_kind,
        phases,
        affected_files: files_parsed + files_removed,
        considered_files: input.file_paths.len(),
        total_ms,
        peak_memory_bytes: read_peak_memory_bytes(),
        duration_p50_ms: summary.p50_ms,
        duration_p95_ms: summary.p95_ms,
        sample_size: summary.sample_size,
    };

    // The metrics are the instrumentation that already exists: when they
    // are not there, the operation does not pay for them.
    if let Some(metrics) = get_global_metrics() {
        let measured: Vec<(&str, f64)> = cost
            .phases
            .iter()
            .map(|phase| (phase.phase.published_label(), phase.duration_ms as f64))
            .collect();
        metrics.record_reparse(cost.rebuild_kind.published_label(), &measured);
    }
    tracing::info!(
        rebuild_kind = %cost.rebuild_kind.published_label(),
        total_ms = %cost.total_ms,
        affected_files = %cost.affected_files,
        "reparse_on_edit"
    );

    Ok(ReparseOutcome {
        files_parsed,
        files_skipped,
        files_removed,
        symbols_added,
        symbols_removed,
        symbols_unmeasured,
        graph_updated,
        cost,
        analysis_time_ms,
    })
}
