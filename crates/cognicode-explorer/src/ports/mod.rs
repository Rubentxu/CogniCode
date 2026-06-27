//! Explorer ports (hexagonal "driven" interfaces).
//!
//! Adapters implement these traits; the domain and service code depend on
//! them, not on concrete adapters.

#[cfg(feature = "multimodal")]
pub mod graph_repository;
pub mod edge_emitter;
pub mod quality_repository;
pub mod search_repository;
pub mod source_reader;
pub mod symbol_repository;

#[cfg(feature = "multimodal")]
pub use graph_repository::{GraphRepository, SearchPage};
pub use edge_emitter::{
    ApiRoute, ApiRouteEdge, BatchStats, EdgeEmitter, EDGE_KIND_GRAPHQL_CALLS,
    EDGE_KIND_GRPC_CALLS, EDGE_KIND_HTTP_CALLS, EDGE_KIND_TRPC_CALLS, PROTOCOL_GRAPHQL,
    PROTOCOL_GRPC, PROTOCOL_HTTP, PROTOCOL_TRPC,
};
pub use quality_repository::{
    QualityGateSummary, QualityIssue, QualityRepository, QualityWritePort, RuleSummary,
};
pub use quality_repository::{NewIssue, UpsertSummary};
pub use search_repository::{SearchHit, SearchRepository};
pub use source_reader::SourceReader;
pub use symbol_repository::{GraphStats, ResolvedSymbol, SymbolRepository};

// Re-export GraphQueryPort types from cognicode-core for consumers that need
// both SymbolRepository (identity) and GraphQueryPort (navigation).
pub use cognicode_core::domain::traits::graph_query_port::{
    CalleeWithMetadata, CallerWithMetadata, EdgeWithMetadata, GraphQueryPort, RelationTarget,
    RelationTargetWithMetadata,
};
