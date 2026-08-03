//! Explorer adapters (hexagonal "driven" implementations).
//!
//! Concrete wiring for [`crate::ports`]: load `CallGraph` from the store
//! and read source files from disk.

pub mod call_graph_repository;
pub mod fs_source_reader;
pub mod in_memory_graph_repository;

/// Graph-shaped quality hotspot adapter combining fan-in with weighted issue counts.
/// Used by the `RiskMap` view.
pub mod quality_graph_repository;

pub use call_graph_repository::CallGraphRepository;
pub use fs_source_reader::FsSourceReader;
pub use in_memory_graph_repository::InMemoryGraphRepository;
pub use quality_graph_repository::{HotspotNode, QualityGraphRepository, RelEdge, TraversalFilter};
