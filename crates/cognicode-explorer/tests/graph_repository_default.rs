//! Unit tests for the default trait methods on `GraphRepository`.
//!
//! Tests that `find_nodes_by_kind_paginated` default implementation:
//! - Returns `SearchPage` with `raw_total == items.len()`
//! - Returns `next_cursor = None` (no real pagination in default impl)

use cognicode_core::domain::aggregates::generic_graph::{GraphNode, NodeId};
use cognicode_core::domain::ports::graph_repository::GraphRepository;
use cognicode_core::domain::value_objects::node_kind::NodeKind;
use std::collections::HashMap;
use std::sync::Arc;

/// A mock `GraphRepository` that only implements the base methods
/// (not the paginated variants), relying on the default implementations.
struct MockGraphRepository {
    nodes: Vec<GraphNode>,
}

impl MockGraphRepository {
    fn new(nodes: Vec<GraphNode>) -> Self {
        Self { nodes }
    }
}

impl GraphRepository for MockGraphRepository {
    fn search(
        &self,
        _query: &str,
        _node_kinds: &[NodeKind],
        _limit: usize,
        _cursor: Option<&str>,
    ) -> cognicode_core::domain::GraphResult<cognicode_core::domain::ports::graph_repository::SearchPage> {
        Ok(cognicode_core::domain::ports::graph_repository::SearchPage {
            items: Vec::new(),
            raw_total: 0,
            next_cursor: None,
            raw_rank: 0.0,
            item_ranks: Vec::new(),
        })
    }

    fn find_nodes_by_kind(&self, kind: &NodeKind) -> cognicode_core::domain::GraphResult<Vec<GraphNode>> {
        Ok(self.nodes.iter().filter(|n| &n.kind == kind).cloned().collect())
    }

    fn get_node(&self, _id: &NodeId) -> cognicode_core::domain::GraphResult<Option<GraphNode>> {
        Ok(None)
    }

    fn find_outgoing_edges(&self, _id: &NodeId) -> cognicode_core::domain::GraphResult<Vec<cognicode_core::domain::aggregates::generic_graph::GraphEdge>> {
        Ok(Vec::new())
    }

    fn edges_by_kind(&self, _node: &NodeId, _kinds: &[cognicode_core::domain::value_objects::edge_kind::EdgeKind]) -> cognicode_core::domain::GraphResult<Vec<cognicode_core::domain::aggregates::generic_graph::GraphEdge>> {
        Ok(Vec::new())
    }

    fn rationale_subgraph(&self, _focus: &NodeId, _max_depth: u32, _max_nodes: usize) -> cognicode_core::domain::GraphResult<(Vec<GraphNode>, Vec<cognicode_core::domain::aggregates::generic_graph::GraphEdge>, bool)> {
        Ok((Vec::new(), Vec::new(), false))
    }
}

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
fn default_find_nodes_by_kind_paginated_returns_raw_total_equals_items_len() {
    let nodes = vec![
        make_node("doc:1", NodeKind::Doc, "Design Doc 1"),
        make_node("doc:2", NodeKind::Doc, "Design Doc 2"),
        make_node("doc:3", NodeKind::Doc, "Design Doc 3"),
    ];
    let repo = Arc::new(MockGraphRepository::new(nodes));

    let result = repo.find_nodes_by_kind_paginated(&NodeKind::Doc, 10, None);
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);

    let page = result.unwrap();
    assert_eq!(page.items.len(), 3, "Expected 3 items");
    assert_eq!(page.raw_total, 3, "Expected raw_total == 3 (items.len())");
}

#[test]
fn default_find_nodes_by_kind_paginated_returns_none_cursor() {
    let nodes = vec![
        make_node("doc:1", NodeKind::Doc, "Design Doc 1"),
        make_node("doc:2", NodeKind::Doc, "Design Doc 2"),
    ];
    let repo = Arc::new(MockGraphRepository::new(nodes));

    let result = repo.find_nodes_by_kind_paginated(&NodeKind::Doc, 10, None);
    assert!(result.is_ok());

    let page = result.unwrap();
    assert!(page.next_cursor.is_none(), "Expected next_cursor = None in default impl");
}

#[test]
fn default_find_nodes_by_kind_paginated_respects_kind_filter() {
    let nodes = vec![
        make_node("doc:1", NodeKind::Doc, "Design Doc"),
        make_node("dec:1", NodeKind::Decision, "ADR 1"),
        make_node("ev:1", NodeKind::Evidence, "Evidence 1"),
    ];
    let repo = Arc::new(MockGraphRepository::new(nodes));

    let result = repo.find_nodes_by_kind_paginated(&NodeKind::Decision, 10, None);
    assert!(result.is_ok());

    let page = result.unwrap();
    assert_eq!(page.items.len(), 1, "Expected 1 Decision node");
    assert_eq!(page.items[0].kind, NodeKind::Decision);
}

#[test]
fn default_search_paginated_empty_query_returns_empty_page() {
    let nodes = vec![
        make_node("doc:1", NodeKind::Doc, "Design Doc"),
    ];
    let repo = Arc::new(MockGraphRepository::new(nodes));

    // Empty query should return empty page per contract
    let result = repo.search_paginated("", &[NodeKind::Doc], 10, None);
    assert!(result.is_ok());

    let page = result.unwrap();
    assert!(page.items.is_empty(), "Empty query should return empty page");
    assert_eq!(page.raw_total, 0, "raw_total should be 0 for empty query");
    assert!(page.next_cursor.is_none());
}
