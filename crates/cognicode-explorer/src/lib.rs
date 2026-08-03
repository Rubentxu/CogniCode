//! CogniCode Explorer.
//!
//! Moldable code exploration layer for Spotter search, inspectable objects,
//! contextual views, evidence blocks, exploration paths, and decision artifacts.

pub mod adapters;
pub mod affordance;
pub mod api;
pub mod ask;
pub mod boundary;
pub mod domain;
pub mod dto;
pub mod error;
pub mod facades;
pub mod mcp;
pub mod moldql;
pub mod ports;
pub mod registry;
pub mod scaffold;
pub mod session;
pub mod view_spec_payload;

// Multimodal (brain-federation) — `FederatedNodeId`,
// `FederatedGraphService`, `SpaceRegistry`, `MergeDetector`. Hidden
// on default builds so the byte-level surface is unchanged.
#[cfg(feature = "multimodal")]
pub mod federation;

#[cfg(test)]
mod api_graph_tests;
#[cfg(feature = "multimodal")]
mod api_rationale_tests;
#[cfg(feature = "multimodal")]
mod api_support_pack_tests;
#[cfg(test)]
mod dto_tests;

pub use domain::ObjectIdentity;
pub use domain::lens::{Lens, LensContext, LensRegistry};
pub use dto::{DesignFinding, FindingSeverity, LensDescriptor, LensResult};
pub use error::{ExplorerError, ExplorerResult};
pub use facades::investigation::Investigation;
pub use facades::{
    GraphService, MoldQLService, PersistenceService, SearchService, ViewService, WorkspaceService,
};
pub use mcp::ExplorerMcpHandler;
pub use moldql::{MoldQLExecutor, MoldQLItem, MoldQLQuery, MoldQLResult, ParseError};
pub use ports::{
    EdgeWithMetadata, FuzzySymbolSearch, QualityGateSummary, QualityIssue, QualityStore,
    RelationTarget, RelationTargetWithMetadata, ResolvedSymbol, RuleSummary, SearchHit,
    SourceReader, SymbolRepository,
};
pub use scaffold::{Scaffold, ScaffoldRegistry, registry};
