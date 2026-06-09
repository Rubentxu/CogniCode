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
pub mod mcp;
pub mod moldql;
pub mod ports;
pub mod service;
pub mod session;

#[cfg(test)]
mod api_graph_tests;
#[cfg(test)]
mod dto_tests;
#[cfg(test)]
mod service_tests;

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
    EdgeWithMetadata, MetadataAwareRepository, QualityGateSummary, QualityIssue, QualityRepository,
    RelationTarget, RelationTargetWithMetadata, ResolvedSymbol, RuleSummary, SearchHit,
    SearchRepository, SourceReader, SymbolRepository,
};
