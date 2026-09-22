//! Persistence layer implementations for GraphStore
//!
//! This module provides concrete implementations of the GraphStore trait
//! for different storage backends.

pub mod memory_graph_store;

pub mod cached_graph_store;

pub use memory_graph_store::InMemoryGraphStore;

pub use cached_graph_store::CachedGraphStore;

#[cfg(test)]
mod store_contract_tests;
