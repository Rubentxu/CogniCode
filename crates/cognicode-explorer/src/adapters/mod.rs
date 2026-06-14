//! Explorer adapters (hexagonal "driven" implementations).
//!
//! Concrete wiring for [`crate::ports`]: load `CallGraph` from the store
//! and read source files from disk.

pub mod call_graph_repository;
pub mod fs_source_reader;
#[cfg(feature = "multimodal")]
pub mod in_memory_graph_repository;

pub use call_graph_repository::CallGraphRepository;
pub use fs_source_reader::FsSourceReader;
#[cfg(feature = "multimodal")]
pub use in_memory_graph_repository::InMemoryGraphRepository;
