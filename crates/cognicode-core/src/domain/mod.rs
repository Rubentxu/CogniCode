//! Domain layer for CogniCode
//!
//! This module contains pure Rust domain logic with no external dependencies
//! for parsing or runtime. It defines the core entities, value objects,
//! aggregates, services, and traits that represent the business logic.

pub mod aggregates;
pub mod error;
pub mod events;
// Multimodal (brain-federation) primitives — `FederatedNodeId`,
// `FederatedNode`, `SpaceRegistry`, `MergeDetector`,
// `MergeCandidate`, `FederatedGraphService`. Hidden on default
// builds so the byte-level surface is unchanged.
#[cfg(feature = "multimodal")]
pub mod federation;
// Hexagonal "driven" ports for the Generic Graph Layer (multimodal).
// Hidden on default builds so the byte-level surface is unchanged.
#[cfg(feature = "multimodal")]
pub mod ports;
pub mod services;
pub mod traits;
pub mod value_objects;

pub use aggregates::{CallGraph, Refactor, Symbol};
pub use error::DomainError;
pub use events::{GraphDiffCalculator, GraphEvent};
pub use services::{ComplexityCalculator, CycleDetector, ImpactAnalyzer};
pub use value_objects::{DependencyType, Location, SourceRange, SymbolKind};

// Multimodal port re-exports — `GraphRepository`, `GraphError`,
// `GraphResult`, `SearchPage` live in `ports`. Hidden on default
// builds so the byte-level surface is unchanged.
#[cfg(feature = "multimodal")]
pub use ports::{GraphError, GraphRepository, GraphResult, SearchPage};

// Multimodal federation re-exports — `FederatedNodeId`,
// `FederatedNode`, `SpaceRegistry`, `MergeDetector`,
// `MergeCandidate`, `FederatedGraphService`, `FederatedSearchPage`
// live in `federation`. Hidden on default builds so the byte-level
// surface is unchanged.
#[cfg(feature = "multimodal")]
pub use federation::{
    FederatedGraphService, FederatedNode, FederatedNodeId, FederatedNodeIdError,
    FederatedSearchPage, MergeCandidate, MergeDetector, MergeReason, SpaceRegistry, MERGE_THRESHOLD,
};
