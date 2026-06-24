//! Federation primitives — `FederatedNodeId`, `FederatedNode`,
//! `MergeCandidate`, `MergeDetector`, `SpaceRegistry`, the
//! `FederatedGraphService`, and the `FederatedSearchPage`.
//!
//! These types used to live in `cognicode-explorer`; Phase 1 of the
//! Graph Intelligence v2 roadmap moves them into `cognicode-core`
//! so future graph adapters (in any consumer crate) can compose
//! against a canonical federation layer.
//!
//! Every type in this module is feature-gated behind the
//! `multimodal` Cargo feature. On a default build the entire
//! `federation` module is absent from the crate, so the byte-level
//! shape of the public surface is unchanged.
//!
//! ## Module map
//!
//! - [`federated_node_id`] — wire-level federated id (`space::local`).
//! - [`federated_node`] — `GraphNode` tagged with its space.
//! - [`merge_candidate`] — heuristic merge pair + `MergeReason`.
//! - [`merge_detector`] — the scoring service.
//! - [`space_registry`] — in-memory registry of federation spaces.
//! - [`federated_graph_service`] — fan-out over N `GraphRepository`s.

#[cfg(feature = "multimodal")]
pub mod federated_graph_service;
#[cfg(feature = "multimodal")]
pub mod federated_node;
#[cfg(feature = "multimodal")]
pub mod federated_node_id;
#[cfg(feature = "multimodal")]
pub mod merge_candidate;
#[cfg(feature = "multimodal")]
pub mod merge_detector;
#[cfg(feature = "multimodal")]
pub mod space_registry;

#[cfg(feature = "multimodal")]
pub use federated_graph_service::{FederatedGraphService, FederatedSearchPage};
#[cfg(feature = "multimodal")]
pub use federated_node::FederatedNode;
#[cfg(feature = "multimodal")]
pub use federated_node_id::{FederatedNodeId, FederatedNodeIdError};
#[cfg(feature = "multimodal")]
pub use merge_candidate::{MergeCandidate, MergeReason};
#[cfg(feature = "multimodal")]
pub use merge_detector::{MERGE_THRESHOLD, MergeDetector};
#[cfg(feature = "multimodal")]
pub use space_registry::SpaceRegistry;
