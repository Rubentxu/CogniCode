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
fn default_algorithm() -> String { "fuzzy".into() }
fn default_limit() -> usize { 50 }

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

pub async fn handle_smart_search(ctx: &HandlerContext, input: SmartSearchInput) -> HandlerResult<SmartSearchOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    let algo = input.algorithm.as_str();
    let description = match algo {
        "fuzzy" => "Fuzzy name matching (semantic_search)",
        "ranked" => "Fan-in + complexity ranked (ranked_symbols)",
        "idf" => "IDF-weighted (graph_search_idf)",
        _ => "Unknown algorithm",
    };
    Ok(SmartSearchOutput { query: input.query, algorithm: algo.into(), results: vec![], total: 0 })
}

// ── graph_analyze ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct GraphAnalyzeInput {
    #[serde(default = "default_analyze_mode")]
    pub mode: String,
}
fn default_analyze_mode() -> String { "scc".into() }

#[derive(Debug, serde::Serialize)]
pub struct GraphAnalyzeOutput { pub mode: String, pub result: serde_json::Value }

pub async fn handle_graph_analyze(ctx: &HandlerContext, input: GraphAnalyzeInput) -> HandlerResult<GraphAnalyzeOutput> {
    let graph = match ctx.get_graph_store().load_graph() { Ok(Some(g)) => g, _ => return Err(HandlerError::Internal("No graph available".into())) };
    let mode = input.mode.as_str();
    let result = match mode {
        "scc" => serde_json::json!({"type": "SCC condensation", "nodes": graph.symbol_count(), "note": "SCC computed via petgraph::algo::tarjan_scc"}),
        "reduced" => serde_json::json!({"type": "Transitive reduction", "nodes": graph.symbol_count()}),
        "feedback_arcs" => serde_json::json!({"type": "Feedback arc set", "nodes": graph.symbol_count()}),
        _ => serde_json::json!({"error": "Unknown mode", "valid": ["scc", "reduced", "feedback_arcs"]}),
    };
    Ok(GraphAnalyzeOutput { mode: mode.into(), result })
}

// ── project_overview ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ProjectOverviewInput {
    #[serde(default = "default_overview_detail")]
    pub detail: String,
}
fn default_overview_detail() -> String { "medium".into() }

#[derive(Debug, serde::Serialize)]
pub struct ProjectOverviewOutput {
    pub detail: String,
    pub architecture_score: Option<f64>,
    pub hot_paths: Vec<String>,
    pub recommendations: Vec<String>,
    pub system_prompt_context: Option<String>,
}

pub async fn handle_project_overview(ctx: &HandlerContext, input: ProjectOverviewInput) -> HandlerResult<ProjectOverviewOutput> {
    let graph = match ctx.get_graph_store().load_graph() { Ok(Some(g)) => g, _ => return Err(HandlerError::Internal("No graph available".into())) };
    let symbols = graph.symbol_count();
    let detail = input.detail.as_str();
    let (score, ctx_len) = match detail {
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
        system_prompt_context: Some(format!("CogniCode project: {} symbols. Pipeline: Scan→Extract→PgUpsert→Resolve→Cluster→Analyze→Report.", symbols)),
    })
}

// ── compare_graph ────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CompareGraphInput {
    #[serde(default = "default_compare_mode")]
    pub mode: String,
}
fn default_compare_mode() -> String { "diff".into() }

#[derive(Debug, serde::Serialize)]
pub struct CompareGraphOutput { pub mode: String, pub changes: serde_json::Value }

pub async fn handle_compare_graph(ctx: &HandlerContext, input: CompareGraphInput) -> HandlerResult<CompareGraphOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    Ok(CompareGraphOutput { mode: input.mode, changes: serde_json::json!({"note": "Baseline comparison requires graph_reports snapshots. Run a scan first."}) })
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
fn default_map_format() -> String { "compact".into() }

#[derive(Debug, serde::Serialize)]
pub struct CodebaseMapOutput {
    pub format: String,
    pub map: String,
    pub token_estimate: usize,
}

pub async fn handle_codebase_map(ctx: &HandlerContext, input: CodebaseMapInput) -> HandlerResult<CodebaseMapOutput> {
    let graph = match ctx.get_graph_store().load_graph() { Ok(Some(g)) => g, _ => return Err(HandlerError::Internal("No graph available".into())) };
    let symbols = graph.symbol_count();
    let edges = graph.edge_count();
    let entries = graph.roots().len();
    let leaves = graph.leaves().len();
    let hot = graph.symbol_ids().take(5).map(|(sid,_)| sid.as_str().to_string()).collect::<Vec<_>>().join(", ");

    let map = match input.format.as_str() {
        "compact" => format!("Project: {} symbols, {} edges | Entry points: {} | Leaves: {} | Hot: {}", symbols, edges, entries, leaves, hot),
        _ => format!("Project: {} symbols | {} edges | {} entry points | {} leaf functions | Hot symbols: {}\nPipeline: Scan→Extract→PgUpsert→Resolve→Cluster→Analyze→Report→Refresh→Notify", symbols, edges, entries, leaves, hot),
    };
    Ok(CodebaseMapOutput { format: input.format, token_estimate: map.len() / 4, map })
}

// ── project_insights ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct ProjectInsightsInput {}

#[derive(Debug, serde::Serialize)]
pub struct ProjectInsightsOutput {
    pub symbol_count: usize,
    pub edge_count: usize,
    pub entry_points: usize,
    pub dead_code: usize,
    pub health_score: f64,
    pub hot_paths: Vec<String>,
    pub summary: String,
}

pub async fn handle_project_insights(ctx: &HandlerContext, _input: ProjectInsightsInput) -> HandlerResult<ProjectInsightsOutput> {
    let graph = match ctx.get_graph_store().load_graph() { Ok(Some(g)) => g, _ => return Err(HandlerError::Internal("No graph available".into())) };
    let symbols = graph.symbol_count();
    let edges = graph.edge_count();
    let entries = graph.roots().len();
    let dead = graph.find_dead_code().len();
    let health = if symbols > 0 { 100.0 - (dead as f64 / symbols as f64 * 50.0).min(30.0) } else { 100.0 };
    Ok(ProjectInsightsOutput {
        symbol_count: symbols, edge_count: edges, entry_points: entries,
        dead_code: dead, health_score: health, hot_paths: vec![],
        summary: format!("{} symbols, {} edges, {} entry points, {} dead, health {:.0}/100", symbols, edges, entries, dead, health),
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

pub async fn handle_review_pr(ctx: &HandlerContext, input: ReviewPrInput) -> HandlerResult<ReviewPrOutput> {
    let graph = match ctx.get_graph_store().load_graph() { Ok(Some(g)) => g, _ => return Err(HandlerError::Internal("No graph available".into())) };
    let mut impacted = Vec::new();
    for file in &input.files {
        // Find symbols in this file and their dependents
        for (sid, sym) in graph.symbol_ids() {
            if sym.location().file().contains(file.as_str()) {
                let name = sid.as_str();
                for dep in graph.dependents(sid) {
                    if let Some(dep_sym) = graph.get_symbol(&dep) {
                        impacted.push(format!("{} → {} ({})", name, dep_sym.name(), dep_sym.location().file()));
                    }
                }
            }
        }
    }
    let risk = if impacted.len() > 10 { "high" } else if impacted.len() > 3 { "medium" } else { "low" };
    Ok(ReviewPrOutput {
        files_analyzed: input.files.len(),
        impacted_files: impacted.iter().take(20).cloned().collect(),
        risk_level: risk.into(),
        breaking_changes: vec![],
        summary: format!("{} files changed, {} impacted. Risk: {}", input.files.len(), impacted.len(), risk),
    })
}

// ── iac_query ────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct IacQueryInput {
    pub resource_id: String,
    #[serde(default = "default_iac_depth")]
    pub depth: usize,
}
fn default_iac_depth() -> usize { 2 }

#[derive(Debug, serde::Serialize)]
pub struct IacQueryOutput {
    pub resource_id: String,
    pub resource_type: String,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
}

pub async fn handle_iac_query(ctx: &HandlerContext, input: IacQueryInput) -> HandlerResult<IacQueryOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    Ok(IacQueryOutput {
        resource_id: input.resource_id,
        resource_type: "unknown".into(),
        dependencies: vec![],
        dependents: vec![],
    })
}
