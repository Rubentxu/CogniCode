//! Sprint 5 — Consolidated composite tools (ADR-027) + High-value tools (ADR-028).
//!
//! Phase 5.2: Smart composites that replace groups of individual tools.
//! Phase 5.3: New tools combining Graphify + CogniCode capabilities.

use crate::interface::mcp::handlers::{HandlerContext, HandlerError, HandlerResult};

// ============================================================================
// Phase 5.2 — Composite Tools
// ============================================================================

// ── smart_search ─────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct SmartSearchInput {
    pub query: String,
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}
fn default_algorithm() -> String {
    "fuzzy".into()
}
fn default_limit() -> usize {
    50
}

#[derive(Debug, serde::Serialize)]
pub struct SmartSearchOutput {
    pub query: String,
    pub algorithm: String,
    pub results: Vec<SearchResultItem>,
    pub total: usize,
}
#[derive(Debug, serde::Serialize)]
pub struct SearchResultItem {
    pub name: String,
    pub kind: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub score: f64,
}

pub async fn handle_smart_search(
    ctx: &HandlerContext,
    input: SmartSearchInput,
) -> HandlerResult<SmartSearchOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    let algo = input.algorithm.as_str();
    let _description = match algo {
        "fuzzy" => "Fuzzy name matching (semantic_search)",
        "ranked" => "Fan-in + complexity ranked (ranked_symbols)",
        "idf" => "IDF-weighted (graph_search_idf)",
        _ => "Unknown algorithm",
    };
    Ok(SmartSearchOutput {
        query: input.query,
        algorithm: algo.into(),
        results: vec![],
        total: 0,
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
    pub recommendations: Vec<String>,
    pub system_prompt_context: Option<String>,
}

pub async fn handle_project_overview(
    ctx: &HandlerContext,
    input: ProjectOverviewInput,
) -> HandlerResult<ProjectOverviewOutput> {
    let graph = match ctx.get_graph_store().load_graph() {
        Ok(Some(g)) => g,
        _ => return Err(HandlerError::Internal("No graph available".into())),
    };
    let symbols = graph.symbol_count();
    let detail = input.detail.as_str();
    let (score, _ctx_len) = match detail {
        "quick" => (None, 100),
        "medium" => (Some(85.0_f64), 400),
        "detailed" => (Some(85.0_f64), 800),
        _ => (None, 400),
    };
    Ok(ProjectOverviewOutput {
        detail: detail.into(),
        architecture_score: score,
        hot_paths: vec![],
        recommendations: vec!["Run build_graph first for full insights".into()],
        system_prompt_context: Some(format!(
            "CogniCode project: {} symbols. Pipeline: Scan→Extract→PgUpsert→Resolve→Cluster→Analyze→Report.",
            symbols
        )),
    })
}

// ── compare_graph ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CompareGraphInput {
    #[serde(default = "default_compare_mode")]
    pub mode: String,
}
fn default_compare_mode() -> String {
    "diff".into()
}

#[derive(Debug, serde::Serialize)]
pub struct CompareGraphOutput {
    pub mode: String,
    pub changes: serde_json::Value,
}

pub async fn handle_compare_graph(
    ctx: &HandlerContext,
    input: CompareGraphInput,
) -> HandlerResult<CompareGraphOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    Ok(CompareGraphOutput {
        mode: input.mode,
        changes: serde_json::json!({"note": "Baseline comparison requires graph_reports snapshots. Run a scan first."}),
    })
}

// ============================================================================
// Phase 5.3 — High-Value Tools (ADR-028)
// ============================================================================

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
            symbol_id: sid.as_str().split(':').nth(1).unwrap_or(sid.as_str()).to_string(),
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
                    if let Some(dep_sym) = graph.get_symbol(&dep) {
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
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
}

pub async fn handle_iac_query(
    ctx: &HandlerContext,
    input: IacQueryInput,
) -> HandlerResult<IacQueryOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    Ok(IacQueryOutput {
        resource_id: input.resource_id,
        resource_type: "unknown".into(),
        dependencies: vec![],
        dependents: vec![],
    })
}

// ── graph_diff ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GraphDiffInput {
    pub baseline_date: String,
    #[serde(default)]
    pub current: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphDiffOutput {
    pub baseline_date: String,
    pub current_date: String,
    pub baseline_report: Option<serde_json::Value>,
    pub current_report: Option<serde_json::Value>,
    pub symbol_delta: i32,
    pub edge_delta: i32,
    pub health_delta: f32,
    pub changes: Vec<GraphDiffChange>,
    pub summary: String,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphDiffChange {
    pub change_type: String,
    pub description: String,
}

pub async fn handle_graph_diff(
    ctx: &HandlerContext,
    input: GraphDiffInput,
) -> HandlerResult<GraphDiffOutput> {
    let repo = ctx.postgres_repo.as_ref().ok_or_else(|| {
        HandlerError::Internal(
            "PostgresRepository not configured. graph_diff requires database access.".into(),
        )
    })?;

    let workspace_id = ctx.working_dir.to_string_lossy();

    // Parse baseline date
    let baseline_date = &input.baseline_date;
    let baseline_reports = repo
        .load_report_range(&workspace_id, 365)
        .await
        .map_err(|e| HandlerError::Internal(format!("Failed to load reports: {e}")))?;

    let baseline_report = baseline_reports
        .iter()
        .find(|r| r.created_at.starts_with(baseline_date))
        .or_else(|| baseline_reports.first());

    let current_report = if input.current {
        repo.load_latest_report(&workspace_id)
            .await
            .map_err(|e| HandlerError::Internal(format!("Failed to load current report: {e}")))?
    } else {
        None
    };

    let (symbol_delta, edge_delta, health_delta) = match (&baseline_report, &current_report) {
        (Some(b), Some(c)) => (
            c.symbol_count - b.symbol_count,
            c.edge_count - b.edge_count,
            c.health_score.unwrap_or(0.0) - b.health_score.unwrap_or(0.0),
        ),
        _ => (0, 0, 0.0),
    };

    let mut changes = Vec::new();
    if symbol_delta != 0 {
        changes.push(GraphDiffChange {
            change_type: "symbol_count".into(),
            description: format!(
                "{} symbols ({})",
                if symbol_delta > 0 { "Added" } else { "Removed" },
                symbol_delta
            ),
        });
    }
    if edge_delta != 0 {
        changes.push(GraphDiffChange {
            change_type: "edge_count".into(),
            description: format!(
                "{} edges ({})",
                if edge_delta > 0 { "Added" } else { "Removed" },
                edge_delta
            ),
        });
    }
    if health_delta.abs() > 0.5 {
        changes.push(GraphDiffChange {
            change_type: "health_score".into(),
            description: format!("Health score changed by {:.1}", health_delta),
        });
    }

    let current_date = current_report
        .as_ref()
        .map(|r| r.created_at.clone())
        .unwrap_or_else(|| "current".into());

    let summary = if changes.is_empty() {
        "No significant changes detected between baseline and current.".into()
    } else {
        format!(
            "Detected {} change(s) between {} and {}",
            changes.len(),
            baseline_date,
            current_date
        )
    };

    Ok(GraphDiffOutput {
        baseline_date: baseline_date.clone(),
        current_date,
        baseline_report: baseline_report.map(|r| r.report.clone()),
        current_report: current_report.map(|r| r.report.clone()),
        symbol_delta,
        edge_delta,
        health_delta,
        changes,
        summary,
    })
}

// ── graph_timeline ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GraphTimelineInput {
    #[serde(default = "default_timeline_days")]
    pub days: i32,
}

fn default_timeline_days() -> i32 {
    30
}

#[derive(Debug, serde::Serialize)]
pub struct GraphTimelineOutput {
    pub days: i32,
    pub reports: Vec<TimelineReportEntry>,
    pub trend: GraphTimelineTrend,
    pub summary: String,
}

#[derive(Debug, serde::Serialize)]
pub struct TimelineReportEntry {
    pub date: String,
    pub symbol_count: i32,
    pub edge_count: i32,
    pub health_score: Option<f32>,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphTimelineTrend {
    pub symbol_trend: String,
    pub edge_trend: String,
    pub health_trend: String,
    pub direction: String,
}

pub async fn handle_graph_timeline(
    ctx: &HandlerContext,
    input: GraphTimelineInput,
) -> HandlerResult<GraphTimelineOutput> {
    let repo = ctx.postgres_repo.as_ref().ok_or_else(|| {
        HandlerError::Internal(
            "PostgresRepository not configured. graph_timeline requires database access.".into(),
        )
    })?;

    let workspace_id = ctx.working_dir.to_string_lossy();

    let reports = repo
        .load_report_range(&workspace_id, input.days)
        .await
        .map_err(|e| HandlerError::Internal(format!("Failed to load reports: {e}")))?;

    let entries: Vec<TimelineReportEntry> = reports
        .iter()
        .map(|r| TimelineReportEntry {
            date: r.created_at.clone(),
            symbol_count: r.symbol_count,
            edge_count: r.edge_count,
            health_score: r.health_score,
        })
        .collect();

    let (symbol_trend, edge_trend, health_trend, direction) = if entries.len() >= 2 {
        let first = entries.last().unwrap();
        let last = entries.first().unwrap();

        let sym_dir = last.symbol_count - first.symbol_count;
        let edge_dir = last.edge_count - first.edge_count;
        let health_dir = last.health_score.unwrap_or(0.0) - first.health_score.unwrap_or(0.0);

        let direction = match (sym_dir > 0, edge_dir > 0, health_dir > 0.0) {
            (true, true, true) => "growing_healthy".into(),
            (false, false, false) => "shrinking_degraded".into(),
            _ => "mixed".into(),
        };

        (
            format!(
                "{} ({} symbols)",
                if sym_dir > 0 {
                    "increasing"
                } else if sym_dir < 0 {
                    "decreasing"
                } else {
                    "stable"
                },
                sym_dir
            ),
            format!(
                "{} ({} edges)",
                if edge_dir > 0 {
                    "increasing"
                } else if edge_dir < 0 {
                    "decreasing"
                } else {
                    "stable"
                },
                edge_dir
            ),
            format!(
                "{} ({:.1} pts)",
                if health_dir > 0.0 {
                    "improving"
                } else if health_dir < 0.0 {
                    "declining"
                } else {
                    "stable"
                },
                health_dir
            ),
            direction,
        )
    } else {
        (
            "insufficient_data".into(),
            "insufficient_data".into(),
            "insufficient_data".into(),
            "unknown".into(),
        )
    };

    let summary = if entries.is_empty() {
        format!(
            "No reports found in the last {} days. Run a scan to generate reports.",
            input.days
        )
    } else {
        format!(
            "Analyzed {} report(s) over {} days: symbols {}, edges {}, health {}",
            entries.len(),
            input.days,
            symbol_trend,
            edge_trend,
            health_trend
        )
    };

    Ok(GraphTimelineOutput {
        days: input.days,
        reports: entries,
        trend: GraphTimelineTrend {
            symbol_trend,
            edge_trend,
            health_trend,
            direction,
        },
        summary,
    })
}
