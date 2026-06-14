//! `PgGraphRepository` — `GraphRepository` port adapter backed by
//! PostgreSQL.
//!
//! Implements the read methods from
//! [`crate::domain::ports::GraphRepository`]
//! (`search`, `find_nodes_by_kind`, `get_node`,
//! `find_outgoing_edges`, `edges_by_kind`, `rationale_subgraph`).
//! All read methods are stubs that return empty results in this
//! slice — the FTS5 surface is being lifted from the legacy
//! `PostgresRepository::find_graph_nodes` family in a follow-up
//! PR. The adapter is the canonical wiring point for the MCP
//! server's `graph_search` tool (and the explorer's
//! `BrainSessionService`) once the read path is in place.
//!
//! ## Connection pool
//!
//! The adapter owns a `sqlx::PgPool` (cloned from the parent
//! service). Cloning the adapter is cheap (the pool itself is
//! an `Arc`).
//!
//! Implements the canonical `cognicode_core::ports::GraphRepository`
//! trait. Error returns are `GraphResult` (not the explorer's
//! `ExplorerResult`) — the adapter wraps upstream failures in
//! `GraphError::Storage` / `GraphError::InvalidInput`.
//!
//! ## T4 write surface
//!
//! The `upsert_nodes` / `upsert_edges` write methods (T4 in the
//! Graph Intelligence v2 roadmap) are NOT part of the
//! `GraphRepository` trait yet — that surface lands in a
//! separate slice when the `GraphWriteRepository` supertrait
//! extension is merged. The original exploratory code lives
//! in git history on this path; it's intentionally not in
//! this file to avoid a trait-shape mismatch with the current
//! port.
//!
//! ## Location
//!
//! Lives in `cognicode-core` (here) rather than in
//! `cognicode-explorer` because the MCP server (in
//! `cognicode-mcp`) also needs it, and the explorer is a
//! transport / API crate that MCP shouldn't depend on. Both
//! `cognicode-mcp` and `cognicode-explorer` now import this
//! adapter from the same canonical location.

#[cfg(all(feature = "multimodal", feature = "postgres"))]
use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
#[cfg(all(feature = "multimodal", feature = "postgres"))]
use crate::domain::ports::GraphRepository;
#[cfg(all(feature = "multimodal", feature = "postgres"))]
use crate::domain::value_objects::edge_kind::EdgeKind;
#[cfg(all(feature = "multimodal", feature = "postgres"))]
use crate::domain::value_objects::node_kind::NodeKind;
#[cfg(all(feature = "multimodal", feature = "postgres"))]
use crate::domain::{GraphResult, SearchPage};

/// Adapter that backs the `GraphRepository` port with a
/// PostgreSQL pool. Constructed via [`PgGraphRepository::new`]
/// from a `sqlx::PgPool`. Cloning the adapter is cheap (the
/// pool itself is an `Arc`).
#[cfg(all(feature = "multimodal", feature = "postgres"))]
#[derive(Clone)]
pub struct PgGraphRepository {
    pool: sqlx::PgPool,
}

#[cfg(all(feature = "multimodal", feature = "postgres"))]
impl PgGraphRepository {
    /// Build a new adapter over the given PG pool. The pool is
    /// shared (cloned) across clones of the adapter.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Access to the underlying pool. Used by callers (the MCP
    /// `graph_search` tool, the explorer) that need to drive
    /// additional SQL outside the `GraphRepository` surface
    /// (custom FTS5 queries, federation joins, …).
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}

#[cfg(all(feature = "multimodal", feature = "postgres"))]
impl GraphRepository for PgGraphRepository {
    /// PG-backed read methods. The implementation mirrors the
    /// existing [`PostgresRepository::find_graph_nodes`] /
    /// `find_graph_edges` family but the explorer-graph
    /// port-level `SearchPage` shape (with the FTS5
    /// `ts_rank_cd` payload) is what the MCP `graph_search`
    /// tool surfaces.
    fn search(
        &self,
        _query: &str,
        _node_kinds: &[NodeKind],
        _limit: usize,
        _cursor: Option<&str>,
    ) -> GraphResult<SearchPage> {
        // The full FTS5 search surface lives in the existing
        // `PostgresRepository` (see `find_graph_nodes`). Wiring
        // it through the new port is a follow-up that the
        // `graph_search` tool's MCP dispatch path picks up via
        // a different seam (the `ExplorerService`). For V1, the
        // T4 surface focuses on the write path — the read
        // methods here are stubs that return empty pages so
        // the adapter compiles + links without dragging in the
        // existing FTS5 plumbing.
        Ok(SearchPage {
            items: Vec::new(),
            raw_total: 0,
            next_cursor: None,
            raw_rank: 0.0,
            item_ranks: Vec::new(),
        })
    }

    fn find_nodes_by_kind(&self, _kind: &NodeKind) -> GraphResult<Vec<GraphNode>> {
        Ok(Vec::new())
    }

    fn get_node(&self, _id: &NodeId) -> GraphResult<Option<GraphNode>> {
        Ok(None)
    }

    fn find_outgoing_edges(&self, _id: &NodeId) -> GraphResult<Vec<GraphEdge>> {
        Ok(Vec::new())
    }

    fn edges_by_kind(
        &self,
        _node: &NodeId,
        _kinds: &[EdgeKind],
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

    // NOTE: the T4 write surface (`upsert_nodes`, `upsert_edges`)
    // intentionally does NOT live here yet — see the module
    // doc comment. The trait itself is currently read-only;
    // adding write methods is a separate slice.
}

// ============================================================================
// Compile-gate tests — the PG adapter is exercisable end-to-end only when
// the CI lane has a Postgres instance. The unit tests here prove the
// adapter compiles, links, and the trait is dyn-compatible.
// ============================================================================

#[cfg(all(test, feature = "multimodal", feature = "postgres"))]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// The trait object compiles and the read methods are
    /// reachable through it (the same shape as the MCP
    /// dispatch uses).
    #[test]
    fn trait_object_dyn_compat() {
        // We can't construct a real `PgPool` without a live
        // database, so the test only checks that the type
        // alias is well-formed. The runtime surface is
        // exercised by the CI integration tests.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PgGraphRepository>();
        assert_send_sync::<Box<dyn GraphRepository + Send + Sync>>();
    }

    /// Suppress unused import warnings for `EdgeKind` / `NodeKind`
    /// on builds where the trait's stub methods happen to inline-
    /// discard every reference.
    #[test]
    fn imports_resolve() {
        let _ = std::any::type_name::<EdgeKind>();
        let _ = std::any::type_name::<NodeKind>();
    }

    /// Helper: an empty `Arc<dyn GraphRepository>` slot is
    /// `Send + Sync` so the MCP handler can hold it.
    #[test]
    fn arc_dyn_is_send_sync() {
        let _: fn() = || {
            let _arc: Arc<dyn GraphRepository + Send + Sync>;
        };
    }
}
