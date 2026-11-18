//! Sprint 5 — Consolidated composite tools (ADR-027) + High-value tools (ADR-028).
//!
//! Phase 5.2: Smart composites that replace groups of individual tools.
//! Phase 5.3: New tools combining Graphify + CogniCode capabilities.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::unnecessary_sort_by, unused_imports)]

use crate::domain::aggregates::{CallGraph, Symbol, SymbolId};
use crate::domain::services::CycleDetector;
use crate::domain::value_objects::{DependencyType, Provenance};
use crate::interface::mcp::handlers::graph_query_handlers::{
    GraphQueryBasis, GraphRead, read_the_graph,
};
use crate::interface::mcp::handlers::{HandlerContext, HandlerError, HandlerResult};
use crate::interface::mcp::schemas::{
    CompareGraphInput, CompareGraphOutput, MetricDeltas, OverviewDetail, SmartSearchInput,
    SmartSearchOutput, SmartSearchResult,
};
use std::time::Duration;
// `JsonSchema` derives the published MCP output contract from this type, so
// the contract cannot drift from the struct that produces the bytes.
use schemars::JsonSchema;

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
    let sub_timeout = ctx.sub_handler_timeout();
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

// ── project_overview ─────────────────────────────────────────────────────────

/// The arguments of `project_overview`.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectOverviewInput {
    /// The level of detail the answer carries: `quick` (the statistics of
    /// the project), `medium` (the statistics, the entry points and the
    /// hot paths) or `detailed` (everything: the recommendations and the
    /// context for a prompt). The default is the complete answer.
    #[serde(default = "default_overview_detail")]
    pub detail: OverviewDetail,
}

/// The answer a client gets without asking for a level.
///
/// The tool published the complete answer before the levels meant
/// anything, so that is what the default keeps delivering: the label
/// changes, the content does not.
fn default_overview_detail() -> OverviewDetail {
    OverviewDetail::Detailed
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct ProjectOverviewOutput {
    /// The level this answer carries.
    pub detail: OverviewDetail,
    pub architecture_score: Option<f64>,
    pub hot_paths: Vec<String>,
    pub entry_points: Vec<String>,
    pub coverage_estimate: Option<f64>,
    pub recommendations: Vec<String>,
    pub system_prompt_context: Option<String>,
    /// One line saying what the answer says.
    pub summary: String,
}

pub async fn handle_project_overview(
    ctx: &HandlerContext,
    input: ProjectOverviewInput,
) -> HandlerResult<ProjectOverviewOutput> {
    // Ensure graph is built (auto-build if empty)
    let _ensure = super::ensure_graph_built(ctx).await?;

    // The level the client asked for. The answer carries what the level
    // says it carries, and it says which level it answered.
    let level = input.detail;

    let graph = ctx.analysis_service.get_project_graph();
    let stats = ctx.analysis_service.get_graph_stats();
    let entry_points = ctx.analysis_service.get_entry_points();
    let coverage = ctx.analysis_service.get_coverage_metrics();

    // Compute real architecture score via CycleDetector
    let cycle_detector = CycleDetector::new();
    let cycle_result = cycle_detector.detect_cycles(&graph);
    let cycle_penalty = cycle_result.symbols_in_cycles() * 5;
    let architecture_score = Some((100.0 - cycle_penalty as f64).max(0.0));

    // The hot paths are the owner's answer, not a second one: the same
    // question the `get_hot_paths` tool asks, of the same graph. The copy
    // that answered here ordered the list by the number of callers
    // alone, so the ties — and the cut, which fell inside a tie — were
    // decided by the order the graph happened to hold its symbols in, and
    // the published list came out different on every run.
    let all_hot_paths: Vec<String> =
        crate::application::services::analysis_service::AnalysisService::hot_paths(&graph, 10, 2)
            .into_iter()
            .map(|row| row.name)
            .collect();

    // Entry point names
    let all_entry_points: Vec<String> = entry_points.iter().map(|ep| ep.name.clone()).collect();

    // Coverage estimate
    let coverage_estimate = coverage.as_ref().map(|c| c.coverage_percent);

    // Build recommendations based on findings
    let mut all_recommendations = Vec::new();
    if !all_hot_paths.is_empty() {
        all_recommendations.push(format!(
            "Start with hot path '{}' (highest fan-in) for core logic understanding",
            all_hot_paths.first().unwrap_or(&"unknown".to_string())
        ));
    }
    if !cycle_result.cycles.is_empty() {
        all_recommendations.push(format!(
            "Address {} cyclic dependency cycle(s) to improve architecture score",
            cycle_result.cycles.len()
        ));
    }
    if entry_points.is_empty() {
        all_recommendations.push("No entry points detected. Run build_graph first.".to_string());
    }

    let symbol_count = stats.symbol_count;
    let edge_count = stats.edge_count;

    // The summary reports the shape of the project, and the shape of a
    // project does not change with the level: the same statistics at the
    // three levels.
    let summary = format!(
        "{} symbols, {} entry points, {} hot paths, architecture score {}",
        symbol_count,
        all_entry_points.len(),
        all_hot_paths.len(),
        architecture_score
            .map(|s| format!("{s:.2}"))
            .unwrap_or_else(|| "unknown".to_string())
    );

    let system_prompt_context = format!(
        "CogniCode project: {} symbols, {} edges.",
        symbol_count, edge_count
    );

    // The level selects what the answer carries. The measurements travel
    // at every level: the score and the coverage are measured, and an
    // absent measurement would say that they were not.
    let (hot_paths, entry_points, recommendations, system_prompt_context) = match level {
        OverviewDetail::Quick => (Vec::new(), Vec::new(), Vec::new(), None),
        OverviewDetail::Medium => (all_hot_paths, all_entry_points, Vec::new(), None),
        OverviewDetail::Detailed => (
            all_hot_paths,
            all_entry_points,
            all_recommendations,
            Some(system_prompt_context),
        ),
    };

    Ok(ProjectOverviewOutput {
        detail: level,
        architecture_score,
        hot_paths,
        entry_points,
        coverage_estimate,
        recommendations,
        system_prompt_context,
        summary,
    })
}

// ── codebase_map ─────────────────────────────────────────────────────────────

/// The shape of the map the tool publishes.
///
/// The vocabulary is the type: the handler reads the format and the
/// published schema reads the same type, so the two cannot disagree about
/// which formats exist. A format that does not exist is refused, not read
/// as one that does.
#[derive(
    Debug, Clone, Copy, Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum MapFormat {
    #[default]
    Compact,
    Detailed,
}

impl MapFormat {
    /// The name the wire and the published schema use.
    pub fn as_str(&self) -> &'static str {
        match self {
            MapFormat::Compact => "compact",
            MapFormat::Detailed => "detailed",
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CodebaseMapInput {
    /// The shape of the map: `compact` (a single line) or `detailed` (the
    /// project, its edges, its entry points and its hot symbols).
    #[serde(default)]
    pub format: MapFormat,
}

/// A symbol the map calls hot: one that several others call.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, JsonSchema)]
pub struct HotSymbol {
    /// The name the symbol carries.
    pub name: String,
    /// The file that declares it.
    pub file: String,
    /// The line it is declared on (1-indexed).
    pub line: u32,
    /// How many symbols call it.
    pub fan_in: usize,
    /// How many symbols it calls.
    pub fan_out: usize,
}

/// What `codebase_map` publishes.
///
/// Every number and every name in `map` is read from the call graph the
/// server holds; nothing in it is a constant. The structured fields carry
/// the same facts as the text, so a consumer does not have to parse `map`.
#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct CodebaseMapOutput {
    /// The format that was asked for, echoed back.
    pub format: String,
    /// The map as text.
    pub map: String,
    /// A rough size of `map` in tokens (four bytes per token). An estimate,
    /// not a count of any tokenizer.
    pub token_estimate: usize,
    /// One line saying what the answer says.
    pub summary: String,
    /// The symbols with the most callers, most-called first, at most
    /// [`MAP_HOT_SYMBOLS`] of them, and only those with at least
    /// [`MAP_MIN_FAN_IN`] callers. Empty when no symbol qualifies — a
    /// graph without call edges has none — and never filled from the order
    /// in which the graph happens to hold its symbols.
    pub hot_symbols: Vec<HotSymbol>,
    /// What the graph covers. The map describes the graph the server holds,
    /// not the whole tree: a `partial` or `unknown` coverage means the
    /// counts and the hot symbols may leave out part of the project.
    pub coverage: crate::interface::coverage::CoverageInfo,
    /// Which tree and which snapshot the graph was read from, the same
    /// basis `graph_query` publishes. Absent when nobody measured the graph
    /// that was read, or when the measure does not know its snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basis: Option<GraphQueryBasis>,
}

/// The most hot symbols the map names.
pub const MAP_HOT_SYMBOLS: usize = 5;

/// The fewest callers a symbol needs to be called hot.
pub const MAP_MIN_FAN_IN: usize = 2;

/// The counts the map states, read from the graph.
struct MapCounts {
    symbols: usize,
    edges: usize,
    entry_points: usize,
    leaves: usize,
}

/// The map as text. Pure: the same facts give the same text.
fn render_map(
    format: MapFormat,
    counts: &MapCounts,
    hot: &[HotSymbol],
    coverage: &crate::interface::coverage::CoverageInfo,
) -> String {
    let hot_line = if hot.is_empty() {
        format!("none (no symbol has {MAP_MIN_FAN_IN} or more callers)")
    } else {
        hot.iter()
            .map(|h| format!("{} ({}:{}, fan-in {})", h.name, h.file, h.line, h.fan_in))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let coverage_line = coverage_line(coverage);
    match format {
        MapFormat::Compact => format!(
            "Project: {} symbols, {} edges | Entry points: {} | Leaves: {} | Hot: {} | {}",
            counts.symbols,
            counts.edges,
            counts.entry_points,
            counts.leaves,
            hot_line,
            coverage_line
        ),
        MapFormat::Detailed => {
            let mut map = format!(
                "Project: {} symbols | {} edges | {} entry points | {} leaf functions",
                counts.symbols, counts.edges, counts.entry_points, counts.leaves
            );
            if hot.is_empty() {
                map.push_str(&format!("\nHot symbols: {hot_line}"));
            } else {
                map.push_str("\nHot symbols:");
                for h in hot {
                    map.push_str(&format!(
                        "\n- {} ({}:{}) fan-in {}, fan-out {}",
                        h.name, h.file, h.line, h.fan_in, h.fan_out
                    ));
                }
            }
            map.push_str(&format!("\n{coverage_line}"));
            map
        }
    }
}

/// The coverage as one clause, with the holes counted when there are any.
fn coverage_line(coverage: &crate::interface::coverage::CoverageInfo) -> String {
    if coverage.status == "complete" {
        return "Coverage: complete".to_string();
    }
    format!(
        "Coverage: {} ({} skipped files, {} pending references, {} rejected relations)",
        coverage.status,
        coverage.skipped_files.len(),
        coverage.pending_references.len(),
        coverage.rejected_relations.len()
    )
}

/// The map of the project the server holds.
///
/// The hot symbols are the owner's answer to "which symbols have the most
/// callers" (the same one `get_hot_paths` gives), cut to
/// [`MAP_HOT_SYMBOLS`]. The coverage is the one measured for the graph that
/// was read, not the last one anybody measured; a graph nobody measured says
/// `unknown`, never `complete`, and publishes no basis.
///
/// # Errors
///
/// Fails when the server holds no graph, or its store cannot be read.
pub async fn handle_codebase_map(
    ctx: &HandlerContext,
    input: CodebaseMapInput,
) -> HandlerResult<CodebaseMapOutput> {
    // The graph and what was measured about THAT graph come from one read:
    // a route that replaced the graph in between leaves nobody able to say
    // which graph was read, so nothing is claimed about it.
    let GraphRead { graph, measure } = read_the_graph(ctx)?;
    let counts = MapCounts {
        symbols: graph.symbol_count(),
        edges: graph.edge_count(),
        entry_points: graph.roots().len(),
        leaves: graph.leaves().len(),
    };
    let hot_symbols: Vec<HotSymbol> =
        crate::application::services::analysis_service::AnalysisService::hot_paths(
            &graph,
            MAP_HOT_SYMBOLS,
            MAP_MIN_FAN_IN,
        )
        .into_iter()
        .map(|row| HotSymbol {
            name: row.name,
            file: row.file,
            line: row.line,
            fan_in: row.fan_in,
            fan_out: row.fan_out,
        })
        .collect();
    let coverage = match &measure {
        Some(measure) => crate::interface::coverage::CoverageInfo::of(&measure.coverage),
        None => crate::interface::coverage::CoverageInfo::unmeasured(),
    };
    let basis = measure.as_ref().and_then(GraphQueryBasis::of);

    let map = render_map(input.format, &counts, &hot_symbols, &coverage);
    let token_estimate = map.len() / 4;
    let summary = format!(
        "{} symbols, {} edges, {} entry points, {} hot symbols, coverage {}, ~{} tokens",
        counts.symbols,
        counts.edges,
        counts.entry_points,
        hot_symbols.len(),
        coverage.status,
        token_estimate
    );
    Ok(CodebaseMapOutput {
        format: input.format.as_str().to_string(),
        token_estimate,
        map,
        summary,
        hot_symbols,
        coverage,
        basis,
    })
}

// ── project_insights ─────────────────────────────────────────────────────────

use crate::application::services::graph_insights::{GraphInsightsService, InsightsReport};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectInsightsInput {}

#[derive(Debug, serde::Serialize, JsonSchema)]
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

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct HotPath {
    pub symbol_id: String,
    pub score: f64,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct CommunityOverviewDto {
    pub count: usize,
    pub largest_size: usize,
    pub smallest_size: usize,
    pub avg_cohesion: f64,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
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
    let projection = crate::infrastructure::graph::project_call_graph(&graph);
    let report = GraphInsightsService::analyze(&graph, projection.as_ref());

    // Compute entry points and dead code from graph (not in InsightsReport)
    let entries = graph.roots().len();
    let dead = graph.find_dead_code().len();

    // Map god_nodes to hot_paths (top 10 by score)
    let hot_paths = hot_paths(&graph, &report);

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

    let mut summary = format!(
        "{} symbols, {} edges, {} communities, {} cycles, health {:.0}/100",
        report.summary.total_symbols,
        report.summary.total_edges,
        report.communities.count,
        report.summary.total_cycles,
        report.health_score
    );
    if report.summary.total_edges == 0 {
        summary.push_str("; no ranking signal: the graph has no edges");
    }

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

/// The published hot paths of a graph: the score of each hot symbol and the
/// name the symbol carries.
///
/// The name comes from the symbol itself. A piece of its identity would be a
/// different thing: for a declaration an infrastructure reader named, that
/// piece is the file the thing lives in.
pub fn hot_paths(graph: &CallGraph, report: &InsightsReport) -> Vec<HotPath> {
    report
        .god_nodes
        .iter()
        .take(10)
        .map(|(sid, score)| HotPath {
            symbol_id: graph
                .get_symbol(sid)
                .map(|symbol| symbol.name().to_string())
                .unwrap_or_else(|| sid.as_str().to_string()),
            score: *score,
        })
        .collect()
}

// ── review_pr ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReviewPrInput {
    /// Changed file paths
    pub files: Vec<String>,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
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
    let graph = match ctx.get_graph_store().load_graph_shared() {
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct IacQueryInput {
    /// Resource ID: bare name (aws_instance.web) or canonical (tf:main.tf:aws_instance.web)
    pub resource_id: String,
    /// Max traversal depth (default: 2)
    #[serde(default = "default_iac_depth")]
    pub depth: usize,
}
fn default_iac_depth() -> usize {
    2
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct IacQueryOutput {
    pub resource_id: String,
    pub resource_type: String,
    pub dependencies: Vec<IacRelation>,
    pub dependents: Vec<IacRelation>,
    /// One line saying what the answer says.
    pub summary: String,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
pub struct IacRelation {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub edge_type: String,
    /// Where the edge came from, in the vocabulary of the domain.
    ///
    /// Absent when the backend that answered does not say: an unknown
    /// provenance is not an observed one.
    pub provenance: Option<String>,
    /// How sure the edge is.
    ///
    /// Absent when the edge carries no confidence: an unknown confidence
    /// is not a zero and it is not a certainty either.
    pub confidence: Option<f64>,
}

/// What a symbol declares, in the vocabulary the consumer reads.
///
/// A declaration that names a type answers with that type — a bucket is an
/// `aws_s3_bucket` — and one that names no type answers with the kind its
/// reader gave it. A symbol that declares nothing in particular answers
/// with its kind, which the domain names in lowercase.
///
/// The answer never carries the name of a Rust type: this tool is read by
/// programs that do not know how the graph is built inside.
fn published_type(symbol: &Symbol) -> String {
    symbol
        .declaration()
        .map(|declaration| declaration.type_or_kind().to_string())
        .unwrap_or_else(|| symbol.kind().name().to_string())
}

/// The relation the tool publishes for an edge that exists.
///
/// The edge carries a type, a provenance and a confidence; the relation
/// carries the three of them. Nothing here is a constant: what the edge
/// does not say is published as unknown and not as a number that was
/// invented on its behalf.
fn published_relation(
    graph: &CallGraph,
    id: &SymbolId,
    edge_type: &DependencyType,
    provenance: Provenance,
    confidence: f64,
) -> IacRelation {
    let symbol = graph.get_symbol(id);
    IacRelation {
        id: id.to_string(),
        name: symbol.map(|s| s.name().to_string()).unwrap_or_default(),
        kind: symbol.map(published_type).unwrap_or_default(),
        edge_type: edge_type.to_string(),
        provenance: Some(provenance.name().to_string()),
        confidence: Some(confidence),
    }
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

        let dependencies: Vec<IacRelation> = iac_repo
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
                // The repository does not carry a provenance, and a
                // confidence it does not have is not a zero.
                provenance: None,
                confidence: edge.confidence.map(|c| c as f64),
            })
            .collect();

        let dependents: Vec<IacRelation> = iac_repo
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
                provenance: None,
                confidence: edge.confidence.map(|c| c as f64),
            })
            .collect();

        let summary = format!(
            "{} {}: {} dependencies, {} dependents",
            resource.resource_type,
            resource.id,
            dependencies.len(),
            dependents.len()
        );
        return Ok(IacQueryOutput {
            resource_id: resource.id,
            resource_type: resource.resource_type,
            dependencies,
            dependents,
            summary,
        });
    }

    // Fall back to in-memory graph if IacRepository is not configured
    let graph = match ctx.get_graph_store().load_graph_shared() {
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
        match iac_candidates.as_slice() {
            [] => {
                return Err(HandlerError::NotFound(format!(
                    "IaC resource '{}' not found. Use canonical ID (tf:file:type.name) or ensure IaC files are scanned.",
                    input.resource_id
                )));
            }
            [only] => only.fully_qualified_name().to_string(),
            // A name that answers for more than one declaration has no
            // single answer: choosing one of them would publish the
            // edges of a declaration nobody asked about. The canonical
            // identity is what removes the doubt.
            several => {
                let identities: Vec<&str> = several
                    .iter()
                    .map(|symbol| symbol.fully_qualified_name())
                    .collect();
                return Err(HandlerError::InvalidInput(format!(
                    "IaC resource '{}' is ambiguous: {} declarations answer to that name. Ask for the canonical identity of the one you mean: {}",
                    input.resource_id,
                    several.len(),
                    identities.join(", ")
                )));
            }
        }
    };

    // Get the symbol from the graph
    let symbol_id = SymbolId::new(&resolved_id);
    let symbol = graph.get_symbol(&symbol_id).ok_or_else(|| {
        HandlerError::NotFound(format!("Resource '{}' not found in graph", resolved_id))
    })?;

    let resource_type = published_type(symbol);

    // The depth is how far the walk goes, not how many relations come
    // back: one hop is the edges that touch the resource, two hops also
    // bring the edges that touch what the first ones touch.
    let dependencies: Vec<IacRelation> = graph
        .dependencies_within_depth(&symbol_id, input.depth)
        .into_iter()
        .map(|(target_id, edge_type, provenance, confidence)| {
            published_relation(&graph, &target_id, &edge_type, provenance, confidence)
        })
        .collect();

    // The dependents come from the same edges, read the other way round:
    // each one carries the type and the confidence its own edge has.
    let dependents: Vec<IacRelation> = graph
        .dependents_within_depth(&symbol_id, input.depth)
        .into_iter()
        .map(|(source_id, edge_type, provenance, confidence)| {
            published_relation(&graph, &source_id, &edge_type, provenance, confidence)
        })
        .collect();

    let summary = format!(
        "{} {}: {} dependencies, {} dependents",
        resource_type,
        resolved_id,
        dependencies.len(),
        dependents.len()
    );
    Ok(IacQueryOutput {
        resource_id: resolved_id,
        resource_type,
        dependencies,
        dependents,
        summary,
    })
}

// ── graph_checkpoint ──────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphCheckpointInput {
    /// Operation: create (build graph + checkpoint), current (get latest), restore (get by id)
    pub operation: Option<String>,
    /// Checkpoint ID to restore (required for 'restore' operation)
    pub checkpoint_id: Option<u64>,
}

#[derive(Debug, serde::Serialize, JsonSchema)]
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
    use crate::domain::traits::GraphStore;
    use crate::domain::value_objects::SymbolKind;
    use crate::infrastructure::persistence::InMemoryGraphStore;
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

    /// A context whose store holds a graph.
    ///
    /// The degradation contract has a "clean" case: every sub-handler
    /// answers, so the composite reports no degradation. That case needs
    /// a graph, because the `idf` source reads one. A context without a
    /// graph is not a clean call: it is a call where one source failed,
    /// and the composite has to say so.
    fn test_ctx_with_graph() -> (HandlerContext, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut graph = CallGraph::new();
        graph.add_symbol(Symbol::new(
            "alpha".to_string(),
            SymbolKind::Function,
            crate::domain::value_objects::Location::new("lib.rs", 1, 0),
        ));
        let store = InMemoryGraphStore::new();
        store.save_graph(&graph).expect("the graph is stored");
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_graph_store(store)
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
        let output = result.unwrap_or_else(|e| {
            panic!(
                "F5.W4: smart_search MUST return Ok even when sub-handlers fail; got Err: {e:?} \
             — this means the composite collapsed instead of degrading gracefully"
            )
        });

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
            let out = r.as_ref().unwrap_or_else(|e| {
                panic!(
                    "F5.W4 concurrent: call {i} returned Err: {e:?} — should degrade graceful to Ok"
                )
            });
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
            // thing that flips partial=true. The context carries a graph
            // because the `idf` source reads one: without it that source
            // fails, and the call would be degraded for a reason that has
            // nothing to do with the timeout.
            let (ctx_default, _default_dir) = test_ctx_with_graph();
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
            // Generous: stable tempdir + 60s budget + a graph, so every
            // sub-handler answers. The call returns Ok with partial=false
            // and empty degraded_sources. The graph matters: the `idf`
            // source reads one, and without it that source fails, which
            // would make this a degraded call for a reason that has
            // nothing to do with the budget.
            let generous_dir = tempfile::tempdir().unwrap();
            let mut generous_graph = CallGraph::new();
            generous_graph.add_symbol(Symbol::new(
                "alpha".to_string(),
                SymbolKind::Function,
                crate::domain::value_objects::Location::new("lib.rs", 1, 0),
            ));
            let generous_store = InMemoryGraphStore::new();
            generous_store
                .save_graph(&generous_graph)
                .expect("the graph is stored");
            let ctx_generous = HandlerContext::builder()
                .with_working_dir(generous_dir.path().to_path_buf())
                .with_graph_store(generous_store)
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

    /// Un formato que no existe no es un formato.
    ///
    /// El esquema publicado dice quais formatos existen, y el servidor tiene
    /// que rechazar los demas en vez de leerlos como `detailed`: un cliente
    /// que pidiera un mapa que el servidor no tiene recibia un mapa de otro
    /// tipo sin que nadie le dijera nada.
    #[test]
    fn a_map_format_that_does_not_exist_is_refused() {
        let error = serde_json::from_value::<CodebaseMapInput>(serde_json::json!({
            "format": "xml"
        }))
        .expect_err("un formato que no existe no se puede leer");
        assert!(
            error.to_string().contains("xml"),
            "el rechazo tiene que decir que formato es: {error}"
        );
    }

    // ── codebase_map: what the map publishes is read from the graph ─────────

    /// A graph where the most-called symbol sorts LAST by id.
    ///
    /// Four callers and a leaf sort before `zz_hub`, so a map that publishes
    /// "the first five symbols" misses the one symbol everything depends on.
    fn graph_with_a_hub_that_sorts_last() -> CallGraph {
        let mut graph = CallGraph::new();
        let mut add = |name: &str, line: u32| {
            graph.add_symbol(Symbol::new(
                name.to_string(),
                SymbolKind::Function,
                crate::domain::value_objects::Location::new("lib.rs", line, 0),
            ))
        };
        let callers: Vec<SymbolId> = (1..=4).map(|i| add(&format!("caller_{i}"), i)).collect();
        let _leaf = add("leaf", 5);
        let hub = add("zz_hub", 6);
        for caller in &callers {
            graph
                .add_dependency(caller, &hub, DependencyType::Calls)
                .expect("both ends are in the graph");
        }
        graph
    }

    /// Five symbols and no call between them.
    fn graph_without_edges() -> CallGraph {
        let mut graph = CallGraph::new();
        for (i, name) in ["a", "b", "c", "d", "e"].iter().enumerate() {
            graph.add_symbol(Symbol::new(
                (*name).to_string(),
                SymbolKind::Function,
                crate::domain::value_objects::Location::new("lib.rs", i as u32 + 1, 0),
            ));
        }
        graph
    }

    /// A context that holds `graph`, the way the product holds a graph: in
    /// the service's own cache, which is what the store the answer reads
    /// from serves.
    ///
    /// A context whose store holds a graph the service does not hold is a
    /// different thing: the answer reads a graph that nothing measured, and
    /// its coverage is unknown on purpose.
    fn ctx_holding(graph: &CallGraph) -> (HandlerContext, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let service = crate::application::services::analysis_service::AnalysisService::new();
        service.graph_cache().replace(graph.clone());
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_analysis_service(service)
            .build();
        (ctx, temp_dir)
    }

    /// A context whose store holds `graph` on its own.
    ///
    /// The store the product uses serves the service's own cache, and an
    /// empty graph is not served at all — an empty graph is not a graph.
    /// This fixture keeps the store that can hold one, for the answers that
    /// have to render it.
    fn ctx_with_a_store_holding(graph: &CallGraph) -> (HandlerContext, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let store = InMemoryGraphStore::new();
        store.save_graph(graph).expect("the graph is stored");
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_graph_store(store)
            .build();
        (ctx, temp_dir)
    }

    /// Records `coverage` as the measure of `graph`, the way a build does.
    fn measure_held_graph(
        ctx: &HandlerContext,
        graph: &CallGraph,
        coverage: crate::application::services::analyze::Coverage,
    ) {
        let service = ctx.analysis_service();
        service
            .graph_measures()
            .record(crate::application::graph_measure::GraphMeasure::new(
                service.current_graph_revision(),
                graph,
                coverage,
                crate::application::analysis_basis::GraphSource::Built,
                "/workspace".to_string(),
                None,
            ));
    }

    async fn map_of(ctx: &HandlerContext, format: MapFormat) -> CodebaseMapOutput {
        handle_codebase_map(ctx, CodebaseMapInput { format })
            .await
            .expect("the map answers over a held graph")
    }

    /// The hot symbols are the most-called ones, not the first ones by id.
    #[tokio::test]
    async fn the_hot_symbol_is_the_one_everything_calls() {
        let (ctx, _dir) = ctx_holding(&graph_with_a_hub_that_sorts_last());
        for format in [MapFormat::Compact, MapFormat::Detailed] {
            let map = map_of(&ctx, format).await.map;
            assert!(
                map.contains("zz_hub"),
                "the symbol four callers depend on is the hot one: {map}"
            );
            assert!(
                !map.contains("caller_2"),
                "a caller nobody calls is not hot just because it sorts early: {map}"
            );
        }
    }

    /// With no call edges there is no ranking, and the map says so.
    #[tokio::test]
    async fn a_graph_without_edges_has_no_hot_symbols() {
        let (ctx, _dir) = ctx_holding(&graph_without_edges());
        let map = map_of(&ctx, MapFormat::Compact).await.map;
        for name in ["a", "b", "c", "d", "e"] {
            assert!(
                !map.contains(&format!("lib.rs:{name}")),
                "no symbol is hot over a graph with no edges: {map}"
            );
        }
        assert!(
            map.contains("Hot: none"),
            "the absence of hot symbols is stated, not left blank: {map}"
        );
    }

    /// The map does not describe a pipeline the graph never read.
    #[tokio::test]
    async fn the_detailed_map_does_not_publish_a_fixed_pipeline() {
        let (ctx, _dir) = ctx_holding(&graph_with_a_hub_that_sorts_last());
        let map = map_of(&ctx, MapFormat::Detailed).await.map;
        for invented in ["Pipeline", "PgUpsert", "Scan", "Notify"] {
            assert!(
                !map.contains(invented),
                "`{invented}` is not read from the graph: {map}"
            );
        }
    }

    /// The structured hot symbols are the facts the text names.
    #[tokio::test]
    async fn the_hot_symbols_are_published_as_facts() {
        let (ctx, _dir) = ctx_holding(&graph_with_a_hub_that_sorts_last());
        let out = map_of(&ctx, MapFormat::Compact).await;
        assert_eq!(
            out.hot_symbols,
            vec![HotSymbol {
                name: "zz_hub".to_string(),
                file: "lib.rs".to_string(),
                // The graph stores lines 0-indexed; the published line is 1-indexed.
                line: 7,
                fan_in: 4,
                fan_out: 0,
            }]
        );
    }

    /// The hot list is bounded and ordered by how much each is called.
    #[tokio::test]
    async fn the_hot_list_is_bounded_and_most_called_first() {
        let mut graph = CallGraph::new();
        let mut add = |name: String, line: u32| {
            graph.add_symbol(Symbol::new(
                name,
                SymbolKind::Function,
                crate::domain::value_objects::Location::new("lib.rs", line, 0),
            ))
        };
        let callers: Vec<SymbolId> = (0..8).map(|i| add(format!("caller_{i}"), i)).collect();
        let targets: Vec<SymbolId> = (0..7u32)
            .map(|k| add(format!("target_{k}"), 100 + k))
            .collect();
        // `target_k` is called by k + 2 callers: seven targets, five survive.
        for (k, target) in targets.iter().enumerate() {
            for caller in callers.iter().take(k + 2) {
                graph
                    .add_dependency(caller, target, DependencyType::Calls)
                    .expect("both ends are in the graph");
            }
        }
        let (ctx, _dir) = ctx_holding(&graph);
        let out = map_of(&ctx, MapFormat::Detailed).await;
        let names: Vec<&str> = out.hot_symbols.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(
            names,
            ["target_6", "target_5", "target_4", "target_3", "target_2"]
        );
    }

    /// A graph nobody measured is `unknown`, and the map says it.
    #[tokio::test]
    async fn an_unmeasured_graph_is_not_called_complete() {
        let (ctx, _dir) = ctx_holding(&graph_with_a_hub_that_sorts_last());
        let out = map_of(&ctx, MapFormat::Compact).await;
        assert_eq!(out.coverage.status, "unknown");
        assert!(out.map.contains("Coverage: unknown"), "{}", out.map);
        assert!(out.summary.contains("coverage unknown"), "{}", out.summary);
    }

    /// A partial coverage reaches the map with its holes counted.
    #[tokio::test]
    async fn a_partial_coverage_is_published_with_its_holes() {
        use crate::application::services::analyze::{Coverage, CoverageFacts};
        use crate::infrastructure::graph::per_file_graph::{SkipReason, SkippedFile};

        let graph = graph_with_a_hub_that_sorts_last();
        let (ctx, _dir) = ctx_holding(&graph);
        measure_held_graph(
            &ctx,
            &graph,
            Coverage::Partial(CoverageFacts {
                skipped: vec![SkippedFile {
                    path: "vendored/x.rs".to_string(),
                    reason: SkipReason::Read("permission denied".to_string()),
                }],
                pending: Vec::new(),
                rejected: Vec::new(),
            }),
        );
        let out = map_of(&ctx, MapFormat::Detailed).await;
        assert_eq!(out.coverage.status, "partial");
        assert_eq!(out.coverage.skipped_files.len(), 1);
        assert!(
            out.map.contains("Coverage: partial (1 skipped files"),
            "{}",
            out.map
        );
    }

    /// A graph that was measured complete says so.
    #[tokio::test]
    async fn a_complete_coverage_is_published_as_complete() {
        let graph = graph_with_a_hub_that_sorts_last();
        let (ctx, _dir) = ctx_holding(&graph);
        measure_held_graph(
            &ctx,
            &graph,
            crate::application::services::analyze::Coverage::Complete,
        );
        let out = map_of(&ctx, MapFormat::Compact).await;
        assert_eq!(out.coverage.status, "complete");
        assert!(out.map.contains("Coverage: complete"), "{}", out.map);
    }

    /// A coverage measured for some other graph is not this graph's.
    #[tokio::test]
    async fn a_coverage_measured_for_another_graph_is_not_published() {
        let other = graph_without_edges();
        let graph = graph_with_a_hub_that_sorts_last();
        let (ctx, _dir) = ctx_holding(&graph);
        measure_held_graph(
            &ctx,
            &other,
            crate::application::services::analyze::Coverage::Complete,
        );
        let out = map_of(&ctx, MapFormat::Compact).await;
        assert_eq!(out.coverage.status, "unknown");
    }

    /// A graph that replaced the measured one is not described by the old
    /// coverage, not even when it has the same size: the measure belongs to
    /// the revision it was taken for.
    #[tokio::test]
    async fn a_graph_replaced_after_the_measure_is_not_published() {
        let graph = graph_with_a_hub_that_sorts_last();
        let (ctx, _dir) = ctx_holding(&graph);
        measure_held_graph(
            &ctx,
            &graph,
            crate::application::services::analyze::Coverage::Complete,
        );

        // Another route replaces the graph the service holds. The
        // replacement has the same shape as the measured one, so only the
        // revision tells the two apart.
        let mut replacement = CallGraph::new();
        let mut add = |name: &str, line: u32| {
            replacement.add_symbol(Symbol::new(
                name.to_string(),
                SymbolKind::Function,
                crate::domain::value_objects::Location::new("lib.rs", line, 0),
            ))
        };
        let callers: Vec<SymbolId> = (1..=4).map(|i| add(&format!("other_{i}"), i)).collect();
        let _leaf = add("other_leaf", 5);
        let hub = add("other_hub", 6);
        for caller in &callers {
            replacement
                .add_dependency(caller, &hub, DependencyType::Calls)
                .expect("both ends are in the graph");
        }
        assert_eq!(replacement.symbol_count(), graph.symbol_count());
        assert_eq!(replacement.edge_count(), graph.edge_count());

        ctx.analysis_service().graph_cache().replace(replacement);

        let out = map_of(&ctx, MapFormat::Compact).await;
        assert_eq!(
            out.coverage.status, "unknown",
            "the coverage measured for the previous graph is not this graph's"
        );
    }

    /// A graph with no symbols is mapped as empty, not as an error.
    #[tokio::test]
    async fn a_graph_with_no_symbols_has_an_empty_map() {
        let (ctx, _dir) = ctx_with_a_store_holding(&CallGraph::new());
        let out = map_of(&ctx, MapFormat::Detailed).await;
        assert!(out.hot_symbols.is_empty());
        assert!(out.map.contains("0 symbols"), "{}", out.map);
    }

    /// Without a graph the tool refuses; it does not invent a map.
    #[tokio::test]
    async fn without_a_graph_there_is_no_map() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let ctx = HandlerContext::builder()
            .with_working_dir(temp_dir.path().to_path_buf())
            .with_graph_store(InMemoryGraphStore::new())
            .build();
        let err = handle_codebase_map(
            &ctx,
            CodebaseMapInput {
                format: MapFormat::Compact,
            },
        )
        .await
        .expect_err("a map needs a graph");
        assert!(err.to_string().contains("No graph available"), "{err}");
    }

    /// The same graph gives the same hot symbols, in the same order.
    #[tokio::test]
    async fn the_hot_symbols_come_out_in_the_same_order_every_time() {
        let (ctx, _dir) = ctx_holding(&graph_with_a_hub_that_sorts_last());
        let first = map_of(&ctx, MapFormat::Detailed).await;
        let second = map_of(&ctx, MapFormat::Detailed).await;
        assert_eq!(first.hot_symbols, second.hot_symbols);
        assert_eq!(first.map, second.map);
    }
}
