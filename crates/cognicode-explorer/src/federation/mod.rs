//! Federation primitives — `FederatedNodeId`, `FederatedNode`, the
//! `FederatedGraphService`, the `SpaceRegistry`, the
//! `MergeDetector`, and the `MergeCandidate` type.
//!
//! Every type in this module is feature-gated behind the
//! `multimodal` Cargo feature. On a default build the entire
//! `federation` module is absent from the crate, so the byte-level
//! shape of the public surface is unchanged.

#[cfg(feature = "multimodal")]
pub mod federated_node_id;
#[cfg(feature = "multimodal")]
pub mod space_registry;
#[cfg(feature = "multimodal")]
pub mod federated_node;
#[cfg(feature = "multimodal")]
pub mod federated_graph_service;
#[cfg(feature = "multimodal")]
pub mod merge_candidate;
#[cfg(feature = "multimodal")]
pub mod merge_detector;
