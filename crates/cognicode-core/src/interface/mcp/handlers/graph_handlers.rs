//! Graph Analytics Handlers — Inline graph tool handlers extracted from rmcp_adapter.rs
//!
//! This module contains all 12 graph analytics tool handlers:
//! - Phase 4b: graph_pagerank, graph_all_paths, graph_condensed, graph_god_nodes,
//!             graph_reduced, graph_feedback_arcs
//! - Phase 5: graph_communities, graph_community_detail, graph_surprising_connections
//! - Phase 6: graph_search_idf, graph_insights, graph_suggest_questions
//!
//! Each handler:
//!   1. Pulls the in-memory `CallGraph` from the context's `GraphStore`
//!   2. Runs the appropriate graph analytics algorithm
//!   3. Returns the result as JSON

use super::*;

// ============================================================================
// Phase 4b: Graph Analytics Handlers (PageRank, paths, condensation, god nodes)
// ============================================================================

/// Handler for graph_pagerank tool — Compute PageRank importance scores for all symbols.
#[cognicode_macros::aix_tool(
    name = "graph_pagerank",
    description = "Compute PageRank importance scores for all symbols in the call graph. Returns a ranked list of symbols by dependency importance. High-scoring symbols are 'god nodes' (heavily depended-upon). Requires build_graph first.",
    input_schema = GraphPageRankInput
)]
pub async fn handle_graph_pagerank(
    ctx: &HandlerContext,
    input: GraphPageRankInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let mut scores = crate::application::services::graph_analytics::GraphAnalyticsService::page_rank(
                &graph,
                input.alpha,
                input.max_iterations as usize,
            );
            // Sort descending by score so the most important symbols come first.
            let mut sorted: Vec<(String, f64)> = scores
                .drain()
                .map(|(id, score)| (id.as_str().to_string(), score))
                .collect();
            sorted.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            // Truncate to a reasonable cap (200) so the JSON payload does not blow up.
            sorted.truncate(200);
            Ok(serde_json::json!({
                "algorithm": "page_rank",
                "alpha": input.alpha,
                "max_iterations": input.max_iterations,
                "symbol_count": sorted.len(),
                "scores": sorted,
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_all_paths tool — Find all simple paths between two symbols.
#[cognicode_macros::aix_tool(
    name = "graph_all_paths",
    description = "Find all simple paths between two symbols in the call graph (no repeated nodes). Useful for enumerating every call chain that connects two functions. Requires build_graph first.",
    input_schema = GraphAllPathsInput
)]
pub async fn handle_graph_all_paths(
    ctx: &HandlerContext,
    input: GraphAllPathsInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            // Substring, case-insensitive lookup with tiered matching:
            // Tier 1: exact symbol name, Tier 2: exact FQN, Tier 3: substring.
            let find_id = |needle: &str| -> Option<crate::domain::aggregates::SymbolId> {
                let needle_lower = needle.to_lowercase();
                // Tier 1: exact symbol name (case-insensitive).
                for (id, sym) in graph.symbol_ids() {
                    if sym.name().to_lowercase() == needle_lower {
                        return Some(id.clone());
                    }
                }
                // Tier 2: exact FQN match.
                for (id, sym) in graph.symbol_ids() {
                    if sym.fully_qualified_name().to_lowercase() == needle_lower {
                        return Some(id.clone());
                    }
                }
                // Tier 3: substring match against FQN.
                for (id, sym) in graph.symbol_ids() {
                    if sym
                        .fully_qualified_name()
                        .to_lowercase()
                        .contains(&needle_lower)
                    {
                        return Some(id.clone());
                    }
                }
                None
            };

            match (find_id(&input.from_symbol), find_id(&input.to_symbol)) {
                (Some(from), Some(to)) => {
                    let paths = crate::application::services::graph_analytics::GraphAnalyticsService::all_simple_paths(
                        &graph,
                        &from,
                        &to,
                        input.max_hops as usize,
                    );
                    // Render paths as lists of FQNs for human-readable output.
                    let rendered: Vec<Vec<String>> = paths
                        .into_iter()
                        .map(|path| {
                            path.into_iter()
                                .map(|id| id.as_str().to_string())
                                .collect()
                        })
                        .collect();
                    Ok(serde_json::json!({
                        "from": from.as_str(),
                        "to": to.as_str(),
                        "max_hops": input.max_hops,
                        "path_count": rendered.len(),
                        "paths": rendered,
                    }))
                }
                (None, _) => Ok(serde_json::json!({
                    "error": format!("Source symbol not found: {}", input.from_symbol)
                })),
                (_, None) => Ok(serde_json::json!({
                    "error": format!("Target symbol not found: {}", input.to_symbol)
                })),
            }
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_condensed tool — Compute SCC condensation of the call graph.
#[cognicode_macros::aix_tool(
    name = "graph_condensed",
    description = "Compute the SCC condensation of the call graph: every strongly connected component is collapsed into a single node, producing an acyclic condensation DAG. Use to spot circular dependency clusters. Requires build_graph first.",
    input_schema = GraphCondensedInput
)]
pub async fn handle_graph_condensed(
    ctx: &HandlerContext,
    _input: GraphCondensedInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let comps = crate::application::services::graph_analytics::GraphAnalyticsService::condensation(&graph);
            let rendered: Vec<Vec<String>> = comps
                .into_iter()
                .map(|c| c.into_iter().map(|id| id.as_str().to_string()).collect())
                .collect();
            let multi: Vec<&Vec<String>> = rendered.iter().filter(|c| c.len() > 1).collect();
            let singletons = rendered.iter().filter(|c| c.len() == 1).count();
            Ok(serde_json::json!({
                "algorithm": "condensation",
                "component_count": rendered.len(),
                "nontrivial_components": multi.len(),
                "singleton_components": singletons,
                "components": rendered,
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_god_nodes tool — Find symbols with unusually high PageRank.
#[cognicode_macros::aix_tool(
    name = "graph_god_nodes",
    description = "Find god nodes — symbols with unusually high PageRank (above the supplied percentile). These are symbols that too many things depend on and are prime refactoring candidates. Requires build_graph first.",
    input_schema = GraphGodNodesInput
)]
pub async fn handle_graph_god_nodes(
    ctx: &HandlerContext,
    input: GraphGodNodesInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let mut god = crate::application::services::graph_analytics::GraphAnalyticsService::god_nodes(
                &graph,
                input.percentile,
            );
            let rendered: Vec<(String, f64)> = god
                .drain(..)
                .map(|(id, score)| (id.as_str().to_string(), score))
                .collect();
            Ok(serde_json::json!({
                "algorithm": "god_nodes",
                "percentile": input.percentile,
                "count": rendered.len(),
                "nodes": rendered,
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_reduced tool — Compute the transitive reduction of the call graph.
#[cognicode_macros::aix_tool(
    name = "graph_reduced",
    description = "Compute the transitive reduction of the call graph — the minimal set of dependency edges that preserves reachability. Redundant edges (implied by longer paths) are dropped. Requires build_graph first.",
    input_schema = GraphReducedInput
)]
pub async fn handle_graph_reduced(
    ctx: &HandlerContext,
    _input: GraphReducedInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let reduced = crate::application::services::graph_analytics::GraphAnalyticsService::transitive_reduction(&graph);
            let rendered: Vec<(String, String)> = reduced
                .into_iter()
                .map(|(s, d)| (s.as_str().to_string(), d.as_str().to_string()))
                .collect();
            let total_edges = graph.edge_count();
            Ok(serde_json::json!({
                "algorithm": "transitive_reduction",
                "original_edge_count": total_edges,
                "reduced_edge_count": rendered.len(),
                "edges": rendered,
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_feedback_arcs tool — Find edges whose removal makes the graph acyclic.
#[cognicode_macros::aix_tool(
    name = "graph_feedback_arcs",
    description = "Find a feedback arc set — edges whose removal would make the call graph acyclic. The greedy heuristic is not optimal but fast; use the result as a starting point when breaking circular dependencies. Requires build_graph first.",
    input_schema = GraphFeedbackArcsInput
)]
pub async fn handle_graph_feedback_arcs(
    ctx: &HandlerContext,
    _input: GraphFeedbackArcsInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let fas = crate::application::services::graph_analytics::GraphAnalyticsService::feedback_arc_set(&graph);
            let rendered: Vec<(String, String)> = fas
                .into_iter()
                .map(|(s, d)| (s.as_str().to_string(), d.as_str().to_string()))
                .collect();
            Ok(serde_json::json!({
                "algorithm": "feedback_arc_set",
                "count": rendered.len(),
                "edges": rendered,
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

// ============================================================================
// Phase 5: Community Detection Handlers
// ============================================================================

/// Handler for graph_communities tool — Detect code communities using Label Propagation.
#[cognicode_macros::aix_tool(
    name = "graph_communities",
    description = "Detect code communities using Label Propagation. Groups symbols that are tightly coupled into clusters. Returns communities with cohesion scores. Requires build_graph first.",
    input_schema = GraphCommunitiesInput
)]
pub async fn handle_graph_communities(
    ctx: &HandlerContext,
    input: GraphCommunitiesInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let result = crate::application::services::community_detector::CommunityDetector::detect(
                &graph,
                input.max_iterations as usize,
            );
            let payload = serde_json::json!({
                "algorithm": "label_propagation",
                "max_iterations": input.max_iterations,
                "iterations_used": result.iterations,
                "converged": result.converged,
                "community_count": result.communities.len(),
                "communities": result.communities.iter().map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "label": c.label,
                        "size": c.nodes.len(),
                        "internal_edges": c.internal_edges,
                        "external_edges": c.external_edges,
                        "cohesion": c.cohesion,
                    })
                }).collect::<Vec<_>>(),
            });
            Ok(payload)
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_community_detail tool — Get details for a specific community.
#[cognicode_macros::aix_tool(
    name = "graph_community_detail",
    description = "Get details for a specific community detected by graph_communities (members, internal/external edge counts, cohesion score, and top god nodes within the community). Requires build_graph first.",
    input_schema = GraphCommunityDetailInput
)]
pub async fn handle_graph_community_detail(
    ctx: &HandlerContext,
    input: GraphCommunityDetailInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let result = crate::application::services::community_detector::CommunityDetector::detect(
                &graph,
                input.max_iterations as usize,
            );
            match result.communities.iter().find(|c| c.id == input.community_id) {
                Some(c) => {
                    // Top god nodes within this community.
                    let god = crate::application::services::community_detector::CommunityDetector::community_god_nodes(
                        &graph,
                        std::slice::from_ref(c),
                        5,
                    );
                    let god_list = god.into_iter().flat_map(|(_id, scored)| scored).collect::<Vec<_>>();
                    let god_rendered: Vec<(String, f64)> = god_list
                        .into_iter()
                        .map(|(id, score)| (id.as_str().to_string(), score))
                        .collect();
                    let nodes_rendered: Vec<String> = c
                        .nodes
                        .iter()
                        .map(|n| n.as_str().to_string())
                        .collect();
                    let payload = serde_json::json!({
                        "community_id": c.id,
                        "label": c.label,
                        "size": c.nodes.len(),
                        "internal_edges": c.internal_edges,
                        "external_edges": c.external_edges,
                        "cohesion": c.cohesion,
                        "nodes": nodes_rendered,
                        "god_nodes": god_rendered,
                    });
                    Ok(payload)
                }
                None => Ok(serde_json::json!({
                    "error": format!(
                        "Community {} not found. Available ids: {:?}",
                        input.community_id,
                        result.communities.iter().map(|c| c.id).collect::<Vec<_>>()
                    )
                })),
            }
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_surprising_connections tool — Find cross-community edges.
#[cognicode_macros::aix_tool(
    name = "graph_surprising_connections",
    description = "Find surprising cross-community connections. These are edges between symbols in different communities, indicating unexpected coupling. Requires build_graph first.",
    input_schema = GraphSurprisingConnectionsInput
)]
pub async fn handle_graph_surprising_connections(
    ctx: &HandlerContext,
    input: GraphSurprisingConnectionsInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let result = crate::application::services::community_detector::CommunityDetector::detect(
                &graph,
                input.max_iterations as usize,
            );
            let top_n = if input.top_n == 0 { 20 } else { input.top_n as usize };
            let crosses = crate::application::services::community_detector::CommunityDetector::surprising_connections(
                &graph,
                &result,
                top_n,
            );
            let rendered: Vec<serde_json::Value> = crosses
                .into_iter()
                .map(|(s, d, sc, dc)| {
                    serde_json::json!({
                        "source": s.as_str(),
                        "target": d.as_str(),
                        "source_community": sc,
                        "target_community": dc,
                    })
                })
                .collect();
            let payload = serde_json::json!({
                "algorithm": "label_propagation_surprising",
                "max_iterations": input.max_iterations,
                "community_count": result.communities.len(),
                "cross_community_edge_count": rendered.len(),
                "edges": rendered,
            });
            Ok(payload)
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

// ============================================================================
// Phase 6: IDF-weighted Search & Unified Insights
// ============================================================================

/// Handler for graph_search_idf tool — IDF-weighted symbol search.
#[cognicode_macros::aix_tool(
    name = "graph_search_idf",
    description = "Search symbols ranked by IDF (Inverse Document Frequency) importance. Rare terms score higher. Includes hub bypass for cleaner results. Requires build_graph first.",
    input_schema = GraphSearchIdfInput
)]
pub async fn handle_graph_search_idf(
    ctx: &HandlerContext,
    input: GraphSearchIdfInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let max_results = if input.max_results == 0 {
                20
            } else {
                input.max_results as usize
            };
            let results = crate::application::services::search_ranker::SearchRanker::search(
                &graph,
                &input.query,
                max_results,
            );
            let payload = serde_json::json!({
                "algorithm": "idf_search",
                "query": input.query,
                "max_results": max_results,
                "result_count": results.len(),
                "results": results.iter().map(|r| {
                    serde_json::json!({
                        "symbol_id": r.symbol_id.as_str(),
                        "name": r.name,
                        "file": r.file,
                        "idf_score": r.idf_score,
                        "degree": r.degree,
                    })
                }).collect::<Vec<_>>(),
            });
            Ok(payload)
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_insights tool — Unified architecture health report.
#[cognicode_macros::aix_tool(
    name = "graph_insights",
    description = "Get a complete architecture health report: god nodes, circular dependencies, community overview, surprising cross-module connections, and a health score (0-100). Requires build_graph first.",
    input_schema = GraphInsightsInput
)]
pub async fn handle_graph_insights(
    ctx: &HandlerContext,
    _input: GraphInsightsInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let report = crate::application::services::graph_insights::GraphInsightsService::analyze(&graph);
            let payload = serde_json::json!({
                "summary": {
                    "total_symbols": report.summary.total_symbols,
                    "total_edges": report.summary.total_edges,
                    "total_communities": report.summary.total_communities,
                    "total_cycles": report.summary.total_cycles,
                    "symbols_in_cycles": report.summary.symbols_in_cycles,
                },
                "god_nodes": report.god_nodes.iter().map(|(id, s)| {
                    serde_json::json!({ "symbol_id": id.as_str(), "score": s })
                }).collect::<Vec<_>>(),
                "cycle_clusters": report.cycle_clusters.iter().map(|cluster| {
                    cluster.iter().map(|id| id.as_str()).collect::<Vec<_>>()
                }).collect::<Vec<_>>(),
                "cycle_breakers": report.cycle_breakers.iter().map(|(s, d)| {
                    serde_json::json!({ "source": s.as_str(), "target": d.as_str() })
                }).collect::<Vec<_>>(),
                "communities": {
                    "count": report.communities.count,
                    "largest_size": report.communities.largest_size,
                    "smallest_size": report.communities.smallest_size,
                    "avg_cohesion": report.communities.avg_cohesion,
                    "top_communities": report.communities.top_communities.iter().map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "label": c.label,
                            "size": c.size,
                            "cohesion": c.cohesion,
                        })
                    }).collect::<Vec<_>>(),
                },
                "surprising_connections": report.surprising_connections.iter().map(|c| {
                    serde_json::json!({
                        "source": c.source.as_str(),
                        "target": c.target.as_str(),
                        "source_community": c.source_community,
                        "target_community": c.target_community,
                    })
                }).collect::<Vec<_>>(),
                "health_score": report.health_score,
                "suggested_questions": report.suggested_questions,
            });
            Ok(payload)
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}

/// Handler for graph_suggest_questions tool — Natural-language prompts for refactoring.
#[cognicode_macros::aix_tool(
    name = "graph_suggest_questions",
    description = "Generate intelligent questions about the codebase architecture based on graph analysis. Helps identify areas that need attention. Requires build_graph first.",
    input_schema = GraphSuggestQuestionsInput
)]
pub async fn handle_graph_suggest_questions(
    ctx: &HandlerContext,
    _input: GraphSuggestQuestionsInput,
) -> HandlerResult<serde_json::Value> {
    match ctx.get_graph_store().load_graph() {
        Ok(Some(graph)) => {
            let report = crate::application::services::graph_insights::GraphInsightsService::analyze(&graph);
            let payload = serde_json::json!({
                "question_count": report.suggested_questions.len(),
                "questions": report.suggested_questions,
            });
            Ok(payload)
        }
        Ok(None) => Ok(serde_json::json!({
            "error": "No call graph available. Run build_graph first."
        })),
        Err(e) => Ok(serde_json::json!({
            "error": format!("Graph store error: {}", e)
        })),
    }
}
