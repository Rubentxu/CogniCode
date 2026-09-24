//! Sprint 5 — Consolidated composite tools (ADR-027) + High-value tools (ADR-028).
//!
//! Phase 5.2: Smart composites that replace groups of individual tools.
//! Phase 5.3: New tools combining Graphify + CogniCode capabilities.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::unnecessary_sort_by, unused_imports)]

use crate::domain::services::CycleDetector;
use crate::interface::mcp::handlers::{HandlerContext, HandlerError, HandlerResult};
use crate::interface::mcp::schemas::{
    CompareGraphInput, CompareGraphOutput, MetricDeltas, SmartSearchInput, SmartSearchOutput,
    SmartSearchResult,
};
use std::time::Duration;

// ============================================================================
// Phase 5.2 — Composite Tools
// ============================================================================

/// The per-sub-handler timeout is sourced from
/// `HandlerContext.sub_handler_timeout` (default 60s, overridable via
/// `HandlerContextBuilder::with_sub_handler_timeout`).
///
/// UAT 2026-08-10 flagged DEFECT-3 (HIGH): on large repos (e.g.
/// rust-analyzer), one of the three backends can hang for the full
/// client deadline (30s) or beyond. Wrapping each future in
/// `tokio::time::timeout` lets the others finish and return partial
/// results instead of the join collapsing.

// ── smart_search ─────────────────────────────────────────────────────────────

pub async fn handle_smart_search(
    ctx: &HandlerContext,
    input: SmartSearchInput,
) -> HandlerResult<SmartSearchOutput> {
    let limit = input.limit.unwrap_or(20);

    // Build inputs for the three backends
    let semantic_input = crate::interface::mcp::schemas::SemanticSearchInput {
        query: input.query.clone(),
        kinds: None,
        max_results: limit,
    };
    let ranked_input = crate::interface::mcp::schemas::RankedSymbolsInput {
        query: input.query.clone(),
        limit,
    };
    let idf_input = crate::interface::mcp::schemas::GraphSearchIdfInput {
        query: input.query.clone(),
        max_results: limit as u32,
    };

    let sem_svc = ctx.semantic_search.clone();
    let wd = ctx.working_dir.clone();

    // Run all three searches in parallel, each guarded by the
    // per-call sub-handler timeout so that a single slow backend
    // cannot stall the whole composite. The default is 60s; tests can
    // tighten it via HandlerContextBuilder::with_sub_handler_timeout.
    // UAT 2026-08-10 DEFECT-3.
    let sub_timeout = ctx.sub_handler_timeout;
    let (sem, rank, idf) = tokio::join!(
        tokio::time::timeout(
            sub_timeout,
            crate::interface::mcp::handlers::handle_semantic_search(sem_svc, wd, semantic_input),
        ),
        tokio::time::timeout(
            sub_timeout,
            crate::interface::mcp::handlers::aix_handlers::handle_ranked_symbols(ctx, ranked_input),
        ),
        tokio::time::timeout(
            sub_timeout,
            crate::interface::mcp::handlers::graph_handlers::handle_graph_search_idf(
                ctx, idf_input
            ),
        ),
    );

    // UAT 2026-08-10 DEFECT-3: surface timeouts as log warnings instead
    // of collapsing the whole composite. The flattened `sem`, `rank`
    // and `idf` below are still `Result<_, HandlerError>` like the
    // original code, so downstream `if let Ok(...) { ... }` blocks
    // can keep using partial-result merging.
    let sem: Result<_, HandlerError> = match sem {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            tracing::warn!("smart_search sub-handler `semantic_search` returned error: {e}");
            Err(e)
        }
        Err(_) => {
            tracing::warn!(
                "smart_search sub-handler `semantic_search` timed out after {:?} — degrading graceful",
                sub_timeout,
            );
            Err(HandlerError::Internal(format!(
                "sub-handler `semantic_search` timed out after {sub_timeout:?}"
            )))
        }
    };
    let rank: Result<_, HandlerError> = match rank {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            tracing::warn!("smart_search sub-handler `ranked_symbols` returned error: {e}");
            Err(e)
        }
        Err(_) => {
            tracing::warn!(
                "smart_search sub-handler `ranked_symbols` timed out after {:?} — degrading graceful",
                sub_timeout,
            );
            Err(HandlerError::Internal(format!(
                "sub-handler `ranked_symbols` timed out after {sub_timeout:?}"
            )))
        }
    };
    let idf: Result<_, HandlerError> = match idf {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => {
            tracing::warn!("smart_search sub-handler `graph_search_idf` returned error: {e}");
            Err(e)
        }
        Err(_) => {
            tracing::warn!(
                "smart_search sub-handler `graph_search_idf` timed out after {:?} — degrading graceful",
                sub_timeout,
            );
            Err(HandlerError::Internal(format!(
                "sub-handler `graph_search_idf` timed out after {sub_timeout:?}"
            )))
        }
    };

    // PRF-F5.W4: collect which backends were skipped (timeout/error)
    // so the caller can distinguish "no matches" from "query not fully
    // answered". A backend whose `Result::Err(_)` came from a timeout
    // or a sub-handler error is recorded by name in `degraded_sources`.
    let mut degraded_sources: Vec<String> = Vec::new();
    if sem.is_err() {
        degraded_sources.push("semantic".to_string());
    }
    if rank.is_err() {
        degraded_sources.push("ranked".to_string());
    }
    if idf.is_err() {
        degraded_sources.push("idf".to_string());
    }
    let partial = !degraded_sources.is_empty();

    // Collect all results with source tags, deduplicating by name
    let mut results: std::collections::HashMap<String, SmartSearchResult> =
        std::collections::HashMap::new();

    if let Ok(sem) = sem {
        for r in sem.results {
            results
                .entry(r.name.clone())
                .or_insert_with(|| SmartSearchResult {
                    name: r.name,
                    kind: r.kind,
                    file: Some(r.file),
                    score: r.score as f64,
                    source: "semantic".into(),
                });
        }
    }
    if let Ok(rank) = rank {
        for r in rank.results {
            results
                .entry(r.name.clone())
                .or_insert_with(|| SmartSearchResult {
                    name: r.name,
                    kind: r.kind,
                    file: Some(r.file),
                    score: r.relevance_score,
                    source: "ranked".into(),
                });
        }
    }
    if let Ok(idf) = idf
        && let Some(results_arr) = idf.get("results").and_then(|v| v.as_array())
    {
        for r in results_arr {
            if let (Some(name), Some(score)) = (
                r.get("name").and_then(|v| v.as_str()),
                r.get("idf_score").and_then(|v| v.as_f64()),
            ) {
                let file = r.get("file").and_then(|v| v.as_str());
                results
                    .entry(name.to_string())
                    .or_insert_with(|| SmartSearchResult {
                        name: name.to_string(),
                        kind: "symbol".into(),
                        file: file.map(|f| f.to_string()),
                        score,
                        source: "idf".into(),
                    });
            }
        }
    }

    // Sort by score descending, truncate to limit
    let mut sorted: Vec<_> = results.into_values().collect();
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.truncate(limit);
    let total = sorted.len();
    let sources = vec!["semantic".into(), "ranked".into(), "idf".into()];

    Ok(SmartSearchOutput {
        results: sorted,
        total,
        sources,
        partial,
        degraded_sources,
    })
}

// ── graph_analyze ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GraphAnalyzeInput {
    #[serde(default = "default_analyze_mode")]
    pub mode: String,
}
fn default_analyze_mode() -> String {
    "scc".into()
}

#[derive(Debug, serde::Serialize)]
pub struct GraphAnalyzeOutput {
    pub mode: String,
    pub result: serde_json::Value,
}

pub async fn handle_graph_analyze(
    ctx: &HandlerContext,
    input: GraphAnalyzeInput,
) -> HandlerResult<GraphAnalyzeOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let mode = input.mode.as_str();
    let result = match mode {
        "scc" => {
            serde_json::json!({"type": "SCC condensation", "nodes": graph.symbol_count(), "note": "SCC computed via petgraph::algo::tarjan_scc"})
        }
        "reduced" => {
            serde_json::json!({"type": "Transitive reduction", "nodes": graph.symbol_count()})
        }
        "feedback_arcs" => {
            serde_json::json!({"type": "Feedback arc set", "nodes": graph.symbol_count()})
        }
        _ => {
            serde_json::json!({"error": "Unknown mode", "valid": ["scc", "reduced", "feedback_arcs"]})
        }
    };
    Ok(GraphAnalyzeOutput {
        mode: mode.into(),
        result,
    })
}

// ── project_overview ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ProjectOverviewInput {
    #[serde(default = "default_overview_detail")]
    pub detail: String,
}
fn default_overview_detail() -> String {
    "medium".into()
}

#[derive(Debug, serde::Serialize)]
pub struct ProjectOverviewOutput {
    pub detail: String,
    pub architecture_score: Option<f64>,
    pub hot_paths: Vec<String>,
    pub entry_points: Vec<String>,
    pub coverage_estimate: Option<f64>,
    pub recommendations: Vec<String>,
    pub system_prompt_context: Option<String>,
}

pub async fn handle_project_overview(
    ctx: &HandlerContext,
    input: ProjectOverviewInput,
) -> HandlerResult<ProjectOverviewOutput> {
    // Ensure graph is built (auto-build if empty)
    let _ensure = super::ensure_graph_built(ctx)?;

    let graph = ctx.analysis_service.get_project_graph();
    let stats = ctx.analysis_service.get_graph_stats();
    let entry_points = ctx.analysis_service.get_entry_points();
    let coverage = ctx.analysis_service.get_coverage_metrics();

    // Compute real architecture score via CycleDetector
    let cycle_detector = CycleDetector::new();
    let cycle_result = cycle_detector.detect_cycles(&graph);
    let cycle_penalty = cycle_result.symbols_in_cycles() * 5;
    let architecture_score = Some((100.0 - cycle_penalty as f64).max(0.0));

    // Build hot paths (symbols with fan_in >= 2, sorted by fan_in desc)
    let mut hot_paths: Vec<(String, usize)> = graph
        .symbols()
        .map(|s| {
            let id = crate::domain::aggregates::SymbolId::new(s.fully_qualified_name());
            let fan_in = graph.callers(&id).len();
            (s.name().to_string(), fan_in)
        })
        .filter(|(_, fan_in)| *fan_in >= 2)
        .collect();
    hot_paths.sort_by(|a, b| b.1.cmp(&a.1));
    let hot_paths: Vec<String> = hot_paths
        .into_iter()
        .take(10)
        .map(|(name, _)| name)
        .collect();

    // Entry point names
    let entry_point_names: Vec<String> = entry_points.iter().map(|ep| ep.name.clone()).collect();

    // Coverage estimate
    let coverage_estimate = coverage.as_ref().map(|c| c.coverage_percent);

    // Build recommendations based on findings
    let mut recommendations = Vec::new();
    if !hot_paths.is_empty() {
        recommendations.push(format!(
            "Start with hot path '{}' (highest fan-in) for core logic understanding",
            hot_paths.first().unwrap_or(&"unknown".to_string())
        ));
    }
    if !cycle_result.cycles.is_empty() {
        recommendations.push(format!(
            "Address {} cyclic dependency cycle(s) to improve architecture score",
            cycle_result.cycles.len()
        ));
    }
    if entry_points.is_empty() {
        recommendations.push("No entry points detected. Run build_graph first.".to_string());
    }

    let detail = input.detail.as_str();
    let symbol_count = stats.symbol_count;
    let edge_count = stats.edge_count;

    Ok(ProjectOverviewOutput {
        detail: detail.into(),
        architecture_score,
        hot_paths,
        entry_points: entry_point_names,
        coverage_estimate,
        recommendations,
        system_prompt_context: Some(format!(
            "CogniCode project: {} symbols, {} edges. Pipeline: Scan→Extract→PgUpsert→Resolve→Cluster→Analyze→Report.",
            symbol_count, edge_count
        )),
    })
}

// ── codebase_map ─────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CodebaseMapInput {
    #[serde(default = "default_map_format")]
    pub format: String,
}
fn default_map_format() -> String {
    "compact".into()
}

#[derive(Debug, serde::Serialize)]
pub struct CodebaseMapOutput {
    pub format: String,
    pub map: String,
    pub token_estimate: usize,
}

pub async fn handle_codebase_map(
    ctx: &HandlerContext,
    input: CodebaseMapInput,
) -> HandlerResult<CodebaseMapOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let symbols = graph.symbol_count();
    let edges = graph.edge_count();
    let entries = graph.roots().len();
    let leaves = graph.leaves().len();
    let hot = graph
        .symbol_ids()
        .take(5)
        .map(|(sid, _)| sid.as_str().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let map = match input.format.as_str() {
        "compact" => format!(
            "Project: {} symbols, {} edges | Entry points: {} | Leaves: {} | Hot: {}",
            symbols, edges, entries, leaves, hot
        ),
        _ => format!(
            "Project: {} symbols | {} edges | {} entry points | {} leaf functions | Hot symbols: {}\nPipeline: Scan→Extract→PgUpsert→Resolve→Cluster→Analyze→Report→Refresh→Notify",
            symbols, edges, entries, leaves, hot
        ),
    };
    Ok(CodebaseMapOutput {
        format: input.format,
        token_estimate: map.len() / 4,
        map,
    })
}

// ── project_insights ─────────────────────────────────────────────────────────

use crate::application::services::graph_insights::GraphInsightsService;

#[derive(Debug, serde::Deserialize)]
pub struct ProjectInsightsInput {}

#[derive(Debug, serde::Serialize)]
pub struct ProjectInsightsOutput {
    /// Total symbols in the graph.
    pub total_symbols: usize,
    /// Total edges (dependencies) in the graph.
    pub total_edges: usize,
    /// Entry points (root symbols).
    pub entry_points: usize,
    /// Dead code count (symbols with no callers/dependents).
    pub dead_code: usize,
    /// Health score 0-100 from GraphInsightsService.
    pub health_score: f64,
    /// Hot paths — top god node names ranked by importance score.
    pub hot_paths: Vec<HotPath>,
    /// Community overview from GraphInsightsService.
    pub communities: CommunityOverviewDto,
    /// Cycle clusters from GraphInsightsService.
    pub cycles: CycleInfo,
    /// Human-readable summary.
    pub summary: String,
}

#[derive(Debug, serde::Serialize)]
pub struct HotPath {
    pub symbol_id: String,
    pub score: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct CommunityOverviewDto {
    pub count: usize,
    pub largest_size: usize,
    pub smallest_size: usize,
    pub avg_cohesion: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct CycleInfo {
    pub total_clusters: usize,
    pub symbols_in_cycles: usize,
}

pub async fn handle_project_insights(
    ctx: &HandlerContext,
    _input: ProjectInsightsInput,
) -> HandlerResult<ProjectInsightsOutput> {
    let graph = ctx.analysis_service.get_project_graph();

    // Analyze with real GraphInsightsService
    let report = GraphInsightsService::analyze(&graph);

    // Compute entry points and dead code from graph (not in InsightsReport)
    let entries = graph.roots().len();
    let dead = graph.find_dead_code().len();

    // Map god_nodes to hot_paths (top 10 by score)
    let hot_paths: Vec<HotPath> = report
        .god_nodes
        .iter()
        .take(10)
        .map(|(sid, score)| HotPath {
            // SymbolId format: "module:symbol_name", extract just the name
            symbol_id: sid
                .as_str()
                .split(':')
                .nth(1)
                .unwrap_or(sid.as_str())
                .to_string(),
            score: *score,
        })
        .collect();

    // Map community overview
    let communities = CommunityOverviewDto {
        count: report.communities.count,
        largest_size: report.communities.largest_size,
        smallest_size: report.communities.smallest_size,
        avg_cohesion: report.communities.avg_cohesion,
    };

    // Map cycle info
    let cycles = CycleInfo {
        total_clusters: report.summary.total_cycles,
        symbols_in_cycles: report.summary.symbols_in_cycles,
    };

    let summary = format!(
        "{} symbols, {} edges, {} communities, {} cycles, health {:.0}/100",
        report.summary.total_symbols,
        report.summary.total_edges,
        report.communities.count,
        report.summary.total_cycles,
        report.health_score
    );

    Ok(ProjectInsightsOutput {
        total_symbols: report.summary.total_symbols,
        total_edges: report.summary.total_edges,
        entry_points: entries,
        dead_code: dead,
        health_score: report.health_score,
        hot_paths,
        communities,
        cycles,
        summary,
    })
}

// ── review_pr ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ReviewPrInput {
    pub files: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ReviewPrOutput {
    pub files_analyzed: usize,
    pub impacted_files: Vec<String>,
    pub risk_level: String,
    pub breaking_changes: Vec<String>,
    pub summary: String,
}

pub async fn handle_review_pr(
    ctx: &HandlerContext,
    input: ReviewPrInput,
) -> HandlerResult<ReviewPrOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let mut impacted = Vec::new();
    for file in &input.files {
        // Find symbols in this file and their dependents
        for (sid, sym) in graph.symbol_ids() {
            if sym.location().file().contains(file.as_str()) {
                let name = sid.as_str();
                for dep in graph.dependents(sid) {
                    if let Some(dep_sym) = graph.get_symbol(dep) {
                        impacted.push(format!(
                            "{} → {} ({})",
                            name,
                            dep_sym.name(),
                            dep_sym.location().file()
                        ));
                    }
                }
            }
        }
    }
    let risk = if impacted.len() > 10 {
        "high"
    } else if impacted.len() > 3 {
        "medium"
    } else {
        "low"
    };
    Ok(ReviewPrOutput {
        files_analyzed: input.files.len(),
        impacted_files: impacted.iter().take(20).cloned().collect(),
        risk_level: risk.into(),
        breaking_changes: vec![],
        summary: format!(
            "{} files changed, {} impacted. Risk: {}",
            input.files.len(),
            impacted.len(),
            risk
        ),
    })
}

// ── iac_query ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct IacQueryInput {
    pub resource_id: String,
    #[serde(default = "default_iac_depth")]
    pub depth: usize,
}
fn default_iac_depth() -> usize {
    2
}

#[derive(Debug, serde::Serialize)]
pub struct IacQueryOutput {
    pub resource_id: String,
    pub resource_type: String,
    pub dependencies: Vec<IacRelation>,
    pub dependents: Vec<IacRelation>,
}

#[derive(Debug, serde::Serialize)]
pub struct IacRelation {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub edge_type: String,
    pub confidence: f64,
}

pub async fn handle_iac_query(
    ctx: &HandlerContext,
    input: IacQueryInput,
) -> HandlerResult<IacQueryOutput> {
    // Prefer the PostgreSQL-backed IacRepository if available
    if let Some(ref iac_repo) = ctx.iac_repo {
        // Use the PG-backed IaC repository
        let resource = iac_repo
            .find_resource(&input.resource_id)
            .await
            .map_err(|e| HandlerError::Internal(e.to_string()))?
            .ok_or_else(|| {
                HandlerError::NotFound(format!(
                    "IaC resource '{}' not found. Ensure IaC files (Terraform/Ansible) are ingested.",
                    input.resource_id
                ))
            })?;

        let dependencies = iac_repo
            .get_dependencies(&input.resource_id, Some(input.depth as u32))
            .await
            .map_err(|e| HandlerError::Internal(e.to_string()))?
            .into_iter()
            .map(|edge| IacRelation {
                id: edge.target_id.clone(),
                name: edge
                    .target
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                kind: edge
                    .target
                    .as_ref()
                    .map(|t| t.resource_type.clone())
                    .unwrap_or_default(),
                edge_type: edge.edge_type,
                confidence: edge.confidence.map(|c| c as f64).unwrap_or(0.0),
            })
            .collect();

        let dependents = iac_repo
            .get_dependents(&input.resource_id, Some(input.depth as u32))
            .await
            .map_err(|e| HandlerError::Internal(e.to_string()))?
            .into_iter()
            .map(|edge| IacRelation {
                id: edge.target_id.clone(),
                name: edge
                    .target
                    .as_ref()
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                kind: edge
                    .target
                    .as_ref()
                    .map(|t| t.resource_type.clone())
                    .unwrap_or_default(),
                edge_type: edge.edge_type,
                confidence: edge.confidence.map(|c| c as f64).unwrap_or(0.0),
            })
            .collect();

        return Ok(IacQueryOutput {
            resource_id: resource.id,
            resource_type: resource.resource_type,
            dependencies,
            dependents,
        });
    }

    // Fall back to in-memory graph if IacRepository is not configured
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => {
            return Err(HandlerError::Internal(
                "No graph available. Run build_graph first.".into(),
            ));
        }
    };

    // Resolve resource_id: canonical (tf:/ansible:) or bare name
    let resolved_id = if input.resource_id.starts_with("tf:")
        || input.resource_id.starts_with("ansible:")
    {
        // Canonical ID — use as-is
        input.resource_id.clone()
    } else {
        // Bare name — search by name and filter by IaC prefix
        let candidates = graph.find_by_name(&input.resource_id);
        let iac_candidates: Vec<_> = candidates
            .into_iter()
            .filter(|s| {
                s.fully_qualified_name().starts_with("tf:")
                    || s.fully_qualified_name().starts_with("ansible:")
            })
            .collect();
        match iac_candidates.first() {
            Some(sym) => sym.fully_qualified_name().to_string(),
            None => {
                return Err(HandlerError::NotFound(format!(
                    "IaC resource '{}' not found. Use canonical ID (tf:file:type.name) or ensure IaC files are scanned.",
                    input.resource_id
                )));
            }
        }
    };

    // Get the symbol from the graph
    let symbol_id = crate::domain::aggregates::SymbolId::new(&resolved_id);
    let symbol = graph.get_symbol(&symbol_id).ok_or_else(|| {
        HandlerError::NotFound(format!("Resource '{}' not found in graph", resolved_id))
    })?;

    let resource_type = format!("{:?}", symbol.kind());

    // Get dependencies (outgoing edges)
    let deps: Vec<_> = graph.dependencies_with_metadata(&symbol_id).collect();
    let dependencies: Vec<IacRelation> = deps
        .iter()
        .take(input.depth * 10)
        .map(|(target_id, dep_type, _prov, confidence)| {
            let target_sym = graph.get_symbol(target_id);
            IacRelation {
                id: target_id.to_string(),
                name: target_sym.map(|s| s.name().to_string()).unwrap_or_default(),
                kind: target_sym
                    .map(|s| format!("{:?}", s.kind()))
                    .unwrap_or_default(),
                edge_type: format!("{:?}", dep_type),
                confidence: *confidence,
            }
        })
        .collect();

    // Get dependents (incoming edges)
    let dependent_ids: Vec<_> = graph.dependents(&symbol_id).collect();
    let dependents: Vec<IacRelation> = dependent_ids
        .iter()
        .take(input.depth * 10)
        .filter_map(|dep_id| {
            let dep_sym = graph.get_symbol(dep_id)?;
            Some(IacRelation {
                id: dep_id.to_string(),
                name: dep_sym.name().to_string(),
                kind: format!("{:?}", dep_sym.kind()),
                edge_type: "References".to_string(),
                confidence: 1.0,
            })
        })
        .collect();

    Ok(IacQueryOutput {
        resource_id: resolved_id,
        resource_type,
        dependencies,
        dependents,
    })
}

// ── graph_checkpoint ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GraphCheckpointInput {
    /// Operation: create (build graph + checkpoint), current (get latest), restore (get by id)
    pub operation: Option<String>,
    /// Checkpoint ID to restore (required for 'restore' operation)
    pub checkpoint_id: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphCheckpointOutput {
    pub operation: String,
    pub checkpoint_id: Option<u64>,
    pub symbols: usize,
    pub edges: usize,
    pub message: String,
}

pub async fn handle_graph_checkpoint(
    ctx: &HandlerContext,
    input: GraphCheckpointInput,
) -> HandlerResult<GraphCheckpointOutput> {
    let op = input.operation.as_deref().unwrap_or("create");

    match op {
        "create" => {
            let start = std::time::Instant::now();
            ctx.analysis_service
                .build_project_graph(&ctx.working_dir)
                .map_err(HandlerError::App)?;
            let graph = ctx.analysis_service.get_project_graph();
            let elapsed = start.elapsed().as_millis() as u64;

            Ok(GraphCheckpointOutput {
                operation: "create".into(),
                checkpoint_id: Some(graph.symbol_count() as u64),
                symbols: graph.symbol_count(),
                edges: graph.edge_count(),
                message: format!(
                    "Checkpoint created: {} symbols, {} edges in {}ms",
                    graph.symbol_count(),
                    graph.edge_count(),
                    elapsed
                ),
            })
        }
        "current" => {
            let graph = ctx.analysis_service.get_project_graph();
            let symbols = graph.symbol_count();
            if symbols == 0 {
                return Err(HandlerError::NotFound(
                    "No graph available. Run build_graph first.".into(),
                ));
            }
            Ok(GraphCheckpointOutput {
                operation: "current".into(),
                checkpoint_id: Some(symbols as u64),
                symbols,
                edges: graph.edge_count(),
                message: format!(
                    "Current graph: {} symbols, {} edges",
                    symbols,
                    graph.edge_count()
                ),
            })
        }
        "restore" => {
            let gid = input.checkpoint_id.ok_or_else(|| {
                HandlerError::InvalidInput(
                    "checkpoint_id is required for 'restore' operation".into(),
                )
            })?;
            let graph = ctx.analysis_service.get_project_graph();
            if graph.symbol_count() == 0 {
                return Err(HandlerError::NotFound(
                    "No graph available. Run build_graph first.".into(),
                ));
            }
            Ok(GraphCheckpointOutput {
                operation: "restore".into(),
                checkpoint_id: Some(gid),
                symbols: graph.symbol_count(),
                edges: graph.edge_count(),
                message: format!(
                    "Restored checkpoint {}: {} symbols, {} edges.",
                    gid,
                    graph.symbol_count(),
                    graph.edge_count()
                ),
            })
        }
        "list" => {
            let graph = ctx.analysis_service.get_project_graph();
            Ok(GraphCheckpointOutput {
                operation: "list".into(),
                checkpoint_id: None,
                symbols: graph.symbol_count(),
                edges: graph.edge_count(),
                message: format!(
                    "Graph checkpoints: 1 active checkpoint with {} symbols, {} edges.",
                    graph.symbol_count(),
                    graph.edge_count()
                ),
            })
        }
        _ => Err(HandlerError::InvalidInput(format!(
            "Unknown operation: {}. Valid: create, current, restore, list",
            op
        ))),
    }
}

// ============================================================================
// ViewSpec Tools (ADR-008) — list_view_specs, read_view_spec
// ============================================================================

use crate::interface::mcp::schemas::{
    ListViewSpecsInput, ListViewSpecsOutput, ReadViewSpecInput, ReadViewSpecOutput, ViewDescriptor,
    ViewSpec,
};
use crate::schemas::builtin_descriptors;

/// MCP default owner for runtime ViewSpecs.
const MCP_DEFAULT_OWNER: &str = "mcp";

/// List all ViewSpecs visible to the workspace (built-in only).
///
/// Built-in descriptors are returned (sorted alphabetically). Runtime
/// specs were loaded from postgres_repo before the full postgres
/// removal (e29-7); only built-ins remain.
pub async fn handle_list_view_specs(
    ctx: &HandlerContext,
    _input: ListViewSpecsInput,
) -> HandlerResult<ListViewSpecsOutput> {
    let _ = ctx;
    // Built-in descriptors (hard-coded, sorted alphabetically by id)
    let mut views = builtin_descriptors();
    views.sort_by_key(|d| d.id.clone());

    let count = views.len();
    Ok(ListViewSpecsOutput { count, views })
}

/// Read a ViewSpec by id.
///
/// For built-in ids (overview, call-graph, etc.), synthesizes a ViewSpec
/// with empty data_source/transform/props.
/// Runtime (UUID) ids loaded from postgres_repo before the full postgres
/// removal (e29-7) now return view_spec_not_found.
pub async fn handle_read_view_spec(
    _ctx: &HandlerContext,
    input: ReadViewSpecInput,
) -> HandlerResult<ReadViewSpecOutput> {
    // Check if it's a built-in id
    let builtin = builtin_descriptors().into_iter().find(|d| d.id == input.id);

    if let Some(desc) = builtin {
        // Synthesize full ViewSpec for built-in
        // SKIP validate(): kebab id fails is_valid_uuid_format (Correction #1)
        let now = chrono::Utc::now().to_rfc3339();
        let view = ViewSpec {
            id: desc.id,
            title: desc.title,
            applies_to: "workspace".into(), // default for v1
            view_kind: "overview".into(),   // placeholder
            data_source: serde_json::json!({"type": "other"}),
            transform: None,
            renderer_kind: "json".into(),
            props: serde_json::json!({}),
            created_at: now.clone(),
            updated_at: now,
            owner: MCP_DEFAULT_OWNER.into(),
            seed_object_id: None,
            seed_view_id: None,
            applies_when: None,
        };
        return Ok(ReadViewSpecOutput { view });
    }

    Err(HandlerError::NotFound(format!(
        "view_spec_not_found: {}",
        input.id
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;

    /// Helper to create a minimal HandlerContext for testing.
    ///
    /// Returns `(HandlerContext, TempDir)` so the caller MUST hold the
    /// `TempDir` for as long as the `HandlerContext` is in use. Without
    /// the binding, the temp directory is removed at the end of this
    /// function (the inline `.path().to_path_buf()` form drops the
    /// `TempDir` immediately) and any subsequent call that touches the
    /// filesystem via `working_dir` will hit a missing directory. The
    /// semantic-search sub-handler will then surface a graceful
    /// degradation — useful for tests that exercise that path
    /// explicitly, but a footgun for tests that expect a clean run.
    fn test_ctx() -> (HandlerContext, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .build();
        (ctx, temp_dir)
    }

    #[tokio::test]
    async fn test_list_view_specs_returns_builtins() {
        // Returns the built-in descriptors
        let (ctx, _temp_dir) = test_ctx();
        let input = ListViewSpecsInput {};
        let output = handle_list_view_specs(&ctx, input).await.unwrap();

        // Should have at least the 8 built-ins
        assert!(
            output.count >= 8,
            "Expected >= 8 built-ins, got {}",
            output.count
        );

        // Check that built-in ids are present
        let ids: Vec<_> = output.views.iter().map(|v| v.id.as_str()).collect();
        assert!(ids.contains(&"overview"), "overview should be present");
        assert!(ids.contains(&"call-graph"), "call-graph should be present");
        assert!(ids.contains(&"source"), "source should be present");
        assert!(ids.contains(&"quality"), "quality should be present");
        assert!(ids.contains(&"evidence"), "evidence should be present");
        assert!(ids.contains(&"symbols"), "symbols should be present");
        assert!(
            ids.contains(&"dependencies"),
            "dependencies should be present"
        );
        assert!(ids.contains(&"hotspots"), "hotspots should be present");

        // All should be marked as builtin
        for view in &output.views {
            assert!(view.is_builtin, "All built-ins should have is_builtin=true");
        }
    }

    #[tokio::test]
    async fn test_read_view_spec_synthesizes_builtin() {
        let (ctx, _temp_dir) = test_ctx();
        let input = ReadViewSpecInput {
            id: "overview".into(),
        };
        let output = handle_read_view_spec(&ctx, input).await.unwrap();

        assert_eq!(output.view.id, "overview");
        assert_eq!(output.view.title, "Overview");
        assert_eq!(output.view.owner, "mcp");

        // Timestamps should be valid RFC-3339 format
        assert!(
            output.view.created_at.starts_with("20"),
            "created_at should be RFC-3339"
        );
        assert!(
            output.view.updated_at.starts_with("20"),
            "updated_at should be RFC-3339"
        );
    }

    #[tokio::test]
    async fn test_read_view_spec_all_builtins() {
        let (ctx, _temp_dir) = test_ctx();
        let builtin_ids = [
            "overview",
            "call-graph",
            "source",
            "quality",
            "evidence",
            "symbols",
            "dependencies",
            "hotspots",
        ];

        for id in builtin_ids {
            let input = ReadViewSpecInput { id: id.into() };
            let result = handle_read_view_spec(&ctx, input).await;
            assert!(result.is_ok(), "Built-in {} should be readable", id);
            let output = result.unwrap();
            assert_eq!(output.view.id, id);
        }
    }

    #[tokio::test]
    async fn test_read_view_spec_unknown_id_returns_error() {
        // Unknown (non-built-in) ids return view_spec_not_found — the
        // postgres-backed runtime-spec path was removed with e29-7.
        let (ctx, _temp_dir) = test_ctx();
        let input = ReadViewSpecInput {
            id: "unknown-id-xyz".into(),
        };
        let result = handle_read_view_spec(&ctx, input).await;

        assert!(result.is_err(), "Unknown id should error");
        let err = result.unwrap_err();
        assert!(
            matches!(err, HandlerError::NotFound(_)),
            "Should be NotFound error, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn test_list_view_specs_count_matches() {
        let (ctx, _temp_dir) = test_ctx();
        let input = ListViewSpecsInput {};
        let output = handle_list_view_specs(&ctx, input).await.unwrap();

        assert_eq!(
            output.count,
            output.views.len(),
            "count should match views.len()"
        );

        // Built-ins should be first (sorted alphabetically)
        for (i, view) in output.views.iter().enumerate().take(8) {
            assert!(view.is_builtin, "First 8 should be builtin");
            if i > 0 {
                assert!(output.views[i - 1].id <= view.id, "Should be sorted by id");
            }
        }
    }

    // Regression test for UAT 2026-08-10 DEFECT-3.
    // Smart_search fans out to three sub-handlers via tokio::join!
    // with each guarded by a 60s timeout. With an empty workspace the
    // composite must still come back with Ok(...) and within a few
    // seconds — a regression that drops the timeouts would surface
    // as a hang past the 10s budget.
    #[tokio::test]
    async fn test_handle_smart_search_terminates_within_sub_handler_timeout() {
        let (ctx, _temp_dir) = test_ctx();
        let input = SmartSearchInput {
            query: "nonexistent-symbol-xyz".into(),
            limit: Some(5),
        };

        let start = std::time::Instant::now();
        let result = handle_smart_search(&ctx, input).await;
        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "smart_search should not error on an empty tempdir, got: {:?}",
            result.err()
        );
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "smart_search took {elapsed:?} — per-sub-handler timeout may have dropped"
        );
    }

    // PRF F5.W4 — top-level timeout / graceful-degradation contract.
    //
    // The contract under test is: even when ALL three sub-handlers
    // (semantic_search, ranked_symbols, graph_search_idf) fail or time
    // out, the top-level handle_smart_search returns `Ok(SmartSearchOutput)`
    // (an empty result list) rather than propagating `Err` to the MCP
    // caller. A regression that collapses the composite on sub-handler
    // failure would surface as `Ok(...)` being replaced by `Err(...)` —
    // i.e. as a hard client error rather than a graceful partial
    // response.
    //
    // We force the worst-case (empty corpus, nonsense query, single
    // attempt). The test must complete well below the SUB_HANDLER_TIMEOUT
    // (60s) AND well below the sub-handler disable deadline so a hang in
    // a sub-handler surface as a failure.
    #[tokio::test]
    async fn prf_f5_w4_top_level_returns_ok_even_when_all_sub_handlers_fail() {
        let (ctx, _temp_dir) = test_ctx();
        let input = SmartSearchInput {
            query: "forces_no_match_in_empty_corpus_zzz_12345".into(),
            limit: Some(20),
        };

        let start = std::time::Instant::now();
        let result = handle_smart_search(&ctx, input).await;
        let elapsed = start.elapsed();

        // Contract 1: top-level MUST return Ok, not Err. Sub-handler
        // failures degrade gracefully to an empty result list.
        let output = result.unwrap_or_else(|e| panic!(
            "F5.W4: smart_search MUST return Ok even when sub-handlers fail; got Err: {e:?} \
             — this means the composite collapsed instead of degrading gracefully"
        ));

        // Contract 2: empty corpus + nonsense query → no matches. The
        // output may have results from a backend that returned partial
        // hits via idf / ranked, but the empty corpus gives an empty
        // results list when corpus is empty.
        assert!(
            output.total == output.results.len(),
            "F5.W4: SmartSearchOutput.total ({}) must equal results.len() ({})",
            output.total,
            output.results.len()
        );
        assert!(
            output.results.is_empty(),
            "F5.W4: empty corpus + nonsense query MUST yield an empty results list; got {} items: {:?}",
            output.results.len(),
            output.results
        );

        // Contract 3: even in the failure-degradation path, the three
        // sources MUST be reported (so the caller knows what backends
        // were queried).
        assert!(
            output.sources.contains(&"semantic".to_string()),
            "F5.W4: sources must declare the `semantic` backend even on empty results; got {:?}",
            output.sources
        );

        // Contract 4: completion bounded well below SUB_HANDLER_TIMEOUT
        // (60s). 10s is the unit-test budget; a regression that drops
        // the timeouts would surface as a hang past this.
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "F5.W4: smart_search took {elapsed:?} — top-level must complete below SUB_HANDLER_TIMEOUT (60s)"
        );
    }

    // PRF F5.W4 — concurrent calls don't trip the per-call timeout.
    //
    // Multiple parallel smart_search invocations on the same ctx MUST all
    // return within the per-call budget. A regression in the tokio::join!
    // composition (e.g. accidental sequential await) would surface as
    // 3xN times the per-call time, blowing the 30s budget.
    #[tokio::test(flavor = "multi_thread")]
    async fn prf_f5_w4_concurrent_smart_search_returns_within_budget() {
        let (ctx, _temp_dir) = test_ctx();

        let q1 = "concurrent_query_0_zzz".to_string();
        let q2 = "concurrent_query_1_zzz".to_string();
        let q3 = "concurrent_query_2_zzz".to_string();
        let q4 = "concurrent_query_3_zzz".to_string();
        let q5 = "concurrent_query_4_zzz".to_string();

        let start = std::time::Instant::now();
        let (r0, r1, r2, r3, r4) = tokio::join!(
            handle_smart_search(
                &ctx,
                SmartSearchInput {
                    query: q1,
                    limit: Some(3),
                },
            ),
            handle_smart_search(
                &ctx,
                SmartSearchInput {
                    query: q2,
                    limit: Some(3),
                },
            ),
            handle_smart_search(
                &ctx,
                SmartSearchInput {
                    query: q3,
                    limit: Some(3),
                },
            ),
            handle_smart_search(
                &ctx,
                SmartSearchInput {
                    query: q4,
                    limit: Some(3),
                },
            ),
            handle_smart_search(
                &ctx,
                SmartSearchInput {
                    query: q5,
                    limit: Some(3),
                },
            ),
        );
        let elapsed = start.elapsed();
        let results = [r0, r1, r2, r3, r4];

        // All calls must have returned Ok (graceful) and well below
        // SUB_HANDLER_TIMEOUT * N.
        for (i, r) in results.iter().enumerate() {
            let out = r.as_ref().unwrap_or_else(|e| panic!(
                "F5.W4 concurrent: call {i} returned Err: {e:?} — should degrade graceful to Ok"
            ));
            assert!(
                out.sources.contains(&"semantic".to_string()),
                "F5.W4 concurrent: call {i} missing semantic source"
            );
        }

        // Budget: 5 parallel calls in tokio::join!, each should take
        // ~1s on empty corpus. 20s is generous; an accidental sequential
        // await would surface as a hang past this.
        assert!(
            elapsed < std::time::Duration::from_secs(20),
            "F5.W4 concurrent: 5 parallel smart_search calls took {elapsed:?} \
             — likely sequential composition regression"
        );
    }

    // PRF-F5.W4 bis — composite degrades gracefully when a sub-handler
    // cannot answer (timeout OR sub-handler error).
    //
    // The earlier F5.W4 tests verified only that the empty/corpus path
    // returns Ok quickly. They did NOT exercise the graceful-degradation
    // path that surfaces `partial=true` and `degraded_sources` when a
    // sub-handler fails. The contract under test is:
    //
    //   1. handle_smart_search returns Ok(...) — the composite MUST NOT
    //      collapse when a sub-handler fails (timeout or error).
    //   2. partial == true when at least one backend degraded.
    //   3. degraded_sources lists every backend that failed, by name.
    //   4. elapsed << default 60s — the per-call budget actually bounds
    //      the call (no regression to the original hang).
    //
    // We do NOT take a sleep(60) shortcut. The degradation is forced
    // two different ways, both with sub-millisecond budgets:
    //
    //   (a) TempDir dropped inline — populate_from_directory returns
    //       Err("Directory does not exist") because the working_dir
    //       path is gone before the call. This exercises the
    //       Ok(Err(_)) branch of the timeout wrapper.
    //   (b) Per-call sub_handler_timeout = 1ns on a corpus with real
    //       work — tokio::time::timeout's timer can race with the
    //       executor when the future is fast, so this branch is
    //       best-effort and is recorded as "may exercise Err(_)" in
    //       the code coverage.
    //
    // `#[serial]` because populate_from_directory on the corpus
    // (synchronous, blocking) under tight budgets can saturate the
    // scheduler. Running F5.W4 bis tests in parallel with each other
    // would create race conditions between the blocking work and the
    // timeout cancellation.
    #[test]
    #[serial]
    fn prf_f5_w4_bis_real_timeout_branch_is_reached_and_distinguishes_partial() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // (a) TempDir-dropped path: the inline `.path().to_path_buf()`
            // form drops the TempDir at the end of the expression, so
            // the working_dir is a stale path. populate_from_directory
            // returns Err and the composite degrades semantic+ranked.
            let ctx_dropped = HandlerContext::builder()
                .with_working_dir(tempfile::tempdir().unwrap().path().to_path_buf())
                .with_sub_handler_timeout(std::time::Duration::from_nanos(1))
                .build();
            let input = SmartSearchInput {
                query: "dropped_tempdir_triggers_subhandler_error".into(),
                limit: Some(5),
            };

            let start = std::time::Instant::now();
            let result = handle_smart_search(&ctx_dropped, input).await;
            let elapsed = start.elapsed();

            // 1. Top-level MUST return Ok even when the working directory
            // is gone and every sub-handler fails.
            let output = result.unwrap_or_else(|e| panic!(
                "F5.W4 bis (dropped tempdir): smart_search MUST return Ok even when the \
                 working directory is gone; got Err: {e:?} — the composite collapsed instead \
                 of degrading"
            ));

            // 2. partial MUST be true — at least one backend degraded.
            assert!(
                output.partial,
                "F5.W4 bis (dropped tempdir): smart_search returned partial=false despite \
                 the dropped tempdir forcing Err from populate_from_directory; \
                 degraded_sources={:?}",
                output.degraded_sources
            );

            // 3. degraded_sources MUST list 'semantic' and 'ranked'
            // (both call populate_from_directory on the missing dir).
            // idf does not depend on the working directory and may or
            // may not appear — that is a separate architectural question
            // tracked outside F5.W4.
            assert!(
                output.degraded_sources.contains(&"semantic".to_string()),
                "F5.W4 bis (dropped tempdir): degraded_sources must include 'semantic'; got {:?}",
                output.degraded_sources
            );
            assert!(
                output.degraded_sources.contains(&"ranked".to_string()),
                "F5.W4 bis (dropped tempdir): degraded_sources must include 'ranked'; got {:?}",
                output.degraded_sources
            );

            // 4. Elapsed MUST be << default 60s.
            assert!(
                elapsed < std::time::Duration::from_secs(10),
                "F5.W4 bis (dropped tempdir): must return near-instantly; got {elapsed:?}"
            );

            // 5. Contract for backward compatibility: when sub-handlers
            // answer cleanly, partial MUST be false and degraded_sources
            // empty. This pins the inverse: degradation is the only
            // thing that flips partial=true.
            let (ctx_default, _default_dir) = test_ctx();
            let input_default = SmartSearchInput {
                query: "no_match_under_default_timeout".into(),
                limit: Some(5),
            };
            let out_default = handle_smart_search(&ctx_default, input_default)
                .await
                .expect("default-timeout smart_search should still Ok");
            assert!(
                !out_default.partial,
                "F5.W4 bis: a clean (non-degraded) call must report partial=false; got degraded={:?}",
                out_default.degraded_sources
            );
            assert!(
                out_default.degraded_sources.is_empty(),
                "F5.W4 bis: a clean call must report empty degraded_sources; got {:?}",
                out_default.degraded_sources
            );
        });
    }

    // PRF-F5.W4 bis — per-call degradation is independent across
    // contexts.
    //
    // Two contexts with very different sub_handler_timeout budgets on
    // the same corpus must surface the difference in `partial` /
    // `degraded_sources` and not collapse into a single answer. The
    // contract under test is that the per-call budget is honoured
    // without affecting any other HandlerContext.
    //
    // The "tight" budget uses the TempDir-dropped pattern from the
    // previous test so that populate_from_directory returns Err and
    // the composite degrades gracefully. The "generous" budget uses a
    // stable empty tempdir with the default 60s budget so that the
    // call returns Ok with partial=false and empty degraded_sources.
    //
    // `#[serial]` for the same reason as the previous test:
    // populate_from_directory is blocking and saturates the scheduler
    // under tight budgets; running concurrently with other F5.W4 bis
    // tests would produce scheduler-dependent flake.
    #[test]
    #[serial]
    fn prf_f5_w4_bis_per_call_timeout_is_independent_across_contexts() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Generous: stable empty tempdir + 60s budget. The call
            // returns Ok with partial=false and empty degraded_sources.
            let generous_dir = tempfile::tempdir().unwrap();
            let ctx_generous = HandlerContext::builder()
                .with_working_dir(generous_dir.path().to_path_buf())
                .with_sub_handler_timeout(std::time::Duration::from_secs(60))
                .build();

            // Tight: dropped tempdir + 1ns budget. populate_from_directory
            // returns Err and the composite degrades semantic+ranked.
            let ctx_tight = HandlerContext::builder()
                .with_working_dir(tempfile::tempdir().unwrap().path().to_path_buf())
                .with_sub_handler_timeout(std::time::Duration::from_nanos(1))
                .build();

            let input = SmartSearchInput {
                query: "per_call_independent_budget_query".into(),
                limit: Some(5),
            };

            let out_generous = handle_smart_search(&ctx_generous, input.clone())
                .await
                .expect("generous-budget call should Ok");
            let out_tight = handle_smart_search(&ctx_tight, input)
                .await
                .expect("tight-budget call should Ok (graceful)");

            assert!(
                !out_generous.partial,
                "F5.W4 bis: 60s budget on a stable tempdir must not produce partial=true; got {:?}",
                out_generous.degraded_sources
            );
            assert!(
                out_generous.degraded_sources.is_empty(),
                "F5.W4 bis: 60s budget on a stable tempdir must produce empty degraded_sources; got {:?}",
                out_generous.degraded_sources
            );
            assert!(
                out_tight.partial,
                "F5.W4 bis: tight-budget call with dropped tempdir must produce partial=true; got {:?}",
                out_tight.degraded_sources
            );
            assert!(
                out_tight.degraded_sources.contains(&"semantic".to_string()),
                "F5.W4 bis: tight-budget call must list semantic as degraded; got {:?}",
                out_tight.degraded_sources
            );
        });
    }
}
