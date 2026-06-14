//! CogniCode Explorer.
//!
//! Moldable code exploration layer for Spotter search, inspectable objects,
//! contextual views, evidence blocks, exploration paths, and decision artifacts.

pub mod adapters;
pub mod api;
pub mod ask;
pub mod cli_dispatch;
pub mod domain;
pub mod dto;
pub mod error;
pub mod facades;
pub mod mcp;
pub mod moldql;
pub mod ports;
pub mod registry;
pub mod session;
#[cfg(feature = "postgres")]
pub mod view_spec_store;

// Multimodal (brain-federation) — `FederatedNodeId`,
// `FederatedGraphService`, `SpaceRegistry`, `MergeDetector`. Hidden
// on default builds so the byte-level surface is unchanged.
#[cfg(feature = "multimodal")]
pub mod federation;

#[cfg(test)]
mod api_graph_tests;
#[cfg(feature = "multimodal")]
mod api_rationale_tests;
#[cfg(test)]
mod dto_tests;

// In-Memory Bridge for loading a `CallGraph` from PostgreSQL into the
// explorer at binary startup. Feature-gated: when the `postgres`
// feature is off, the helper (and its sqlx dependency) is unreachable.
#[cfg(feature = "postgres")]
pub mod postgres_bridge;

pub use domain::ObjectIdentity;
pub use domain::lens::{Lens, LensContext, LensRegistry};
pub use dto::{DesignFinding, FindingSeverity, LensDescriptor, LensResult};
pub use error::{ExplorerError, ExplorerResult};
pub use mcp::ExplorerMcpHandler;
pub use moldql::{MoldQLExecutor, MoldQLItem, MoldQLQuery, MoldQLResult, ParseError};
pub use ports::{
    EdgeWithMetadata, QualityGateSummary, QualityIssue, QualityRepository, RelationTarget,
    RelationTargetWithMetadata, ResolvedSymbol, RuleSummary, SearchHit, SearchRepository,
    SourceReader, SymbolRepository,
};
