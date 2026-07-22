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

    fn find_nodes_by_kind_paginated(
        &self,
        kind: &NodeKind,
        limit: usize,
        cursor: Option<&str>,
    ) -> GraphResult<SearchPage> {
        let all_nodes: Vec<GraphNode> = self
            .nodes
            .iter()
            .filter(|n| &n.kind == kind)
            .cloned()
            .collect();

        let raw_total = all_nodes.len() as u64;

        // Apply cursor offset. The cursor format is "<offset>".
        let offset: usize = cursor.and_then(|c| c.parse::<usize>().ok()).unwrap_or(0);
        if offset > all_nodes.len() {
            return Ok(SearchPage {
                items: Vec::new(),
                raw_total,
                next_cursor: None,
                raw_rank: 0.0,
                item_ranks: Vec::new(),
            });
        }

        let end = (offset + limit).min(all_nodes.len());
        let page_items: Vec<GraphNode> = all_nodes[offset..end].to_vec();
        let next_cursor = if end < all_nodes.len() {
            Some(end.to_string())
        } else {
            None
        };

        Ok(SearchPage {
            items: page_items,
            raw_total,
            next_cursor,
            raw_rank: 0.0,
            item_ranks: Vec::new(),
        })
    }

    fn search_paginated(
        &self,
        query: &str,
        kinds: &[NodeKind],
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
        let allowed: Option<HashSet<&'static str>> = if kinds.is_empty() {
            None
        } else {
            Some(kinds.iter().map(|k| k.as_str()).collect())
        };

        // Score each candidate by the simple formula:
        //   1.0 if label contains the query, 0.5 if any property does.
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

        // Apply cursor offset.
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
        let raw_rank = item_ranks.first().copied().unwrap_or(0.0);

        Ok(SearchPage {
            items: page_items,
            raw_total,
            next_cursor,
            raw_rank,
            item_ranks,
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::value_objects::node_kind::NodeKind;
    use std::collections::HashMap;

    fn make_node(id: &str, kind: NodeKind, label: &str) -> GraphNode {
        GraphNode {
            id: NodeId(id.to_string()),
            kind,
            label: label.to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn find_nodes_by_kind_paginated_returns_first_page() {
        let nodes: Vec<GraphNode> = (1..=25)
            .map(|i| make_node(&format!("doc:{i}"), NodeKind::Doc, &format!("Doc {i}")))
            .collect();
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        let result = repo.find_nodes_by_kind_paginated(&NodeKind::Doc, 10, None);
        assert!(result.is_ok());
        let page = result.unwrap();
        assert_eq!(page.items.len(), 10);
        assert_eq!(page.raw_total, 25);
        assert!(page.next_cursor.is_some());
        assert_eq!(page.next_cursor.unwrap(), "10");
    }

    #[test]
    fn find_nodes_by_kind_paginated_cursor_advance_no_overlap() {
        let nodes: Vec<GraphNode> = (1..=25)
            .map(|i| make_node(&format!("doc:{i}"), NodeKind::Doc, &format!("Doc {i}")))
            .collect();
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        // First page
        let page1 = repo
            .find_nodes_by_kind_paginated(&NodeKind::Doc, 10, None)
            .unwrap();
        let cursor = page1.next_cursor.clone();

        // Second page using cursor
        let page2 = repo
            .find_nodes_by_kind_paginated(&NodeKind::Doc, 10, cursor.as_deref())
            .unwrap();

        assert_eq!(page2.items.len(), 10);
        // No overlap: first page has items 1-10, second has 11-20
        let page1_ids: Vec<_> = page1.items.iter().map(|n| n.id.as_str()).collect();
        let page2_ids: Vec<_> = page2.items.iter().map(|n| n.id.as_str()).collect();
        for id in &page1_ids {
            assert!(!page2_ids.contains(id), "Found overlap: {id}");
        }
    }

    #[test]
    fn find_nodes_by_kind_paginated_kind_filter() {
        let nodes = vec![
            make_node("doc:1", NodeKind::Doc, "Design Doc"),
            make_node("dec:1", NodeKind::Decision, "ADR 1"),
            make_node("ev:1", NodeKind::Evidence, "Evidence 1"),
        ];
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        let result = repo
            .find_nodes_by_kind_paginated(&NodeKind::Decision, 10, None)
            .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].id.as_str(), "dec:1");
    }

    #[test]
    fn search_paginated_basic_query() {
        let nodes = vec![
            make_node("doc:1", NodeKind::Doc, "Getting Started Guide"),
            make_node("doc:2", NodeKind::Doc, "API Reference"),
            make_node("doc:3", NodeKind::Doc, "Developer Guide"),
        ];
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        let result = repo
            .search_paginated("guide", &[NodeKind::Doc], 10, None)
            .unwrap();
        assert_eq!(result.items.len(), 2); // "Getting Started Guide" and "Developer Guide"
    }

    #[test]
    fn search_paginated_cursor_pagination() {
        let nodes: Vec<GraphNode> = (1..=15)
            .map(|i| make_node(&format!("doc:{i}"), NodeKind::Doc, &format!("Document {i}")))
            .collect();
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        // First page of 5
        let page1 = repo
            .search_paginated("document", &[NodeKind::Doc], 5, None)
            .unwrap();
        assert_eq!(page1.items.len(), 5);
        let cursor = page1.next_cursor.clone();

        // Second page
        let page2 = repo
            .search_paginated("document", &[NodeKind::Doc], 5, cursor.as_deref())
            .unwrap();
        assert_eq!(page2.items.len(), 5);

        // No overlap
        let ids1: Vec<_> = page1.items.iter().map(|n| n.id.as_str()).collect();
        let ids2: Vec<_> = page2.items.iter().map(|n| n.id.as_str()).collect();
        for id in &ids1 {
            assert!(!ids2.contains(id), "Overlap found: {id}");
        }
    }

    #[test]
    fn search_paginated_empty_query_returns_empty_page() {
        let nodes = vec![make_node("doc:1", NodeKind::Doc, "Test Doc")];
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        let result = repo
            .search_paginated("", &[NodeKind::Doc], 10, None)
            .unwrap();
        assert!(result.items.is_empty());
        assert_eq!(result.raw_total, 0);
        assert!(result.next_cursor.is_none());
    }

    // -------------------------------------------------------------------------
    // rationale_subgraph tests — BFS over Justifies/Cites/Resolves/CorroboratedBy
    // -------------------------------------------------------------------------

    /// Scenario 3 CRITICAL: BFS with non-empty subgraph returns nodes AND edges.
    /// Graph: A(Decision) --Justifies--> D(Decision) --Cites--> X(Doc)
    ///         D --CorroboratedBy--> Y(Evidence)
    ///         Z(Decision) --Justifies--> D
    /// When calling rationale_subgraph on "A" with depth=2, we expect:
    /// - Nodes: A, D (direct), X (via D->X), Y (via D->Y)
    /// - Edges: A->D (Justifies), D->X (Cites), D->Y (CorroboratedBy)
    /// Note: Z->D is NOT included because BFS from A never visits Z (Z is not reachable from A)
    #[test]
    fn rationale_subgraph_bfs_with_edges() {
        use cognicode_core::domain::value_objects::Provenance;

        let nodes = vec![
            make_node("A", NodeKind::Decision, "Decision A"),
            make_node("D", NodeKind::Decision, "Decision D"),
            make_node("X", NodeKind::Doc, "Doc X"),
            make_node("Y", NodeKind::Evidence, "Evidence Y"),
            make_node("Z", NodeKind::Decision, "Decision Z"),
        ];
        let edges = vec![
            GraphEdge {
                source: NodeId::new("A"),
                target: NodeId::new("D"),
                kind: EdgeKind::Justifies,
                provenance: Provenance::Manual,
                confidence: 0.9,
                metadata: HashMap::new(),
            },
            GraphEdge {
                source: NodeId::new("D"),
                target: NodeId::new("X"),
                kind: EdgeKind::Cites,
                provenance: Provenance::Extracted,
                confidence: 0.8,
                metadata: HashMap::new(),
            },
            GraphEdge {
                source: NodeId::new("D"),
                target: NodeId::new("Y"),
                kind: EdgeKind::CorroboratedBy,
                provenance: Provenance::Tested,
                confidence: 0.7,
                metadata: HashMap::new(),
            },
            GraphEdge {
                source: NodeId::new("Z"),
                target: NodeId::new("D"),
                kind: EdgeKind::Justifies,
                provenance: Provenance::Inferred,
                confidence: 0.5,
                metadata: HashMap::new(),
            },
        ];
        let repo = InMemoryGraphRepository::new(nodes, edges);

        let result = repo
            .rationale_subgraph(&NodeId::new("A"), 2, 100)
            .expect("rationale_subgraph should succeed");

        let (subgraph_nodes, subgraph_edges, truncated) = result;

        // Focus node A is always included
        assert!(
            subgraph_nodes.iter().any(|n| n.id.as_str() == "A"),
            "Focus node A should be in subgraph"
        );
        // D is reachable via A->D (depth 1)
        assert!(
            subgraph_nodes.iter().any(|n| n.id.as_str() == "D"),
            "D should be in subgraph (A->D)"
        );
        // X is reachable via A->D->X (depth 2)
        assert!(
            subgraph_nodes.iter().any(|n| n.id.as_str() == "X"),
            "X should be in subgraph (A->D->X)"
        );
        // Y is reachable via A->D->Y (depth 2)
        assert!(
            subgraph_nodes.iter().any(|n| n.id.as_str() == "Y"),
            "Y should be in subgraph (A->D->Y)"
        );
        // Z is NOT reachable from A (incoming edge only), so should NOT be included
        assert!(
            !subgraph_nodes.iter().any(|n| n.id.as_str() == "Z"),
            "Z should NOT be in subgraph (only reachable via incoming edge from Z->D)"
        );

        // Edges should be non-empty
        assert!(
            !subgraph_edges.is_empty(),
            "Edges should be non-empty for BFS with edges"
        );

        // Verify specific edges are present
        assert!(
            subgraph_edges
                .iter()
                .any(|e| e.source.as_str() == "A" && e.target.as_str() == "D"),
            "A->D Justifies edge should be present"
        );
        assert!(
            subgraph_edges
                .iter()
                .any(|e| e.source.as_str() == "D" && e.target.as_str() == "X"),
            "D->X Cites edge should be present"
        );
        assert!(
            subgraph_edges
                .iter()
                .any(|e| e.source.as_str() == "D" && e.target.as_str() == "Y"),
            "D->Y CorroboratedBy edge should be present"
        );

        // Z->D should NOT be present (Z not in node set)
        assert!(
            !subgraph_edges
                .iter()
                .any(|e| e.source.as_str() == "Z" && e.target.as_str() == "D"),
            "Z->D edge should NOT be present (Z not reachable from A)"
        );

        // Should not be truncated
        assert!(!truncated, "Should not be truncated with max_nodes=100");
    }

    /// Scenario 6 partial: focus-only BFS with max_nodes=1 returns only focus node, no edges.
    /// When max_nodes=1, BFS cannot expand beyond the focus node, so edges should be empty.
    #[test]
    fn rationale_subgraph_focus_only_no_edges() {
        use cognicode_core::domain::value_objects::Provenance;

        let nodes = vec![
            make_node("A", NodeKind::Decision, "Decision A"),
            make_node("D", NodeKind::Decision, "Decision D"),
        ];
        let edges = vec![GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("D"),
            kind: EdgeKind::Justifies,
            provenance: Provenance::Manual,
            confidence: 0.9,
            metadata: HashMap::new(),
        }];
        let repo = InMemoryGraphRepository::new(nodes, edges);

        // max_nodes=1 means only the focus node can be in the result
        let result = repo
            .rationale_subgraph(&NodeId::new("A"), 2, 1)
            .expect("rationale_subgraph should succeed");

        let (subgraph_nodes, subgraph_edges, truncated) = result;

        // Focus node should be present
        assert_eq!(
            subgraph_nodes.len(),
            1,
            "Only focus node should be present with max_nodes=1"
        );
        assert_eq!(
            subgraph_nodes[0].id.as_str(),
            "A",
            "Focus node A should be the only node"
        );

        // Edges should be empty because BFS couldn't expand
        assert!(
            subgraph_edges.is_empty(),
            "Edges should be empty when BFS cannot expand (max_nodes=1)"
        );

        // truncated=true because we hit max_nodes during expansion - we wanted to
        // add D but couldn't fit it within max_nodes=1
        assert!(
            truncated,
            "Should be truncated when max_nodes=1 prevents edge expansion"
        );
    }

    /// Scenario 4: max_depth=0 returns only the focus node, no edges.
    /// When max_depth=0, the BFS never expands beyond the focus node because
    /// depth >= max_depth immediately, so edges should be empty.
    #[test]
    fn rationale_subgraph_max_depth_zero_returns_focus_only() {
        use cognicode_core::domain::value_objects::Provenance;

        let nodes = vec![
            make_node("A", NodeKind::Decision, "Decision A"),
            make_node("D", NodeKind::Decision, "Decision D"),
        ];
        let edges = vec![GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("D"),
            kind: EdgeKind::Justifies,
            provenance: Provenance::Manual,
            confidence: 0.9,
            metadata: HashMap::new(),
        }];
        let repo = InMemoryGraphRepository::new(nodes, edges);

        // max_depth=0 means no expansion beyond the focus node
        let result = repo
            .rationale_subgraph(&NodeId::new("A"), 0, 100)
            .expect("rationale_subgraph should succeed");

        let (subgraph_nodes, subgraph_edges, truncated) = result;

        // Focus node should be present
        assert_eq!(
            subgraph_nodes.len(),
            1,
            "Only focus node should be present with max_depth=0"
        );
        assert_eq!(
            subgraph_nodes[0].id.as_str(),
            "A",
            "Focus node A should be the only node"
        );

        // Edges should be empty because BFS couldn't expand (depth >= max_depth)
        assert!(
            subgraph_edges.is_empty(),
            "Edges should be empty when max_depth=0"
        );

        // Should not be truncated
        assert!(
            !truncated,
            "Should not be truncated with max_depth=0 and sufficient max_nodes"
        );
    }

    /// Scenario 5 partial: rationale_subgraph returns Ok(empty) when no graph data.
    /// This is the fallback behavior - both nodes and edges empty, truncated=false.
    #[test]
    fn rationale_subgraph_empty_graph_returns_empty() {
        let nodes = vec![make_node("A", NodeKind::Decision, "Decision A")];
        let repo = InMemoryGraphRepository::new(nodes, Vec::new());

        let result = repo
            .rationale_subgraph(&NodeId::new("A"), 2, 100)
            .expect("rationale_subgraph should succeed even with empty edges");

        let (subgraph_nodes, subgraph_edges, truncated) = result;

        // Focus node should still be present (always included)
        assert_eq!(
            subgraph_nodes.len(),
            1,
            "Focus node should be present even with empty edges"
        );
        assert_eq!(
            subgraph_nodes[0].id.as_str(),
            "A",
            "Focus node A should be present"
        );

        // Edges should be empty
        assert!(
            subgraph_edges.is_empty(),
            "Edges should be empty when no edges in graph"
        );

        // Should not be truncated
        assert!(!truncated, "Should not be truncated with empty edges");
    }
}
