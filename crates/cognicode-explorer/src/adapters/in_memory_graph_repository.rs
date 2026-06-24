//! `InMemoryGraphRepository` — in-memory `GraphRepository` impl for tests.
//!
//! Holds a `Vec<GraphNode>` + `Vec<GraphEdge>`. The `search` method
//! does a simple case-insensitive substring match on the label +
//! properties (no FTS5 — the port contract is "rank + paginate",
//! not "use FTS5"; the PG adapter provides FTS5).
//!
//! T21 — backs the `graph_search` MCP tool's unit tests.
//!
//! Implements the canonical `cognicode_core::ports::GraphRepository`
//! trait. Error returns are `GraphResult` (not the explorer's
//! `ExplorerResult`) — the adapter wraps upstream failures in
//! `GraphError::Storage`.

use std::collections::{HashMap, HashSet, VecDeque};

use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::ports::GraphRepository;
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
use cognicode_core::domain::value_objects::node_kind::NodeKind;
use cognicode_core::domain::{GraphError, GraphResult, SearchPage};

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
    ) -> GraphResult<SearchPage> {
        // Empty query → empty page (contract).
        if query.is_empty() {
            return Ok(SearchPage {
                items: Vec::new(),
                raw_total: 0,
                next_cursor: None,
                raw_rank: 0.0,
                item_ranks: Vec::new(),
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
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.1.id.as_str().cmp(b.1.id.as_str()))
        });

        let raw_total = scored.len() as u64;

        // Apply cursor offset. The cursor format is "<offset>".
        let offset: usize = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
        if offset > scored.len() {
            return Ok(SearchPage {
                items: Vec::new(),
                raw_total,
                next_cursor: None,
                raw_rank: 0.0,
                item_ranks: Vec::new(),
            });
        }
        let end = (offset + limit).min(scored.len());
        // Keep the per-item scores so the MCP tool can emit
        // distinct `score` values per result. `Vec::new()` (the
        // fallback) would mean "page-level rank only" — see the
        // MCP handler.
        let page_items: Vec<GraphNode> = scored[offset..end]
            .iter()
            .map(|(_, n)| (*n).clone())
            .collect();
        let item_ranks: Vec<f64> = scored[offset..end].iter().map(|(s, _)| *s).collect();
        let next_cursor = if end < scored.len() {
            Some(end.to_string())
        } else {
            None
        };
        // `raw_rank` mirrors the top item's score (kept for
        // backward compatibility — the federation layer
        // and existing tests rely on it).
        let raw_rank = item_ranks.first().copied().unwrap_or(0.0);
        Ok(SearchPage {
            items: page_items,
            raw_total,
            next_cursor,
            raw_rank,
            item_ranks,
        })
    }

    fn find_nodes_by_kind(&self, kind: &NodeKind) -> GraphResult<Vec<GraphNode>> {
        Ok(self
            .nodes
            .iter()
            .filter(|n| &n.kind == kind)
            .cloned()
            .collect())
    }

    fn get_node(&self, id: &NodeId) -> GraphResult<Option<GraphNode>> {
        Ok(self.nodes.iter().find(|n| &n.id == id).cloned())
    }

    fn find_outgoing_edges(&self, id: &NodeId) -> GraphResult<Vec<GraphEdge>> {
        Ok(self
            .edges
            .iter()
            .filter(|e| &e.source == id)
            .cloned()
            .collect())
    }

    fn edges_by_kind(&self, node: &NodeId, kinds: &[EdgeKind]) -> GraphResult<Vec<GraphEdge>> {
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
                results
                    .iter()
                    .position(|r| r.source == k.0 && r.target == k.1 && r.kind == k.2)
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

    fn rationale_subgraph(
        &self,
        focus: &NodeId,
        max_depth: u32,
        max_nodes: usize,
    ) -> GraphResult<(Vec<GraphNode>, Vec<GraphEdge>, bool)> {
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
        // Tracks whether the BFS was cut short by `max_nodes` (as
        // opposed to draining the queue naturally). A natural
        // drain — depth exhausted or queue empty — is NOT a
        // truncation; only the explicit `break` at the size
        // boundary counts.
        let mut truncated = false;

        visited.insert(focus.clone());
        queue.push_back((focus.clone(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            if nodes.len() >= max_nodes {
                truncated = true;
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
                    truncated = true;
                    break;
                }

                let is_new = visited.insert(e.target.clone());
                if is_new {
                    if let Some(target_node) = self.nodes.iter().find(|n| n.id == e.target).cloned()
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

        Ok((nodes, edges, truncated))
    }
}

// Suppress the unused `GraphError` import warning when nothing in
// the file actually instantiates the variant — the import is kept
// for symmetry with the PG adapter and to make the adapter's
// `GraphResult` return type self-documenting.
#[allow(dead_code)]
fn _graph_error_compiles(err: GraphError) -> String {
    format!("{err}")
}
