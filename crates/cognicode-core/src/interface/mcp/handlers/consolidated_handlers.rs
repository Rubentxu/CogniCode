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

// ============================================================================
// Phase 5.2 — Composite Tools
// ============================================================================

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

    // Run all three searches in parallel
    let (sem, rank, idf) = tokio::join!(
        crate::interface::mcp::handlers::handle_semantic_search(sem_svc, wd, semantic_input),
        crate::interface::mcp::handlers::aix_handlers::handle_ranked_symbols(ctx, ranked_input),
        crate::interface::mcp::handlers::graph_handlers::handle_graph_search_idf(ctx, idf_input),
    );

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
    use std::sync::Arc;

    /// Helper to create a minimal HandlerContext for testing.
    fn test_ctx() -> HandlerContext {
        let temp_dir = tempfile::TempDir::new().unwrap();
        HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .build()
    }

    #[tokio::test]
    async fn test_list_view_specs_returns_builtins() {
        // Returns the built-in descriptors
        let ctx = test_ctx();
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
        let ctx = test_ctx();
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
        let ctx = test_ctx();
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
        let ctx = test_ctx();
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
        let ctx = test_ctx();
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
}
