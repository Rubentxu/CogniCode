//! `FederatedNode` — a `GraphNode` tagged with the `SpaceId` it
//! belongs to.
//!
//! Constructed by the per-space `GraphRepository` adapters and by
//! the `FederatedGraphService` when fanning out queries. Provides
//! the `federated_id()` method that joins `space_id` + `local_id`
//! with the `::` separator (the canonical wire form).
//!
//! Gated behind the `multimodal` Cargo feature. Default builds
//! do not include this module.

use std::fmt;

use cognicode_core::domain::aggregates::generic_graph::GraphNode;
use cognicode_core::domain::value_objects::SpaceId;
use serde::{Deserialize, Serialize};

use crate::federation::federated_node_id::FederatedNodeId;

/// A `GraphNode` paired with the space it belongs to.
///
/// The inner `node.id` is the LOCAL id (no space prefix). The
/// `federated_id()` method joins the space and local ids with
/// the `::` separator to produce the canonical federated id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederatedNode {
    /// The local `GraphNode` (id, label, kind, source_path,
    /// properties, timestamps). Its `id` MUST NOT contain `::`.
    pub node: GraphNode,
    /// The space this node was loaded from.
    pub space_id: SpaceId,
}

impl FederatedNode {
    /// Build a `FederatedNode` from a local node and its space.
    /// The local node's `id` is stored verbatim; the `space_id`
    /// lives in the wrapper. No prefix is applied to the local id
    /// — the prefix is a property of `federated_id()`.
    pub fn new(node: GraphNode, space_id: SpaceId) -> Self {
        Self { node, space_id }
    }

    /// The canonical federated id: `"{space_id}::{node.id}"`.
    ///
    /// Returns an error (via the `FederatedNodeId::try_new` path)
    /// if the local id contains the `::` separator — that is a
    /// contract violation, and the caller MUST NOT construct a
    /// `FederatedNode` whose local id contains the separator.
    pub fn federated_id(&self) -> Result<FederatedNodeId, String> {
        FederatedNodeId::from_parts(&self.space_id, self.node.id.as_str())
            .map_err(|e| format!("{e}"))
    }
}

impl fmt::Display for FederatedNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.federated_id() {
            Ok(id) => f.write_str(&id.to_string()),
            Err(_) => f.write_str(&self.node.id.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::generic_graph::GraphNode;
    use cognicode_core::domain::value_objects::node_kind::NodeKind;

    fn make_node(id: &str) -> GraphNode {
        GraphNode::builder(id, NodeKind::Symbol(cognicode_core::domain::value_objects::SymbolKind::Function))
            .label("foo")
            .build()
    }

    /// `FederatedNode::new` stores the node and the space.
    #[test]
    fn federated_node_constructs_with_node_and_space_id() {
        let node = make_node("file.rs:main:1");
        let space = SpaceId::try_new("auth").unwrap();
        let fnode = FederatedNode::new(node.clone(), space.clone());
        assert_eq!(fnode.node, node);
        assert_eq!(fnode.space_id, space);
    }

    /// `federated_id()` joins with `::` to produce the wire form.
    #[test]
    fn federated_node_federated_id_joins_space_id_and_local_id_with_separator() {
        let node = make_node("file.rs:main:1");
        let space = SpaceId::try_new("auth").unwrap();
        let fnode = FederatedNode::new(node, space);
        let fid = fnode.federated_id().expect("valid federated id");
        assert_eq!(fid.as_str(), "auth::file.rs:main:1");
        assert_eq!(fid.space_id_str(), "auth");
        assert_eq!(fid.local_id_str(), "file.rs:main:1");
    }

    /// `Display` writes the federated id.
    #[test]
    fn federated_node_display_prints_federated_id() {
        let node = make_node("file.rs:main:1");
        let space = SpaceId::try_new("auth").unwrap();
        let fnode = FederatedNode::new(node, space);
        assert_eq!(format!("{fnode}"), "auth::file.rs:main:1");
    }

    /// Regression: the inner `node.id` MUST NOT contain `::`. The
    /// wrapper stores it verbatim — the prefix is applied at
    /// `federated_id()` time, not on construction.
    #[test]
    fn federated_node_local_id_is_unprefixed() {
        let node = make_node("file.rs:main:1");
        let space = SpaceId::try_new("auth").unwrap();
        let fnode = FederatedNode::new(node, space);
        // The local id is the inner NodeId, with no space prefix.
        assert_eq!(fnode.node.id.as_str(), "file.rs:main:1");
        // No `::` in the inner id (the wrapper's contract is
        // that the local id is unprefixed).
        assert!(!fnode.node.id.as_str().contains("::"));
    }
}
