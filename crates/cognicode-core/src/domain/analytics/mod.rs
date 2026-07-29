//! Analytics domain types for the algorithm registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR3 Bounded Paths.

pub mod bounded_shortest_paths_descriptor;
pub mod descriptor;
pub mod lineage;
pub mod pagerank_descriptor;
pub mod scc_descriptor;
pub mod wcc_descriptor;

pub use bounded_shortest_paths_descriptor::*;
pub use descriptor::*;
pub use lineage::*;
