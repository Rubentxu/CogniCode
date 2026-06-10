//! `InMemoryGraphRepository` — in-memory `GraphRepository` impl for tests.
//!
//! Holds a `Vec<GraphNode>` + `Vec<GraphEdge>`. The `search` method
//! does a simple case-insensitive substring match on the label +
//! properties (no FTS5 — the port contract is "rank + paginate",
//! not "use FTS5"; the PG adapter provides FTS5).
//!
//! T21 — backs the `graph_search` MCP tool's unit tests.

use std::collections::{HashMap, HashSet, VecDeque};

use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
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

    #[cfg(feature = "multimodal")]
    fn edges_by_kind(
        &self,
        node: &NodeId,
        kinds: &[EdgeKind],
    ) -> ExplorerResult<Vec<GraphEdge>> {
        // Empty kinds short-circuit: no kind to match → no edges.
        if kinds.is_empty() {
            return Ok(Vec::new());
        }
        let kind_set: HashSet<&EdgeKind> = kinds.iter().collect();
        let mut seen: HashSet<(NodeId, NodeId, EdgeKind)> = HashSet::new();
        let mut results: Vec<GraphEdge> = Vec::new();

        for e in self.edges.iter().filter(|e| &e.source == node) {
            if !kind_set.contains(&e.kind) {
                continue;
            }
            let key = (e.source.clone(), e.target.clone(), e.kind.clone());
            // Dedup: keep the edge with the highest confidence.
            if let Some(pos) = seen.get(&key).and_then(|k| {
                results.iter().position(|r| {
                    r.source == k.0 && r.target == k.1 && r.kind == k.2
                })
            }) {
                if e.confidence > results[pos].confidence {
                    results[pos] = e.clone();
                }
            } else {
                seen.insert(key);
                results.push(e.clone());
            }
        }
        Ok(results)
    }

    #[cfg(feature = "multimodal")]
    fn rationale_subgraph(
        &self,
        focus: &NodeId,
        max_depth: u32,
        max_nodes: usize,
    ) -> ExplorerResult<(Vec<GraphNode>, Vec<GraphEdge>)> {
        // Multimodal edge kinds for rationale traversal.
        let rationale_kinds: HashSet<EdgeKind> = [
            EdgeKind::Justifies,
            EdgeKind::Cites,
            EdgeKind::Resolves,
            EdgeKind::CorroboratedBy,
        ]
        .into();

        // Always include the focus node.
        let focus_node = self.get_node(focus)?.unwrap_or_else(|| GraphNode {
            id: focus.clone(),
            kind: NodeKind::Doc,
            label: focus.0.clone(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        });

        let mut nodes: Vec<GraphNode> = vec![focus_node];
        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut queue: VecDeque<(NodeId, u32)> = VecDeque::new();

        visited.insert(focus.clone());
        queue.push_back((focus.clone(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            if nodes.len() >= max_nodes {
                break;
            }

            for e in self.edges.iter() {
                if &e.source != &current {
                    continue;
                }
                if !rationale_kinds.contains(&e.kind) {
                    continue;
                }
                if nodes.len() >= max_nodes {
                    break;
                }

                let is_new = visited.insert(e.target.clone());
                if is_new {
                    if let Some(target_node) = self
                        .nodes
                        .iter()
                        .find(|n| n.id == e.target)
                        .cloned()
                    {
                        nodes.push(target_node);
                    } else {
                        // Create a stub node for unknown targets.
                        nodes.push(GraphNode {
                            id: e.target.clone(),
                            kind: NodeKind::Doc,
                            label: e.target.0.clone(),
                            source_path: None,
                            properties: HashMap::new(),
                            created_at: chrono::Utc::now(),
                            updated_at: chrono::Utc::now(),
                        });
                    }
                }
                edges.push(e.clone());
                if is_new {
                    queue.push_back((e.target.clone(), depth + 1));
                }
            }
        }

        // Drop edges whose endpoints are not in the kept set.
        let kept: HashSet<&NodeId> = nodes.iter().map(|n| &n.id).collect();
        edges.retain(|e| kept.contains(&e.source) && kept.contains(&e.target));

        Ok((nodes, edges))
    }
}
