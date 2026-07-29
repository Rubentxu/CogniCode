//! Analytics domain types for the algorithm registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR2 Cohort-1 Core.

pub mod descriptor;
pub mod lineage;
pub mod pagerank_descriptor;
pub mod scc_descriptor;
pub mod wcc_descriptor;

pub use descriptor::*;
pub use lineage::*;
