//! `GraphRepository` — domain port for the Generic Graph Layer.
//!
//! Defines the contract the `graph_search` MCP tool (T21) needs to
//! query multimodal nodes + edges. The canonical adapter (in the
//! `cognicode-explorer` crate) implements this trait on top of
//! concrete adapters; an in-memory mock (also in the explorer)
//! is used by the unit tests.
//!
//! Why a separate port? The Generic Graph Layer has different
//! primary keys (`graph_nodes(id, kind)`) and different query
//! patterns (full-text search over `label || metadata`) than the existing
//! `SymbolRepository`. Forcing both onto a single trait would
//! create a fat-interface smell.
//!
//! All methods are `Send + Sync` so the trait object can be
//! shared across MCP worker threads.
//!
//! Available in the default build. The `multimodal` feature gates
//! the WRITE/exTRACTION path only; the read port is always present.
//!
//! ## Async Migration (ADR-NNN)
//!
//! This trait was migrated from synchronous to asynchronous to eliminate
//! `tokio::runtime::Handle::current().block_on` anti-patterns in the PG
//! adapter. See design obs #4419 for the full rationale. The migration
//! preserves `Send + Sync` and all method signatures verbatim; only the
//! calling convention changed from sync `fn` to `async fn` with `#[async_trait]`.

use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use crate::domain::aggregates::SymbolId;
use crate::domain::example_block::ExampleBlock;
use crate::domain::ports::graph_error::GraphResult;
use crate::domain::value_objects::edge_kind::EdgeKind;
use crate::domain::value_objects::node_kind::NodeKind;

use async_trait::async_trait;

/// One page of a search result. The cursor is opaque (a base64
/// string the tool passes back unchanged); the page's `items` are
/// the matching `GraphNode`s and `raw_total` is the total number
/// of matches in the index (NOT just this page).
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
    /// The raw relevance score (the underlying search backend's relevance score
    /// as a positive float) of the top item on the page. The MCP tool
    /// surfaces this alongside the normalised score per the IB
    /// check in `design.md`. Kept for backward compatibility —
    /// the per-item scores live in `item_ranks` and are
    /// preferred when present.
    pub raw_rank: f64,
    /// Per-item raw ranks, parallel to `items` (so
    /// `item_ranks.len() == items.len()`). The MCP tool uses
    /// this to emit a distinct `score` per result. `Vec::new()`
    /// when the underlying search backend does not surface
    /// per-item ranks (e.g. an unimplemented search stub); the
    /// caller then falls back to `raw_rank` for every item.
    pub item_ranks: Vec<f64>,
}

/// Read-only port for the Generic Graph Layer.
#[async_trait]
pub trait GraphRepository: Send + Sync {
    /// Full-text search across `graph_nodes`. Returns at most
    /// `limit` items, paginated by the opaque `cursor` (start at
    /// the beginning when `None`).
    ///
    /// When `node_kinds` is non-empty, only nodes whose kind
    /// appears in the filter are returned. An empty `query` MUST
    /// return an empty page (no errors).
    async fn search(
        &self,
        query: &str,
        node_kinds: &[NodeKind],
        limit: usize,
        cursor: Option<&str>,
    ) -> GraphResult<SearchPage>;

    /// Find all nodes of a given kind. Used by ExplorerQL
    /// `FIND decisions` / `FIND docs` (T20) dispatch.
    async fn find_nodes_by_kind(&self, kind: &NodeKind) -> GraphResult<Vec<GraphNode>>;

    /// Find a single node by its `NodeId`. Returns `Ok(None)` when
    /// the id is not in the index.
    async fn get_node(&self, id: &NodeId) -> GraphResult<Option<GraphNode>>;

    /// Find all edges whose source equals `id`.
    async fn find_outgoing_edges(&self, id: &NodeId) -> GraphResult<Vec<GraphEdge>>;

    /// Find edges from `node` that match any of the given `kinds`.
    /// Edges are deduplicated on `(source, target, kind)`, keeping the
    /// highest confidence for duplicate tuples.
    async fn edges_by_kind(&self, node: &NodeId, kinds: &[EdgeKind])
    -> GraphResult<Vec<GraphEdge>>;

    /// Return code usage examples for a symbol.
    ///
    /// Returns example blocks that demonstrate how the given symbol is used,
    /// tested, or benchmarked. Used by the ExampleObject view to populate
    /// the narrative runtime.
    ///
    /// Default implementation returns an empty vector — the LadybugDB adapter
    /// provides the production implementation.
    async fn example_blocks_for_symbol(&self, _symbol_id: &SymbolId) -> GraphResult<Vec<ExampleBlock>> {
        Ok(Vec::new())
    }

    /// BFS traversal of the multimodal sub-graph from `focus`, following
    /// only multimodal edges (Justifies, Cites, Resolves, CorroboratedBy).
    ///
    /// Returns `(nodes, edges, truncated)` where `truncated` is `true`
    /// when the traversal stopped early because the reachable set
    /// exceeded `max_nodes`. The traversal is bounded by `max_depth`
    /// and `max_nodes`. When truncation kicks in, edges with missing
    /// endpoints are dropped.
    async fn rationale_subgraph(
        &self,
        focus: &NodeId,
        max_depth: u32,
        max_nodes: usize,
    ) -> GraphResult<(Vec<GraphNode>, Vec<GraphEdge>, bool)>;

    /// Find nodes of a given kind with pagination support.
    ///
    /// Default implementation delegates to `find_nodes_by_kind` and wraps
    /// the result in a [`SearchPage`] without real pagination
    /// (`next_cursor = None`, `raw_total = items.len()`).
    async fn find_nodes_by_kind_paginated(
        &self,
        kind: &NodeKind,
        limit: usize,
        cursor: Option<&str>,
    ) -> GraphResult<SearchPage> {
        let items = self.find_nodes_by_kind(kind).await?;
        let raw_total = items.len() as u64;
        Ok(SearchPage {
            items,
            raw_total,
            next_cursor: None,
            raw_rank: 0.0,
            item_ranks: Vec::new(),
        })
    }

    /// Full-text search with pagination support.
    ///
    /// Default implementation delegates to `search`. The cursor format
    /// is opaque; implementations that override this method should handle
    /// the cursor encoding. An empty `query` MUST return an empty page
    /// (no errors).
    async fn search_paginated(
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
        self.search(query, kinds, limit, cursor).await
    }
}
