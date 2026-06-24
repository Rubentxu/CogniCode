//! Unified graph insights — consolidates all analytics into one report.
//!
//! Combines: god nodes, cycles, community overview, surprising connections,
//! architecture health, and suggested questions.

use crate::application::services::community_detector::CommunityDetector;
use crate::application::services::graph_analytics::GraphAnalyticsService;
use crate::domain::aggregates::CallGraph;
use crate::domain::aggregates::call_graph::SymbolId;
use crate::infrastructure::graph::CallGraphProjection;

/// A complete graph insights report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InsightsReport {
    /// Summary statistics.
    pub summary: GraphSummary,
    /// God nodes — symbols with unusually high importance.
    pub god_nodes: Vec<(SymbolId, f64)>,
    /// Cycle clusters (from SCC condensation).
    pub cycle_clusters: Vec<Vec<SymbolId>>,
    /// Edges whose removal breaks cycles.
    pub cycle_breakers: Vec<(SymbolId, SymbolId)>,
    /// Community overview.
    pub communities: CommunityOverview,
    /// Cross-community connections (surprising coupling).
    pub surprising_connections: Vec<SurprisingConnection>,
    /// Architecture health score (0-100).
    pub health_score: f64,
    /// Suggested questions for the agent/user to explore.
    pub suggested_questions: Vec<String>,
}

/// Top-level summary of the call graph state.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GraphSummary {
    pub total_symbols: usize,
    pub total_edges: usize,
    pub total_communities: usize,
    pub total_cycles: usize,
    pub symbols_in_cycles: usize,
}

/// Roll-up of community detection results.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommunityOverview {
    pub count: usize,
    pub largest_size: usize,
    pub smallest_size: usize,
    pub avg_cohesion: f64,
    pub top_communities: Vec<CommunitySummary>,
}

/// Summary of a single community (label, size, cohesion).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommunitySummary {
    pub id: u32,
    pub label: String,
    pub size: usize,
    pub cohesion: f64,
}

/// An edge that crosses community boundaries.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SurprisingConnection {
    pub source: SymbolId,
    pub target: SymbolId,
    pub source_community: u32,
    pub target_community: u32,
}

/// Generate a complete insights report for a call graph.
pub struct GraphInsightsService;

impl GraphInsightsService {
    /// Generate a unified insights report from a `CallGraph`.
    ///
    /// The report combines:
    /// - **Summary stats** — node/edge counts and cycle counts.
    /// - **God nodes** — the top 10 PageRank symbols (p95 threshold).
    /// - **Cycle clusters** — strongly-connected components of size > 1.
    /// - **Cycle breakers** — first 10 edges in the feedback arc set.
    /// - **Communities** — top 5 communities by size with cohesion scores.
    /// - **Surprising connections** — first 20 cross-community edges.
    /// - **Health score** — composite 0-100 number combining cycle,
    ///   god-node, cohesion, and cross-community penalties.
    /// - **Suggested questions** — natural-language prompts that an
    ///   AI agent can use to drive a refactoring conversation.
    ///
    /// The algorithm is total: an empty graph returns a `health_score`
    /// of `100.0` (a vacuum has perfect architecture) and an empty
    /// `suggested_questions` list.
    pub fn analyze(graph: &CallGraph) -> InsightsReport {
        let projection = CallGraphProjection::from_call_graph(graph);

        // Summary
        let summary = GraphSummary {
            total_symbols: projection.node_count(),
            total_edges: projection.edge_count(),
            total_communities: 0, // filled below
            total_cycles: 0,      // filled below
            symbols_in_cycles: 0,
        };

        // God nodes (top 10, p95)
        let god_nodes = GraphAnalyticsService::god_nodes(graph, 0.95);
        let god_nodes_top: Vec<_> = god_nodes.into_iter().take(10).collect();

        // Cycles (SCCs of size > 1 are non-trivial cycles)
        let sccs = projection.strongly_connected_components();
        let cycle_clusters: Vec<Vec<SymbolId>> =
            sccs.into_iter().filter(|scc| scc.len() > 1).collect();
        let total_cycles = cycle_clusters.len();
        let symbols_in_cycles: usize = cycle_clusters.iter().map(|c| c.len()).sum();

        // Cycle breakers
        let cycle_breakers = GraphAnalyticsService::feedback_arc_set(graph);
        let cycle_breakers_top: Vec<_> = cycle_breakers.into_iter().take(10).collect();

        // Communities
        let community_result = CommunityDetector::detect(graph, 100);
        let communities = &community_result.communities;

        let community_overview = CommunityOverview {
            count: communities.len(),
            largest_size: communities.first().map(|c| c.nodes.len()).unwrap_or(0),
            smallest_size: communities.last().map(|c| c.nodes.len()).unwrap_or(0),
            avg_cohesion: if communities.is_empty() {
                0.0
            } else {
                communities.iter().map(|c| c.cohesion).sum::<f64>() / communities.len() as f64
            },
            top_communities: communities
                .iter()
                .take(5)
                .map(|c| CommunitySummary {
                    id: c.id,
                    label: c.label.clone(),
                    size: c.nodes.len(),
                    cohesion: c.cohesion,
                })
                .collect(),
        };

        // Surprising connections
        let cross = CommunityDetector::surprising_connections(graph, &community_result, 20);
        let surprising_connections: Vec<SurprisingConnection> = cross
            .into_iter()
            .take(20)
            .map(|(src, dst, sc, dc)| SurprisingConnection {
                source: src,
                target: dst,
                source_community: sc,
                target_community: dc,
            })
            .collect();

        // Health score: 100 minus four penalty components.
        //   - cycle_penalty: 5 points per cycle, capped at 30.
        //   - god_penalty: 10 if there are 4+ god nodes (concentration risk).
        //   - cohesion_penalty: up to 20 points for poor cohesion
        //     (avg_cohesion below 1.0 means cross-community edges exist).
        //     Only applies when at least one community exists — an
        //     empty graph (or a graph with isolated singletons only)
        //     gets the full 100 score because there is nothing to
        //     measure.
        //   - cross_penalty: 0.5 per cross-community edge, capped at 20.
        // The final value is clamped to [0, 100].
        let cycle_penalty = (total_cycles as f64 * 5.0).min(30.0);
        let god_penalty = if god_nodes_top.len() > 3 { 10.0 } else { 0.0 };
        let cohesion_penalty = if community_overview.count > 0 {
            (1.0 - community_overview.avg_cohesion) * 20.0
        } else {
            0.0
        };
        let cross_penalty = (surprising_connections.len() as f64 * 0.5).min(20.0);
        let health_score =
            (100.0 - cycle_penalty - god_penalty - cohesion_penalty - cross_penalty).max(0.0);

        // Suggested questions
        let mut suggested_questions = Vec::new();

        if total_cycles > 0 {
            suggested_questions.push(format!(
                "There are {} circular dependency clusters involving {} symbols. Should we break these cycles?",
                total_cycles, symbols_in_cycles
            ));
        }
        if !god_nodes_top.is_empty() {
            let top_god = &god_nodes_top[0];
            suggested_questions.push(format!(
                "'{}' is the most depended-upon symbol (score: {:.3}). Is it doing too much?",
                top_god
                    .0
                    .as_str()
                    .split(':')
                    .nth(1)
                    .unwrap_or(top_god.0.as_str()),
                top_god.1
            ));
        }
        if !surprising_connections.is_empty() {
            suggested_questions.push(format!(
                "Found {} cross-community connections. These might indicate unexpected coupling between modules.",
                surprising_connections.len()
            ));
        }
        if community_overview.avg_cohesion < 0.5 && community_overview.count > 0 {
            suggested_questions.push(
                "Average community cohesion is below 50%. Consider restructuring modules to reduce coupling."
                    .to_string(),
            );
        }
        if communities.len() == 1 && summary.total_symbols > 10 {
            suggested_questions.push(
                "All symbols are in a single community. The codebase may need module boundaries."
                    .to_string(),
            );
        }

        InsightsReport {
            summary: GraphSummary {
                total_communities: communities.len(),
                total_cycles,
                symbols_in_cycles,
                ..summary
            },
            god_nodes: god_nodes_top,
            cycle_clusters,
            cycle_breakers: cycle_breakers_top,
            communities: community_overview,
            surprising_connections,
            health_score,
            suggested_questions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::Symbol;
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    #[test]
    fn test_empty_graph_insights() {
        let graph = CallGraph::new();
        let report = GraphInsightsService::analyze(&graph);
        assert_eq!(report.summary.total_symbols, 0);
        assert_eq!(report.summary.total_edges, 0);
        assert_eq!(report.summary.total_cycles, 0);
        assert_eq!(report.health_score, 100.0); // Empty graph is "healthy"
        assert!(report.god_nodes.is_empty());
        assert!(report.cycle_breakers.is_empty());
        assert_eq!(report.communities.count, 0);
    }

    #[test]
    fn test_simple_graph_has_insights() {
        let mut graph = CallGraph::new();
        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("m1.rs", 1, 1),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("m2.rs", 1, 1),
        ));
        let _ = graph.add_dependency(&a, &b, DependencyType::Calls);

        let report = GraphInsightsService::analyze(&graph);
        assert_eq!(report.summary.total_symbols, 2);
        assert_eq!(report.summary.total_edges, 1);
        assert_eq!(report.cycle_clusters.len(), 0); // No cycles in a->b
        // Either a question about god nodes should be present, or the
        // health score should reflect a healthy graph (no cycles).
        assert!(
            !report.suggested_questions.is_empty() || report.health_score >= 50.0,
            "Expected either suggestions or a healthy score, got questions={:?} score={}",
            report.suggested_questions,
            report.health_score
        );
    }

    #[test]
    fn test_cycle_detected_in_insights() {
        let mut graph = CallGraph::new();
        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("m1.rs", 1, 1),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("m2.rs", 1, 1),
        ));
        let _ = graph.add_dependency(&a, &b, DependencyType::Calls);
        let _ = graph.add_dependency(&b, &a, DependencyType::Calls);

        let report = GraphInsightsService::analyze(&graph);
        assert!(report.summary.total_cycles > 0);
        assert!(!report.cycle_breakers.is_empty());
        assert!(report.health_score < 100.0);
        // The cycle question should be in the suggestions.
        assert!(
            report
                .suggested_questions
                .iter()
                .any(|q| q.contains("circular dependency"))
        );
    }

    #[test]
    fn test_health_score_in_range() {
        let mut graph = CallGraph::new();
        // Build a graph with cycles and many edges:
        //   0->1->2->3->4->0 (cycle of 5).
        let mut ids = Vec::new();
        for i in 0..5 {
            let id = graph.add_symbol(Symbol::new(
                format!("sym{}", i).as_str(),
                SymbolKind::Function,
                Location::new("m.rs", i, 1),
            ));
            ids.push(id);
        }
        for i in 0..5 {
            let next = (i + 1) % 5;
            let _ = graph.add_dependency(&ids[i], &ids[next], DependencyType::Calls);
        }
        let report = GraphInsightsService::analyze(&graph);
        assert!(report.health_score >= 0.0 && report.health_score <= 100.0);
    }

    #[test]
    fn test_suggested_questions_are_useful() {
        let mut graph = CallGraph::new();
        let a = graph.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("m1.rs", 1, 1),
        ));
        let b = graph.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("m2.rs", 1, 1),
        ));
        let _ = graph.add_dependency(&a, &b, DependencyType::Calls);
        let _ = graph.add_dependency(&b, &a, DependencyType::Calls);

        let report = GraphInsightsService::analyze(&graph);
        // At least one question should be non-empty and not a placeholder.
        assert!(!report.suggested_questions.is_empty());
        for q in &report.suggested_questions {
            assert!(!q.trim().is_empty());
        }
    }
}
