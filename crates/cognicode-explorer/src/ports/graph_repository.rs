//! `GraphRepository` — domain port for the Generic Graph Layer.
//!
//! Defines the contract the `graph_search` MCP tool (T21) needs to
//! query multimodal nodes + edges. The PG adapter (when the
//! `postgres` feature is on) implements this trait on top of
//! `PostgresRepository`; an in-memory mock is used by the unit tests.
//!
//! Why a separate port? The Generic Graph Layer has different
//! primary keys (`graph_nodes(id, kind)`) and different query
//! patterns (FTS5 over `label || metadata`) than the existing
//! `SymbolRepository`. Forcing both onto a single trait would
//! create a fat-interface smell.
//!
//! All methods are `Send + Sync` so the trait object can be
//! shared across MCP worker threads.

#[cfg(feature = "multimodal")]
use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
#[cfg(feature = "multimodal")]
use cognicode_core::domain::value_objects::node_kind::NodeKind;

#[cfg(feature = "multimodal")]
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;

use crate::error::ExplorerResult;

/// One page of a search result. The cursor is opaque (a base64
/// string the tool passes back unchanged); the page's `items` are
/// the matching `GraphNode`s and `raw_total` is the total number
/// of matches in the index (NOT just this page).
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPage {
    pub items: Vec<GraphNode>,
    /// The total number of matches in the index (NOT the size of
    /// the current page). The MCP tool surfaces this as
    /// `total_count`.
    pub raw_total: u64,
    /// Opaque cursor for the next page. `None` on the last page
    /// (and on the only page of a small result set).
    pub next_cursor: Option<String>,
    /// The raw FTS5 rank (the `ts_rank_cd` value as a positive
    /// float) of the top item on the page. The MCP tool
    /// surfaces this alongside the normalised score per the IB
    /// check in `design.md`. Kept for backward compatibility —
    /// the per-item scores live in `item_ranks` and are
    /// preferred when present.
    pub raw_rank: f64,
    /// Per-item raw ranks, parallel to `items` (so
    /// `item_ranks.len() == items.len()`). The MCP tool uses
    /// this to emit a distinct `score` per result. `Vec::new()`
    /// when the underlying search backend does not surface
    /// per-item ranks (e.g. an unimplemented PG stub); the
    /// caller then falls back to `raw_rank` for every item.
    pub item_ranks: Vec<f64>,
}

/// Read-only port for the Generic Graph Layer.
#[cfg(feature = "multimodal")]
pub trait GraphRepository: Send + Sync {
    /// FTS5-backed search across `graph_nodes`. Returns at most
    /// `limit` items, paginated by the opaque `cursor` (start at
    /// the beginning when `None`).
    ///
    /// When `node_kinds` is non-empty, only nodes whose kind
    /// appears in the filter are returned. An empty `query` MUST
    /// return an empty page (no errors).
    fn search(
        &self,
        query: &str,
        node_kinds: &[NodeKind],
        limit: usize,
        cursor: Option<&str>,
    ) -> ExplorerResult<SearchPage>;

    /// Find all nodes of a given kind. Used by ExplorerQL
    /// `FIND decisions` / `FIND docs` (T20) dispatch.
    fn find_nodes_by_kind(&self, kind: &NodeKind) -> ExplorerResult<Vec<GraphNode>>;

    /// Find a single node by its `NodeId`. Returns `Ok(None)` when
    /// the id is not in the index.
    fn get_node(&self, id: &NodeId) -> ExplorerResult<Option<GraphNode>>;

    /// Find all edges whose source equals `id`.
    fn find_outgoing_edges(&self, id: &NodeId) -> ExplorerResult<Vec<GraphEdge>>;

    /// Find edges from `node` that match any of the given `kinds`.
    /// Edges are deduplicated on `(source, target, kind)`, keeping the
    /// highest confidence for duplicate tuples.
    #[cfg(feature = "multimodal")]
    fn edges_by_kind(
        &self,
        node: &NodeId,
        kinds: &[EdgeKind],
    ) -> ExplorerResult<Vec<GraphEdge>>;

    /// BFS traversal of the multimodal sub-graph from `focus`, following
    /// only multimodal edges (Justifies, Cites, Resolves, CorroboratedBy).
    ///
    /// Returns `(nodes, edges, truncated)` where `truncated` is `true`
    /// when the traversal stopped early because the reachable set
    /// exceeded `max_nodes`. The traversal is bounded by `max_depth`
    /// and `max_nodes`. When truncation kicks in, edges with missing
    /// endpoints are dropped.
    #[cfg(feature = "multimodal")]
    fn rationale_subgraph(
        &self,
        focus: &NodeId,
        max_depth: u32,
        max_nodes: usize,
    ) -> ExplorerResult<(Vec<GraphNode>, Vec<GraphEdge>, bool)>;
}
