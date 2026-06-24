//! Analyze stage — god nodes, surprising connections, dead code, hot paths
//! (ADR-017, Sprint 2).
//!
//! Runs after Cluster stage. Uses the in-memory CallGraph which now has
//! community assignments in `graph_nodes.properties.community`.
//!
//! Returns an `AnalysisSummary` containing all computed insights.

use serde::Serialize;

use crate::infrastructure::graph::graph_cache::GraphCache;

/// Summary of graph analysis results.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisSummary {
    pub god_nodes: Vec<GodNode>,
    pub surprising_connections: Vec<SurprisingConnection>,
    pub dead_code: Vec<String>,
    pub hot_paths: Vec<HotPath>,
    pub health_score: f64,
    pub symbol_count: usize,
    pub edge_count: usize,
    pub community_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct GodNode {
    pub symbol: String,
    pub pagerank: f64,
    pub fan_in: usize,
    pub fan_out: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SurprisingConnection {
    pub source: String,
    pub target: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotPath {
    pub symbol: String,
    pub fan_in: usize,
}

/// Run all analysis passes on the current in-memory graph.
/// Requires the cluster stage to have already run (for community data).
pub async fn run_analyze(cache: &GraphCache) -> AnalysisSummary {
    let graph = cache.get();
    let symbol_count = graph.symbol_count();
    let edge_count = graph.edge_count();

    if symbol_count == 0 {
        return AnalysisSummary {
            god_nodes: Vec::new(),
            surprising_connections: Vec::new(),
            dead_code: Vec::new(),
            hot_paths: Vec::new(),
            health_score: 100.0,
            symbol_count: 0,
            edge_count: 0,
            community_count: 0,
        };
    }

    // ── God Nodes (Top PageRank) ────────────────────────────────
    let god_nodes = compute_god_nodes(&graph, 10);

    // ── Hot Paths (Top fan-in) ──────────────────────────────────
    let hot_paths = compute_hot_paths(&graph, 10);

    // ── Dead Code ──────────────────────────────────────────────
    let dead_code: Vec<String> = graph
        .find_dead_code()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    // ── Health Score (0-100) ────────────────────────────────────
    let dead_ratio = if symbol_count > 0 {
        dead_code.len() as f64 / symbol_count as f64
    } else {
        0.0f64
    };
    let mut health: f64 = 100.0;
    health -= (dead_ratio * 50.0f64).min(30.0f64);
    if (symbol_count as f64) > 1000.0f64 {
        health -= 5.0f64;
    }

    let community_count = 0; // set by cluster stage

    AnalysisSummary {
        god_nodes,
        surprising_connections: Vec::new(),
        dead_code,
        hot_paths,
        health_score: health.max(0.0f64).min(100.0f64),
        symbol_count,
        edge_count,
        community_count,
    }
}

fn compute_god_nodes(graph: &crate::domain::aggregates::CallGraph, top_n: usize) -> Vec<GodNode> {
    let mut scored: Vec<(String, f64, usize, usize)> = graph
        .symbol_ids()
        .map(|(sid, _sym)| {
            let fan_in = graph.fan_in(sid);
            let fan_out = graph.fan_out(sid);
            // Simple PageRank approximation: fan-in weighted
            let pagerank = fan_in as f64 / (graph.symbol_count() as f64).max(1.0);
            (sid.as_str().to_string(), pagerank, fan_in, fan_out)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_n);

    scored
        .into_iter()
        .map(|(symbol, pagerank, fan_in, fan_out)| GodNode {
            symbol,
            pagerank,
            fan_in,
            fan_out,
        })
        .collect()
}

fn compute_hot_paths(graph: &crate::domain::aggregates::CallGraph, top_n: usize) -> Vec<HotPath> {
    let mut scored: Vec<(String, usize)> = graph
        .symbol_ids()
        .map(|(sid, _sym)| (sid.as_str().to_string(), graph.fan_in(sid)))
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(top_n);

    scored
        .into_iter()
        .map(|(symbol, fan_in)| HotPath { symbol, fan_in })
        .collect()
}
