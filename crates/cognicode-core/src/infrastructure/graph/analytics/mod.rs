//! Analytics services that run directly on the petgraph-backed
//! [`CallGraphProjection`](crate::infrastructure::graph::CallGraphProjection).
//!
//! These services consume the concrete infra projection (not the domain
//! port) because they need infra-specific graph access (`graph()`,
//! `id_to_index()`) beyond the port surface. They are exempt from the
//! domain/application → infrastructure dependency rule by location.

pub mod community_detector;
pub mod search_ranker;
