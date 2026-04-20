//! Persistence layer implementations for GraphStore
//!
//! This module provides concrete implementations of the GraphStore trait
//! for different storage backends.

#[cfg(feature = "persistence")]
pub mod memory_graph_store;
#[cfg(feature = "persistence")]
pub mod redb_graph_store;

#[cfg(feature = "persistence")]
pub use memory_graph_store::InMemoryGraphStore;
#[cfg(feature = "persistence")]
pub use redb_graph_store::RedbGraphStore;

#[cfg(test)]
mod store_contract_tests;