//! WASM-friendly adapter: build adjacency from JSON DTOs.
//!
//! This adapter is what the `cognicode-graph-wasm` bindgen shim calls.
//! It reuses the same JSON shapes the frontend already carries
//! (`GraphNode[]` + `GraphEdge[]`), so no DTO mapping is needed.

use crate::graph_builder::GraphBuilder;
use serde::{Deserialize, Serialize};

/// JSON node DTO — matches frontend `GraphNode`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonNode {
    /// Unique node identifier.
    pub id: String,
    /// Optional human-readable label.
    #[serde(default)]
    pub label: Option<String>,
}

/// JSON edge DTO — matches frontend `GraphEdge`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonEdge {
    /// Source node id.
    pub source: String,
    /// Target node id.
    pub target: String,
}

/// WASM-friendly graph: holds JSON DTOs and builds adjacency on demand.
#[derive(Debug, Clone, Default)]
pub struct JsonGraph {
    /// List of nodes in the graph.
    pub nodes: Vec<JsonNode>,
    /// List of directed edges in the graph.
    pub edges: Vec<JsonEdge>,
}

impl JsonGraph {
    /// Create a new `JsonGraph` from nodes and edges.
    pub fn new(nodes: Vec<JsonNode>, edges: Vec<JsonEdge>) -> Self {
        Self { nodes, edges }
    }
}

impl GraphBuilder for JsonGraph {
    fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>) {
        let n = self.nodes.len();
        let id_to_idx: std::collections::HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut out_degree: Vec<usize> = vec![0; n];

        for edge in &self.edges {
            // Dangling edges — an endpoint that is not a declared node — are
            // ignored, per spec REQ-006 and the skip-orphan behavior of
            // call_graph_projection.rs.
            //
            // This used to allocate a "ghost" index for each dangling
            // endpoint and then discard it: the index was only ever compared
            // against `n`, so every ghost was ignored and the counter that
            // assigned them was unobservable. The mutation gate is what made
            // that visible — the counter could be changed from `+= 1` to
            // `*= 1` without changing a single output — and a counter whose
            // value nobody can see is a counter that should not exist.
            let (Some(&s), Some(&t)) = (
                id_to_idx.get(edge.source.as_str()),
                id_to_idx.get(edge.target.as_str()),
            ) else {
                continue;
            };
            in_neighbors[t].push(s);
            out_degree[s] += 1;
        }
        (in_neighbors, out_degree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_yields_empty_adjacency() {
        let g = JsonGraph::default();
        let (in_neighbors, out_degree) = g.build_adjacency();
        assert!(in_neighbors.is_empty());
        assert!(out_degree.is_empty());
    }

    #[test]
    fn three_node_cycle() {
        let g = JsonGraph::new(
            vec![
                JsonNode {
                    id: "A".into(),
                    label: None,
                },
                JsonNode {
                    id: "B".into(),
                    label: None,
                },
                JsonNode {
                    id: "C".into(),
                    label: None,
                },
            ],
            vec![
                JsonEdge {
                    source: "A".into(),
                    target: "B".into(),
                },
                JsonEdge {
                    source: "B".into(),
                    target: "C".into(),
                },
                JsonEdge {
                    source: "C".into(),
                    target: "A".into(),
                },
            ],
        );
        let (in_neighbors, out_degree) = g.build_adjacency();
        assert_eq!(in_neighbors.len(), 3);
        assert_eq!(out_degree, vec![1, 1, 1]);
        // A is called by C
        assert_eq!(in_neighbors[0], vec![2]);
    }

    #[test]
    fn dangling_edge_ignored() {
        // All edges involve ghost nodes (indices >= n) and are skipped per
        // skip-orphan behavior: phantom slots (s or t >= n) are ignored.
        let g = JsonGraph::new(
            vec![JsonNode {
                id: "A".into(),
                label: None,
            }],
            vec![
                JsonEdge {
                    source: "A".into(),
                    target: "B".into(),
                }, // dangling target
                JsonEdge {
                    source: "X".into(),
                    target: "A".into(),
                }, // dangling source
                JsonEdge {
                    source: "A".into(),
                    target: "Y".into(),
                }, // dangling target
            ],
        );
        let (_in_neighbors, out_degree) = g.build_adjacency();
        // No edge has both source AND target in [0, n), so no contributions.
        assert_eq!(out_degree, vec![0]);
    }
}
