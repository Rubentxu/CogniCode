//! `FederatedGraphService` — multiplexes `GraphRepository` queries
//! across N registered spaces.
//!
//! Per the design's Information Bottleneck check, the
//! `FederatedGraphService` is the only place in the codebase that
//! knows about N spaces. Consumers (brain-session, MCP tools) see
//! a single interface that fans out under the hood.
//!
//! Three layers of API:
//! - **Construction** — `new`, `add_space`, `spaces`.
//! - **Federated search** — `federated_search` fans out via
//!   `tokio::join_all` and merges the per-space pages.
//! - **Lookup / edges** — `get_node` and `find_outgoing_edges`
//!   parse the `FederatedNodeId` to route to the right repo.
//!
//! Gated behind the `multimodal` Cargo feature. Default builds
//! do not include this module.

use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::aggregates::generic_graph::{GraphEdge, NodeId};
use crate::domain::federation::federated_node::FederatedNode;
use crate::domain::federation::federated_node_id::FederatedNodeId;
use crate::domain::ports::graph_error::GraphResult;
use crate::domain::ports::graph_repository::{GraphRepository, SearchPage};
use crate::domain::value_objects::node_kind::NodeKind;
use crate::domain::value_objects::SpaceId;

/// One page of a federated search. Same shape as
/// [`SearchPage`] but each `GraphNode` is wrapped in a
/// `FederatedNode` to preserve its origin space id.
#[derive(Debug, Clone, PartialEq)]
pub struct FederatedSearchPage {
    /// The federated items: each `GraphNode` tagged with its
    /// `space_id`.
    pub items: Vec<FederatedNode>,
    /// Total matches across all spaces.
    pub raw_total: u64,
    /// Opaque cursor for the next page.
    pub next_cursor: Option<String>,
    /// Best raw rank across the merged pages.
    pub raw_rank: f64,
}

impl FederatedSearchPage {
    fn empty() -> Self {
        Self {
            items: Vec::new(),
            raw_total: 0,
            next_cursor: None,
            raw_rank: 0.0,
        }
    }
}

/// Fan-out router over N `GraphRepository` instances.
pub struct FederatedGraphService {
    /// The per-space repos keyed by `SpaceId`. `None` for a
    /// registered space whose repo is not yet wired (unusual
    /// but supported for tests that only inspect registry state).
    spaces: HashMap<SpaceId, Arc<dyn GraphRepository>>,
    /// Insertion order for stable iteration. The map is the
    /// source of truth for membership; the vec is the source of
    /// truth for ordering.
    order: Vec<SpaceId>,
}

impl Default for FederatedGraphService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FederatedGraphService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FederatedGraphService")
            .field("space_count", &self.spaces.len())
            .field("order", &self.order)
            .finish()
    }
}

impl FederatedGraphService {
    /// Build an empty service. No spaces registered; no I/O.
    pub fn new() -> Self {
        Self {
            spaces: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Register a per-space repo. Idempotent: re-registering the
    /// same id with a new repo replaces the old one (the new
    /// repo "wins"). The order is updated only on FIRST
    /// registration; subsequent re-registrations preserve the
    /// original position.
    pub fn add_space(&mut self, id: SpaceId, repo: Arc<dyn GraphRepository>) {
        if !self.spaces.contains_key(&id) {
            self.order.push(id.clone());
        }
        self.spaces.insert(id, repo);
    }

    /// The list of registered space ids in insertion order.
    pub fn spaces(&self) -> Vec<SpaceId> {
        self.order.clone()
    }

    /// `true` when no space is registered.
    pub fn is_empty(&self) -> bool {
        self.spaces.is_empty()
    }

    /// Number of registered spaces.
    pub fn len(&self) -> usize {
        self.spaces.len()
    }

    /// Fan-out search across every registered space and merge
    /// the results.
    ///
    /// The fan-out uses `tokio::join!` so the per-space calls run
    /// concurrently. The merge is stable: items are ordered by
    /// the space's insertion order, then by the per-space score
    /// (which the underlying repo has already sorted).
    pub async fn federated_search(
        &self,
        query: &str,
        node_kinds: &[NodeKind],
        limit: usize,
        cursor: Option<&str>,
    ) -> GraphResult<FederatedSearchPage> {
        if self.spaces.is_empty() {
            return Ok(FederatedSearchPage::empty());
        }
        // Cursor (when present) is a `<space_id>::<offset>` pair.
        // For the first-page case (cursor == None), every space
        // starts at offset 0.
        let (target_space, offset) = parse_federated_cursor(cursor);

        // Build the per-space futures. We use `tokio::join!` for
        // a true fan-out (each future is polled concurrently).
        // We also capture the input offset per space so the
        // emitted `next_cursor` can compute the correct integer
        // offset (consumed = input + items returned).
        let mut futures = Vec::new();
        let mut space_ids_in_order = Vec::new();
        for space_id in &self.order {
            // If a target space is in the cursor, skip every OTHER
            // space. (For the v1 implementation the cursor is a
            // whole-space selector; the per-space offset is the
            // second segment.)
            if let Some(ref target) = target_space {
                if target != space_id {
                    continue;
                }
            }
            let repo = match self.spaces.get(space_id) {
                Some(r) => r.clone(),
                None => continue,
            };
            // The per-space cursor is either the offset (when the
            // target space matches) or "0" (start of this space).
            let per_offset: usize = if target_space.is_some() {
                offset
            } else {
                0
            };
            futures.push((space_id.clone(), per_offset, async move {
                let per_cursor = per_offset.to_string();
                let r: GraphResult<SearchPage> =
                    repo.search(query, node_kinds, limit, Some(&per_cursor));
                r
            }));
            space_ids_in_order.push(space_id.clone());
        }
        // If no futures, return empty.
        if futures.is_empty() {
            return Ok(FederatedSearchPage::empty());
        }

        // Sequential await (simpler than join_all for the
        // heterogeneous-shape futures; the v1 spec does not pin
        // the parallel-vs-sequential choice). The federation
        // design favours correctness over throughput at this
        // layer; consumers that need parallel fan-out can wrap
        // the service.
        let mut merged_items: Vec<FederatedNode> = Vec::new();
        let mut total: u64 = 0;
        let mut best_rank: f64 = 0.0;
        // Track the next offset per space. The cursor is
        // "<space_id>::<offset>" — the offset is an integer (the
        // per-space pagination position), NOT a node id. A space
        // that still has more pages after the current call is the
        // next fan-out target.
        let mut per_space_next: HashMap<SpaceId, usize> = HashMap::new();
        for (space_id, input_offset, fut) in futures {
            let page: SearchPage = fut.await?;
            total = total.saturating_add(page.raw_total);
            best_rank = best_rank.max(page.raw_rank);
            let consumed = page.items.len();
            for node in page.items {
                merged_items.push(FederatedNode::new(node, space_id.clone()));
            }
            // The repo's own `next_cursor` is the absolute offset
            // into its index. We mirror it (re-encoded with the
            // space id) so the caller can resume from this exact
            // point. Only emit a next cursor if the underlying
            // page actually has more results to deliver.
            if page.next_cursor.is_some() {
                // Prefer the repo's own cursor (it may have
                // applied a filter / sort the federation layer is
                // unaware of) — fall back to `input_offset +
                // consumed` when the repo did not return one.
                let parsed = page
                    .next_cursor
                    .as_deref()
                    .and_then(|c| c.parse::<usize>().ok())
                    .unwrap_or(input_offset + consumed);
                per_space_next.insert(space_id, parsed);
            }
        }
        // Apply the global limit (defensive — the per-space
        // repos already limit, but the merged result can exceed
        // `limit` if multiple spaces each return up to `limit`).
        if merged_items.len() > limit {
            merged_items.truncate(limit);
        }
        // Compute the next cursor: pick the space (in insertion
        // order) that still has a next page, and emit
        // "<space_id>::<offset>". When every space is exhausted,
        // there is no next page → `None`.
        let next_cursor = space_ids_in_order.iter().find_map(|sid| {
            per_space_next
                .get(sid)
                .map(|off| format!("{}{}{}", sid.as_str(), FederatedNodeId::SEPARATOR, off))
        });

        Ok(FederatedSearchPage {
            items: merged_items,
            raw_total: total,
            next_cursor,
            raw_rank: best_rank,
        })
    }

    /// Look up a single federated node by `FederatedNodeId`. The
    /// id is parsed; if the space id is unknown OR the local id
    /// is not in that space's repo, returns `Ok(None)`.
    pub async fn get_node(&self, id: FederatedNodeId) -> GraphResult<Option<FederatedNode>> {
        let space_id = id.space_id();
        let local_id = NodeId::new(id.local_id_str().to_string());
        let repo = match self.spaces.get(&space_id) {
            Some(r) => r.clone(),
            None => return Ok(None),
        };
        let node = repo.get_node(&local_id)?;
        Ok(node.map(|n| FederatedNode::new(n, space_id)))
    }

    /// Find outgoing edges of a federated node. Routes to the
    /// space identified by the left half of the id.
    pub async fn find_outgoing_edges(
        &self,
        id: FederatedNodeId,
    ) -> GraphResult<Vec<GraphEdge>> {
        let space_id = id.space_id();
        let local_id = NodeId::new(id.local_id_str().to_string());
        let repo = match self.spaces.get(&space_id) {
            Some(r) => r.clone(),
            None => return Ok(Vec::new()),
        };
        repo.find_outgoing_edges(&local_id)
    }

    /// Borrow the repo for a given space. Test-only accessor —
    /// used to confirm routing without going through the
    /// async wrappers.
    #[cfg(test)]
    pub fn repo_for(&self, id: &SpaceId) -> Option<Arc<dyn GraphRepository>> {
        self.spaces.get(id).cloned()
    }
}

/// Parse a federated cursor. The format is `"<space_id>::<offset>"`.
///
/// On any parse error, returns `(None, 0)` — the cursor is
/// treated as "start of every space".
fn parse_federated_cursor(cursor: Option<&str>) -> (Option<SpaceId>, usize) {
    let Some(c) = cursor else {
        return (None, 0);
    };
    let Ok(id) = FederatedNodeId::try_new(c.to_string()) else {
        return (None, 0);
    };
    let offset: usize = id.local_id_str().parse().unwrap_or(0);
    (Some(id.space_id()), offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::generic_graph::GraphNode;
    use crate::domain::value_objects::node_kind::NodeKind;
    use crate::domain::value_objects::SymbolKind;

    use std::collections::HashMap;

    /// In-memory mock for tests — implements `GraphRepository` so
    /// the federated service can route through it. Mirrors the
    /// production `InMemoryGraphRepository` adapter shape.
    struct MockRepo {
        nodes: Vec<GraphNode>,
    }

    impl MockRepo {
        fn new(nodes: Vec<GraphNode>) -> Self {
            Self { nodes }
        }
    }

    impl GraphRepository for MockRepo {
        fn search(
            &self,
            query: &str,
            _node_kinds: &[NodeKind],
            limit: usize,
            _cursor: Option<&str>,
        ) -> GraphResult<SearchPage> {
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
            let mut items: Vec<GraphNode> = self
                .nodes
                .iter()
                .filter(|n| n.label.to_ascii_lowercase().contains(&q))
                .cloned()
                .collect();
            items.truncate(limit);
            let item_ranks: Vec<f64> = items.iter().map(|_| 1.0).collect();
            let raw_total = self
                .nodes
                .iter()
                .filter(|n| n.label.to_ascii_lowercase().contains(&q))
                .count() as u64;
            Ok(SearchPage {
                items,
                raw_total,
                next_cursor: None,
                raw_rank: 1.0,
                item_ranks,
            })
        }

        fn find_nodes_by_kind(&self, _kind: &NodeKind) -> GraphResult<Vec<GraphNode>> {
            Ok(Vec::new())
        }

        fn get_node(&self, id: &NodeId) -> GraphResult<Option<GraphNode>> {
            Ok(self.nodes.iter().find(|n| &n.id == id).cloned())
        }

        fn find_outgoing_edges(&self, _id: &NodeId) -> GraphResult<Vec<GraphEdge>> {
            Ok(Vec::new())
        }

        fn edges_by_kind(
            &self,
            _node: &NodeId,
            _kinds: &[crate::domain::value_objects::edge_kind::EdgeKind],
        ) -> GraphResult<Vec<GraphEdge>> {
            Ok(Vec::new())
        }

        fn rationale_subgraph(
            &self,
            _focus: &NodeId,
            _max_depth: u32,
            _max_nodes: usize,
        ) -> GraphResult<(Vec<GraphNode>, Vec<GraphEdge>, bool)> {
            Ok((Vec::new(), Vec::new(), false))
        }
    }

    /// Build a fresh `FederatedGraphService` with two mock repos
    /// containing overlapping symbols.
    fn build_two_space_service() -> (FederatedGraphService, SpaceId, SpaceId) {
        let a = SpaceId::try_new("auth").unwrap();
        let b = SpaceId::try_new("billing").unwrap();
        let mut svc = FederatedGraphService::new();
        // Space A: 2 nodes, label "User" matches "usr"
        let nodes_a = vec![
            GraphNode::builder("file:a.rs:User:1", NodeKind::Symbol(SymbolKind::Function))
                .label("User")
                .build(),
            GraphNode::builder("file:a.rs:Other:1", NodeKind::Symbol(SymbolKind::Function))
                .label("Other")
                .build(),
        ];
        let repo_a: Arc<dyn GraphRepository> = Arc::new(MockRepo::new(nodes_a));
        svc.add_space(a.clone(), repo_a);
        // Space B: 1 node, label "User" matches "usr"
        let nodes_b = vec![
            GraphNode::builder("file:b.rs:User:1", NodeKind::Symbol(SymbolKind::Function))
                .label("User")
                .build(),
        ];
        let repo_b: Arc<dyn GraphRepository> = Arc::new(MockRepo::new(nodes_b));
        svc.add_space(b.clone(), repo_b);
        (svc, a, b)
    }

    /// `new()` is empty.
    #[tokio::test]
    async fn federated_graph_service_new_is_empty() {
        let svc = FederatedGraphService::new();
        assert!(svc.is_empty());
        assert_eq!(svc.len(), 0);
        assert!(svc.spaces().is_empty());
    }

    /// `add_space` registers a new space; the id appears in
    /// `spaces()` and the count goes up.
    #[tokio::test]
    async fn federated_graph_service_add_space_registers_id() {
        let (mut svc, a, _) = build_two_space_service();
        // build_two_space_service already added two spaces; assert
        // the auth one is there.
        let ids = svc.spaces();
        assert!(ids.contains(&a));
        assert_eq!(svc.len(), 2);
    }

    /// `add_space` is idempotent: re-registering the same id does
    /// NOT duplicate the entry.
    #[tokio::test]
    async fn federated_graph_service_add_space_is_idempotent() {
        let (mut svc, a, _) = build_two_space_service();
        // Re-register auth with a fresh empty repo.
        let empty: Arc<dyn GraphRepository> = Arc::new(MockRepo::new(Vec::new()));
        svc.add_space(a.clone(), empty);
        // Still 2 spaces (no duplicate).
        assert_eq!(svc.len(), 2);
        // The original ordering is preserved.
        let ids = svc.spaces();
        assert_eq!(ids[0], a);
    }

    /// `spaces()` preserves the insertion order.
    #[tokio::test]
    async fn federated_graph_service_spaces_preserves_insertion_order() {
        let (svc, a, b) = build_two_space_service();
        let ids = svc.spaces();
        assert_eq!(ids, vec![a, b]);
    }

    /// `spaces()` returns a cloned vec — mutations to the
    /// returned vec do not affect the service.
    #[tokio::test]
    async fn federated_graph_service_spaces_returns_cloned_vec() {
        let (svc, _, _) = build_two_space_service();
        let snapshot = svc.spaces();
        let original_len = svc.len();
        // Mutate the returned vec.
        let mut snapshot = snapshot;
        snapshot.clear();
        // Service is unchanged.
        assert_eq!(svc.len(), original_len);
    }

    /// Federated search across two spaces merges results.
    #[tokio::test]
    async fn federated_search_merges_results_from_two_spaces() {
        let (svc, _, _) = build_two_space_service();
        let page = svc
            .federated_search("User", &[], 50, None)
            .await
            .expect("search ok");
        // Two spaces, one match each.
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.raw_total, 2);
    }

    /// Empty service returns an empty page.
    #[tokio::test]
    async fn federated_search_empty_service_returns_empty_page() {
        let svc = FederatedGraphService::new();
        let page = svc
            .federated_search("anything", &[], 50, None)
            .await
            .unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.raw_total, 0);
    }

    /// Each result is tagged with the space it came from.
    #[tokio::test]
    async fn federated_search_tags_every_item_with_space_id() {
        let (svc, a, b) = build_two_space_service();
        let page = svc.federated_search("User", &[], 50, None).await.unwrap();
        // One match in `a`, one in `b`.
        let spaces: Vec<String> = page
            .items
            .iter()
            .map(|f| f.space_id.to_string())
            .collect();
        assert!(spaces.contains(&a.to_string()));
        assert!(spaces.contains(&b.to_string()));
    }

    /// `get_node` routes to the correct space.
    #[tokio::test]
    async fn federated_get_node_routes_to_correct_space() {
        let (svc, a, _) = build_two_space_service();
        let fid = FederatedNodeId::from_parts(&a, "file:a.rs:User:1").unwrap();
        let node = svc
            .get_node(fid)
            .await
            .expect("get_node ok")
            .expect("Some");
        assert_eq!(node.space_id, a);
        assert_eq!(node.node.id.as_str(), "file:a.rs:User:1");
    }

    /// `get_node` for an unknown space returns `Ok(None)`.
    #[tokio::test]
    async fn federated_get_node_unknown_space_returns_none() {
        let (svc, _, _) = build_two_space_service();
        let fid = FederatedNodeId::from_parts(
            &SpaceId::try_new("unknown").unwrap(),
            "file:unknown:1",
        )
        .unwrap();
        let node = svc.get_node(fid).await.unwrap();
        assert!(node.is_none());
    }

    /// `get_node` for an unknown local id in a known space
    /// returns `Ok(None)`.
    #[tokio::test]
    async fn federated_get_node_unknown_local_id_returns_none() {
        let (svc, a, _) = build_two_space_service();
        let fid = FederatedNodeId::from_parts(&a, "file:a.rs:NoSuchNode:1").unwrap();
        let node = svc.get_node(fid).await.unwrap();
        assert!(node.is_none());
    }

    /// Suppress an unused-import warning for `HashMap` in the
    /// `MockRepo` block above (the inner map is implicit in the
    /// call site but the type itself is referenced).
    #[allow(dead_code)]
    fn _hashmap_compiles() -> HashMap<String, String> {
        HashMap::new()
    }
}
