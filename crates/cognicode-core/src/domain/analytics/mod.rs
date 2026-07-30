//! Analytics domain types for the algorithm registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR3 Bounded Paths.
//! Part of E28.5 Structural Analytics Cohort 2 — PR2 Descriptors.

pub mod articulation_descriptor;
pub mod bounded_shortest_paths_descriptor;
pub mod bridges_descriptor;
pub mod descriptor;
pub mod dominators_descriptor;
pub mod kcore_descriptor;
pub mod lineage;
pub mod pagerank_descriptor;
pub mod scc_descriptor;
pub mod wcc_descriptor;

pub use articulation_descriptor::*;
pub use bounded_shortest_paths_descriptor::*;
pub use bridges_descriptor::*;
pub use descriptor::*;
pub use dominators_descriptor::*;
pub use kcore_descriptor::*;
pub use lineage::*;
pub use pagerank_descriptor::*;
pub use scc_descriptor::*;
pub use wcc_descriptor::*;
