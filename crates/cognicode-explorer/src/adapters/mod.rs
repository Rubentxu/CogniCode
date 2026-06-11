//! Explorer adapters (hexagonal "driven" implementations).
//!
//! Concrete wiring for [`crate::ports`]: load `CallGraph` from the store,
//! read source files from disk, query the FTS5 symbol index, and surface
//! quality findings from the shared SQLite store.
//!
//! The FTS5 search and SQLite quality adapters are gated behind the
//! `sqlite` feature (see `postgres-default-config`): when the feature
//! is off, those adapters compile to stubs that always return empty
//! results, so the service still works in PG-only mode.

pub mod call_graph_repository;
pub mod fs_source_reader;
#[cfg(feature = "sqlite")]
pub mod fts5_search_adapter;
#[cfg(feature = "multimodal")]
pub mod in_memory_graph_repository;
#[cfg(feature = "sqlite")]
pub mod sqlite_quality_adapter;

pub use call_graph_repository::CallGraphRepository;
pub use fs_source_reader::FsSourceReader;
#[cfg(feature = "sqlite")]
pub use fts5_search_adapter::Fts5SearchAdapter;
#[cfg(feature = "multimodal")]
pub use in_memory_graph_repository::InMemoryGraphRepository;
#[cfg(feature = "sqlite")]
pub use sqlite_quality_adapter::SqliteQualityAdapter;
