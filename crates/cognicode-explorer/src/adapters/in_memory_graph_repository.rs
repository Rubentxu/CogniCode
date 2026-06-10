//! `InMemoryGraphRepository` — in-memory `GraphRepository` impl for tests.
//!
//! Holds a `Vec<GraphNode>` + `Vec<GraphEdge>`. The `search` method
//! does a simple case-insensitive substring match on the label +
//! properties (no FTS5 — the port contract is "rank + paginate",
//! not "use FTS5"; the PG adapter provides FTS5).
//!
//! T21 — backs the `graph_search` MCP tool's unit tests.

use std::collections::HashSet;

use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::value_objects::node_kind::NodeKind;

use crate::error::ExplorerResult;
use crate::ports::graph_repository::{GraphRepository, SearchPage};

/// In-memory store keyed by `NodeId`. Edges are stored as a flat
/// list and filtered on `find_outgoing_edges`.
pub struct InMemoryGraphRepository {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

impl InMemoryGraphRepository {
    pub fn new(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> Self {
        Self { nodes, edges }
    }

    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl GraphRepository for InMemoryGraphRepository {
    fn search(
        &self,
        query: &str,
        node_kinds: &[NodeKind],
        limit: usize,
        cursor: Option<&str>,
    ) -> ExplorerResult<SearchPage> {
        // Empty query → empty page (contract).
        if query.is_empty() {
            return Ok(SearchPage {
                items: Vec::new(),
                raw_total: 0,
                next_cursor: None,
                raw_rank: 0.0,
            });
        }
        let q = query.to_ascii_lowercase();
        let allowed: Option<HashSet<&'static str>> = if node_kinds.is_empty() {
            None
        } else {
            Some(node_kinds.iter().map(|k| k.as_str()).collect())
        };

        // Score each candidate by the simple formula:
        //   1.0 if label contains the query, 0.5 if any property does.
        // This mirrors the PG FTS5 behaviour closely enough for tests
        // and is the same shape the MCP tool surfaces to callers.
        let mut scored: Vec<(f64, &GraphNode)> = self
            .nodes
            .iter()
            .filter_map(|n| {
                if let Some(allowed) = allowed.as_ref() {
                    if !allowed.contains(n.kind.as_str()) {
                        return None;
                    }
                }
                let label_hit = n.label.to_ascii_lowercase().contains(&q);
                let prop_hit = n
                    .properties
                    .values()
                    .any(|v| v.to_ascii_lowercase().contains(&q));
                if label_hit {
                    Some((1.0, n))
                } else if prop_hit {
                    Some((0.5, n))
                } else {
                    None
                }
            })
            .collect();
        // Stable sort: higher score first, then by id for determinism.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.id.as_str().cmp(b.1.id.as_str())));

        let raw_total = scored.len() as u64;

        // Apply cursor offset. The cursor format is "<offset>".
        let offset: usize = cursor
            .and_then(|c| c.parse::<usize>().ok())
            .unwrap_or(0);
        if offset > scored.len() {
            return Ok(SearchPage {
                items: Vec::new(),
                raw_total,
                next_cursor: None,
                raw_rank: 0.0,
            });
        }
        let end = (offset + limit).min(scored.len());
        let page_items: Vec<GraphNode> = scored[offset..end].iter().map(|(_, n)| (*n).clone()).collect();
        let next_cursor = if end < scored.len() {
            Some(end.to_string())
        } else {
            None
        };
        let raw_rank = page_items.first().map(|_| 1.0).unwrap_or(0.0);
        Ok(SearchPage {
            items: page_items,
            raw_total,
            next_cursor,
            raw_rank,
        })
    }

    fn find_nodes_by_kind(&self, kind: &NodeKind) -> ExplorerResult<Vec<GraphNode>> {
        Ok(self
            .nodes
            .iter()
            .filter(|n| &n.kind == kind)
            .cloned()
            .collect())
    }

    fn get_node(&self, id: &NodeId) -> ExplorerResult<Option<GraphNode>> {
        Ok(self.nodes.iter().find(|n| &n.id == id).cloned())
    }

    fn find_outgoing_edges(&self, id: &NodeId) -> ExplorerResult<Vec<GraphEdge>> {
        Ok(self
            .edges
            .iter()
            .filter(|e| &e.source == id)
            .cloned()
            .collect())
    }
}
