//! Explorer adapters (hexagonal "driven" implementations).
//!
//! Concrete wiring for [`crate::ports`]: load `CallGraph` from the store
//! and read source files from disk.

pub mod call_graph_repository;
pub mod fs_source_reader;
#[cfg(feature = "multimodal")]
pub mod in_memory_graph_repository;
/// PG-backed adapter for the `QualityRepository` port. Compiled
/// only when the `postgres` feature is enabled; without it, the
/// runtime must wire a different adapter (or leave the port
/// unwired, in which case the MCP tools return `quality_unavailable`).
#[cfg(feature = "postgres")]
pub mod postgres_quality;

/// PG-backed adapter for the `EdgeEmitter` port (cycle e15.5).
/// Compiled only when the `postgres` feature is enabled; backs the
/// `cognicode_ingest_openapi` handler's persistence path and the
/// `cognicode_trace_route` tool's reverse lookup.
#[cfg(feature = "postgres")]
pub mod postgres_edge_emitter;

pub use call_graph_repository::CallGraphRepository;
pub use fs_source_reader::FsSourceReader;
#[cfg(feature = "multimodal")]
pub use in_memory_graph_repository::InMemoryGraphRepository;
#[cfg(feature = "postgres")]
pub use postgres_quality::PostgresQualityRepository;
#[cfg(feature = "postgres")]
pub use postgres_edge_emitter::PostgresEdgeEmitter;
