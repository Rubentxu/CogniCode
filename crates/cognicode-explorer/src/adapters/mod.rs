//! Explorer adapters (hexagonal "driven" implementations).
//!
//! Concrete wiring for [`crate::ports`]: load `CallGraph` from the store,
//! read source files from disk, query the FTS5 symbol index, and surface
//! quality findings from the shared SQLite store.

pub mod call_graph_repository;
pub mod fs_source_reader;
pub mod fts5_search_adapter;
pub mod sqlite_quality_adapter;

pub use call_graph_repository::CallGraphRepository;
pub use fs_source_reader::FsSourceReader;
pub use fts5_search_adapter::Fts5SearchAdapter;
pub use sqlite_quality_adapter::SqliteQualityAdapter;
