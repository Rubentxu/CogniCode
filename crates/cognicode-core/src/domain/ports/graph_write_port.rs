//! Domain port for write+read access to the Generic Graph Layer
//! (`graph_nodes` + `graph_edges`).
//!
//! This port exists so the docs extractor (and other writers) can
//! upsert nodes/edges without depending on `PostgresRepository`
//! concrete types. Reads (`find_*`, `get_*`) are also surfaced so
//! consumers don't need direct pool access for graph lookups.
//!
//! **Multimodal feature gating**: the `GraphNode`/`GraphEdge` types
//! are only available under the `multimodal` feature, so this port
//! is too.

#[cfg(feature = "multimodal")]
#[async_trait::async_trait]
pub trait GraphWritePort: Send + Sync {
    /// Upsert a batch of `graph_nodes` rows in a single transaction.
    ///
    /// Conflict policy: PK is `id`; a collision updates the mutable
    /// columns and refreshes `updated_at`. `created_at` is preserved.
    async fn store_nodes(
        &self,
        nodes: Vec<crate::domain::aggregates::generic_graph::GraphNode>,
    ) -> Result<(), crate::domain::traits::repository::CallGraphStoreError>;

    /// Upsert a batch of `graph_edges` rows in a single transaction.
    ///
    /// Conflict policy: natural-key UNIQUE `(source_id, target_id, kind)`
    /// updates the mutable columns. The surrogate `id` is preserved.
    async fn store_edges(
        &self,
        edges: Vec<crate::domain::aggregates::generic_graph::GraphEdge>,
    ) -> Result<(), crate::domain::traits::repository::CallGraphStoreError>;

    /// Find graph nodes, optionally filtered by `kind`. Ordered by
    /// `id ASC` for deterministic pagination. `limit <= 0` means
    /// unbounded.
    async fn find_nodes(
        &self,
        kind: Option<crate::domain::value_objects::node_kind::NodeKind>,
        limit: i64,
    ) -> Result<Vec<crate::domain::aggregates::generic_graph::GraphNode>, crate::domain::traits::repository::CallGraphStoreError>;

    /// Find graph edges. At least one of `source` or `target` MUST be
    /// supplied; passing both is allowed and the predicate is an AND.
    async fn find_edges(
        &self,
        source: Option<crate::domain::aggregates::generic_graph::NodeId>,
        target: Option<crate::domain::aggregates::generic_graph::NodeId>,
    ) -> Result<Vec<crate::domain::aggregates::generic_graph::GraphEdge>, crate::domain::traits::repository::CallGraphStoreError>;

    /// Look up a single graph node by `id`. Returns `Ok(None)` when
    /// the id is missing.
    async fn get_node(
        &self,
        id: crate::domain::aggregates::generic_graph::NodeId,
    ) -> Result<Option<crate::domain::aggregates::generic_graph::GraphNode>, crate::domain::traits::repository::CallGraphStoreError>;

    /// Return the `properties` JSONB map for a node, or `None` if the
    /// node does not exist. Used by the ownership attribution feature
    /// (e12f).
    async fn node_properties(
        &self,
        id: &crate::domain::aggregates::SymbolId,
    ) -> Result<
        Option<std::collections::HashMap<String, String>>,
        crate::domain::traits::repository::CallGraphStoreError,
    >;
}

#[cfg(feature = "postgres")]
mod postgres_adapter {
    use super::GraphWritePort;
    use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
    use crate::domain::aggregates::SymbolId;
    use crate::domain::traits::repository::CallGraphStoreError;
    use crate::domain::value_objects::node_kind::NodeKind;
    use crate::infrastructure::persistence::PostgresRepository;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Adapter that delegates every [`GraphWritePort`] method to a
    /// [`PostgresRepository`].
    #[cfg(feature = "postgres")]
    pub struct PostgresGraphWritePort {
        repo: Arc<PostgresRepository>,
    }

    #[cfg(feature = "postgres")]
    impl PostgresGraphWritePort {
        pub fn new(repo: Arc<PostgresRepository>) -> Self {
            Self { repo }
        }
    }

    #[cfg(feature = "postgres")]
    #[async_trait]
    impl GraphWritePort for PostgresGraphWritePort {
        async fn store_nodes(&self, nodes: Vec<GraphNode>) -> Result<(), CallGraphStoreError> {
            self.repo.store_graph_nodes(nodes).await
        }

        async fn store_edges(&self, edges: Vec<GraphEdge>) -> Result<(), CallGraphStoreError> {
            self.repo.store_graph_edges(edges).await
        }

        async fn find_nodes(
            &self,
            kind: Option<NodeKind>,
            limit: i64,
        ) -> Result<Vec<GraphNode>, CallGraphStoreError> {
            self.repo.find_graph_nodes(kind, limit).await
        }

        async fn find_edges(
            &self,
            source: Option<NodeId>,
            target: Option<NodeId>,
        ) -> Result<Vec<GraphEdge>, CallGraphStoreError> {
            self.repo.find_graph_edges(source, target).await
        }

        async fn get_node(&self, id: NodeId) -> Result<Option<GraphNode>, CallGraphStoreError> {
            self.repo.get_graph_node(id).await
        }

        async fn node_properties(
            &self,
            id: &SymbolId,
        ) -> Result<Option<HashMap<String, String>>, CallGraphStoreError> {
            self.repo.node_properties(id).await
        }
    }
}

#[cfg(feature = "postgres")]
pub use postgres_adapter::PostgresGraphWritePort;