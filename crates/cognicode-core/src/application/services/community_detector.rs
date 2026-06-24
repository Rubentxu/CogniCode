//! Community detection using Label Propagation algorithm.
//!
//! Label Propagation is a simple O(V+E) per iteration algorithm that
//! assigns each node to the community (label) most common among its
//! neighbors. Converges when no node changes label.
//!
//! The algorithm is deterministic when using sorted label normalization:
//! after convergence, labels are sorted by the first node ID in each
//! community, then renumbered sequentially.

use std::collections::HashMap;

use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::domain::aggregates::{CallGraph, SymbolId};
use crate::infrastructure::graph::CallGraphProjection;

/// A detected community of symbols.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Community {
    /// Sequential community ID (0-based, deterministic).
    pub id: u32,
    /// Human-readable label derived from the most common file prefix.
    pub label: String,
    /// Symbols in this community.
    pub nodes: Vec<SymbolId>,
    /// Number of internal edges (within community).
    pub internal_edges: usize,
    /// Number of external edges (to other communities).
    pub external_edges: usize,
    /// Cohesion score: internal / (internal + external). 1.0 = fully cohesive.
    pub cohesion: f64,
}

/// Result of community detection.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommunityResult {
    /// Detected communities, sorted by size descending.
    pub communities: Vec<Community>,
    /// Map from SymbolId to community ID.
    pub node_communities: HashMap<String, u32>,
    /// Number of iterations until convergence.
    pub iterations: usize,
    /// Whether the algorithm converged.
    pub converged: bool,
}

/// Detect communities in a call graph using Label Propagation.
pub struct CommunityDetector;

impl CommunityDetector {
    /// Default maximum iterations before giving up.
    pub const MAX_ITERATIONS: usize = 100;

    /// Detect communities using Label Propagation.
    ///
    /// The algorithm treats the directed call graph as undirected for
    /// community detection (neighbors = both callers and callees).
    ///
    /// Returns deterministic results via sorted label normalization.
    pub fn detect(graph: &CallGraph, max_iterations: usize) -> CommunityResult {
        let projection = CallGraphProjection::from_call_graph(graph);
        Self::detect_from_projection(&projection, max_iterations)
    }

    fn detect_from_projection(
        projection: &CallGraphProjection,
        max_iterations: usize,
    ) -> CommunityResult {
        let g = projection.graph();

        if g.node_count() == 0 {
            return CommunityResult {
                communities: Vec::new(),
                node_communities: HashMap::new(),
                iterations: 0,
                converged: true,
            };
        }

        // Initialize: each node gets its own label (NodeIndex as u32).
        let mut labels: HashMap<petgraph::graph::NodeIndex, u32> = HashMap::new();
        for ni in g.node_indices() {
            labels.insert(ni, ni.index() as u32);
        }

        // Iterate label propagation.
        let mut iterations = 0;
        let mut converged = false;

        for _ in 0..max_iterations {
            iterations += 1;
            let mut changed = false;

            // Process nodes in deterministic order (sorted by NodeIndex).
            let mut node_indices: Vec<_> = g.node_indices().collect();
            node_indices.sort_by_key(|ni| ni.index());

            for ni in &node_indices {
                // Count labels among neighbors (undirected: both in and out).
                let mut label_counts: HashMap<u32, usize> = HashMap::new();

                for edge in g.edges_directed(*ni, petgraph::Direction::Outgoing) {
                    let neighbor = edge.target();
                    if let Some(&label) = labels.get(&neighbor) {
                        *label_counts.entry(label).or_insert(0) += 1;
                    }
                }
                for edge in g.edges_directed(*ni, petgraph::Direction::Incoming) {
                    let neighbor = edge.source();
                    if let Some(&label) = labels.get(&neighbor) {
                        *label_counts.entry(label).or_insert(0) += 1;
                    }
                }

                if label_counts.is_empty() {
                    // Isolated node: keep its label.
                    continue;
                }

                // Find the most common label. Ties: smaller label wins.
                let best_label = label_counts
                    .into_iter()
                    .max_by(|(l1, c1), (l2, c2)| {
                        c1.cmp(c2).then_with(|| l2.cmp(l1)) // higher count wins, then smaller label
                    })
                    .map(|(l, _)| l)
                    .unwrap();

                if labels[ni] != best_label {
                    labels.insert(*ni, best_label);
                    changed = true;
                }
            }

            if !changed {
                converged = true;
                break;
            }
        }

        // Normalize labels: sort by first NodeIndex in each group, renumber 0..N.
        let mut group_first_node: HashMap<u32, petgraph::graph::NodeIndex> = HashMap::new();
        for (&ni, &label) in &labels {
            group_first_node
                .entry(label)
                .and_modify(|first| {
                    if ni < *first {
                        *first = ni;
                    }
                })
                .or_insert(ni);
        }

        // Sort by first node index for deterministic ordering.
        let mut sorted_groups: Vec<_> = group_first_node.into_iter().collect();
        sorted_groups.sort_by_key(|(_, first_node)| first_node.index());

        let old_to_new: HashMap<u32, u32> = sorted_groups
            .into_iter()
            .enumerate()
            .map(|(new_id, (old_label, _))| (old_label, new_id as u32))
            .collect();

        // Apply new labels.
        for (_, label) in labels.iter_mut() {
            if let Some(&new_label) = old_to_new.get(label) {
                *label = new_label;
            }
        }

        // Build communities.
        let mut community_nodes: HashMap<u32, Vec<SymbolId>> = HashMap::new();
        for ni in g.node_indices() {
            if let Some(&label) = labels.get(&ni) {
                if let Some(symbol_id) = g.node_weight(ni) {
                    community_nodes
                        .entry(label)
                        .or_default()
                        .push(symbol_id.clone());
                }
            }
        }

        // Compute cohesion per community.
        let mut communities: Vec<Community> = community_nodes
            .into_iter()
            .map(|(id, nodes)| {
                let node_set: std::collections::HashSet<&SymbolId> = nodes.iter().collect();
                let mut internal_edges = 0usize;
                let mut external_edges = 0usize;

                for node in &nodes {
                    if let Some(&ni) = projection.id_to_index().get(node) {
                        for edge in g.edges_directed(ni, petgraph::Direction::Outgoing) {
                            if let Some(target_id) = g.node_weight(edge.target()) {
                                if node_set.contains(target_id) {
                                    internal_edges += 1;
                                } else {
                                    external_edges += 1;
                                }
                            }
                        }
                    }
                }

                let cohesion = if internal_edges + external_edges > 0 {
                    internal_edges as f64 / (internal_edges + external_edges) as f64
                } else {
                    1.0 // Isolated community: fully cohesive by definition.
                };

                let label = Self::derive_community_label(&nodes);

                Community {
                    id,
                    label,
                    nodes,
                    internal_edges,
                    external_edges,
                    cohesion,
                }
            })
            .collect();

        // Sort by size descending for stable, predictable output.
        communities.sort_by(|a, b| {
            b.nodes
                .len()
                .cmp(&a.nodes.len())
                .then_with(|| a.id.cmp(&b.id))
        });

        // Build node -> community map.
        let node_communities: HashMap<String, u32> = communities
            .iter()
            .flat_map(|c| c.nodes.iter().map(move |n| (n.to_string(), c.id)))
            .collect();

        CommunityResult {
            communities,
            node_communities,
            iterations,
            converged,
        }
    }

    /// Derive a human-readable label from the community's nodes.
    ///
    /// Returns the most common **parent directory** of the file paths
    /// encoded in the symbol IDs. Format of each `SymbolId` is
    /// `file:symbol:line`, so the first `:`-delimited segment is the
    /// file path. The "parent directory" of a path is the last `'/`-
    /// separated component, e.g. `src/auth/login.rs -> auth`.
    fn derive_community_label(nodes: &[SymbolId]) -> String {
        if nodes.is_empty() {
            return "empty".to_string();
        }

        // Extract file paths from SymbolIds (format: "file:symbol:line").
        let paths: Vec<&str> = nodes
            .iter()
            .filter_map(|id| id.as_str().split(':').next())
            .collect();

        if paths.is_empty() {
            return "unknown".to_string();
        }

        // Derive the parent directory name of each path. rsplit_once('/')
        // gives Some((parent_path, file)) when a slash is present. The
        // last component of the parent path is the directory name.
        let dirs: Vec<&str> = paths
            .iter()
            .filter_map(|p| {
                let (parent, _file) = p.rsplit_once('/')?;
                Some(parent.rsplit('/').next().unwrap_or(parent))
            })
            .collect();

        if dirs.is_empty() {
            return "root".to_string();
        }

        // Most frequent parent directory.
        let mut dir_counts: HashMap<&str, usize> = HashMap::new();
        for dir in &dirs {
            *dir_counts.entry(dir).or_insert(0) += 1;
        }
        dir_counts
            .into_iter()
            .max_by(|(d1, c1), (d2, c2)| {
                c1.cmp(c2).then_with(|| d1.cmp(d2)) // tie: lexicographic order
            })
            .map(|(d, _)| d.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Find god nodes per community — nodes with highest PageRank within their community.
    pub fn community_god_nodes(
        graph: &CallGraph,
        communities: &[Community],
        top_n: usize,
    ) -> Vec<(u32, Vec<(SymbolId, f64)>)> {
        use crate::application::services::graph_analytics::GraphAnalyticsService;

        let all_scores = GraphAnalyticsService::page_rank(graph, 0.85, 100);

        communities
            .iter()
            .map(|c| {
                let mut scored: Vec<(SymbolId, f64)> = c
                    .nodes
                    .iter()
                    .filter_map(|n| all_scores.get(n).map(|&s| (n.clone(), s)))
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                scored.truncate(top_n);
                (c.id, scored)
            })
            .collect()
    }

    /// Find surprising connections — edges that cross community boundaries.
    pub fn surprising_connections(
        graph: &CallGraph,
        result: &CommunityResult,
        top_n: usize,
    ) -> Vec<(SymbolId, SymbolId, u32, u32)> {
        let projection = CallGraphProjection::from_call_graph(graph);
        let g = projection.graph();

        let mut cross_edges: Vec<(SymbolId, SymbolId, u32, u32)> = Vec::new();

        for edge in g.edge_references() {
            let src_id = g.node_weight(edge.source());
            let dst_id = g.node_weight(edge.target());
            if let (Some(src), Some(dst)) = (src_id, dst_id) {
                let src_comm = result.node_communities.get(&src.to_string());
                let dst_comm = result.node_communities.get(&dst.to_string());
                if let (Some(&sc), Some(&dc)) = (src_comm, dst_comm) {
                    if sc != dc {
                        cross_edges.push((src.clone(), dst.clone(), sc, dc));
                    }
                }
            }
        }

        // Sort for deterministic ordering before truncating.
        cross_edges.sort_by(|a, b| {
            a.0.as_str()
                .cmp(b.0.as_str())
                .then_with(|| a.1.as_str().cmp(b.1.as_str()))
        });
        cross_edges.truncate(top_n);
        cross_edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::{CallGraph, Symbol};
    use crate::domain::value_objects::{DependencyType, Location, SymbolKind};

    fn sym(name: &str, file: &str) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new(file, 1, 1))
    }

    fn id(name: &str, file: &str) -> SymbolId {
        SymbolId::new(format!("{}:{}:1", file, name))
    }

    fn add_edge(g: &mut CallGraph, a: &str, fa: &str, b: &str, fb: &str) {
        g.add_symbol(sym(a, fa));
        g.add_symbol(sym(b, fb));
        let _ = g.add_dependency(&id(a, fa), &id(b, fb), DependencyType::Calls);
    }

    #[test]
    fn test_empty_graph_returns_empty() {
        let graph = CallGraph::new();
        let result = CommunityDetector::detect(&graph, 100);
        assert!(result.communities.is_empty());
        assert!(result.converged);
    }

    #[test]
    fn test_single_node_is_one_community() {
        let mut graph = CallGraph::new();
        graph.add_symbol(sym("main", "main.rs"));
        let result = CommunityDetector::detect(&graph, 100);
        assert_eq!(result.communities.len(), 1);
        assert_eq!(result.communities[0].nodes.len(), 1);
    }

    #[test]
    fn test_two_disconnected_groups_form_two_communities() {
        // Group 1: a→b, Group 2: c→d (no connections between groups).
        let mut graph = CallGraph::new();
        add_edge(&mut graph, "a", "mod1.rs", "b", "mod1.rs");
        add_edge(&mut graph, "c", "mod2.rs", "d", "mod2.rs");

        let result = CommunityDetector::detect(&graph, 100);
        assert!(result.communities.len() >= 1);
        assert!(result.converged);
    }

    #[test]
    fn test_fully_connected_single_community() {
        // a→b, a→c, b→c (all connected).
        let mut graph = CallGraph::new();
        add_edge(&mut graph, "a", "mod.rs", "b", "mod.rs");
        add_edge(&mut graph, "a", "mod.rs", "c", "mod.rs");
        add_edge(&mut graph, "b", "mod.rs", "c", "mod.rs");

        let result = CommunityDetector::detect(&graph, 100);
        assert_eq!(result.communities.len(), 1);
        assert!(result.communities[0].cohesion > 0.5);
    }

    #[test]
    fn test_convergence_within_max_iterations() {
        let mut graph = CallGraph::new();
        // Build a chain: a→b→c→d→e.
        for pair in [("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")] {
            add_edge(&mut graph, pair.0, "chain.rs", pair.1, "chain.rs");
        }
        let result = CommunityDetector::detect(&graph, 100);
        assert!(result.converged);
        assert!(result.iterations <= 100);
    }

    #[test]
    fn test_community_labels_are_sequential() {
        let mut graph = CallGraph::new();
        add_edge(&mut graph, "a", "m1.rs", "b", "m1.rs");
        add_edge(&mut graph, "c", "m2.rs", "d", "m2.rs");

        let result = CommunityDetector::detect(&graph, 100);
        let ids: Vec<u32> = result.communities.iter().map(|c| c.id).collect();
        // Should be sequential: 0, 1, ...
        for (i, &id) in ids.iter().enumerate() {
            assert_eq!(id, i as u32);
        }
    }

    #[test]
    fn test_deterministic_results() {
        let mut graph = CallGraph::new();
        add_edge(&mut graph, "a", "m1.rs", "b", "m1.rs");
        add_edge(&mut graph, "a", "m1.rs", "c", "m1.rs");
        add_edge(&mut graph, "x", "m2.rs", "y", "m2.rs");

        let result1 = CommunityDetector::detect(&graph, 100);
        let result2 = CommunityDetector::detect(&graph, 100);

        assert_eq!(result1.communities.len(), result2.communities.len());
        assert_eq!(result1.iterations, result2.iterations);
        // Compare node→community mapping for full determinism.
        assert_eq!(result1.node_communities, result2.node_communities);
    }

    #[test]
    fn test_derive_community_label() {
        let nodes = vec![
            SymbolId::new("src/auth/login.rs:authenticate:10"),
            SymbolId::new("src/auth/oauth.rs:validate:20"),
        ];
        let label = CommunityDetector::derive_community_label(&nodes);
        assert_eq!(label, "auth");
    }
}
