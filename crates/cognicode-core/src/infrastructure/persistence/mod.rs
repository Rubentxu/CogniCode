//! Persistence layer implementations for GraphStore
//!
//! This module provides concrete implementations of the GraphStore trait
//! for different storage backends.

#[cfg(feature = "persistence")]
pub mod memory_graph_store;

#[cfg(feature = "persistence")]
pub use memory_graph_store::InMemoryGraphStore;

// PostgreSQL-backed implementation of the async `Repository` trait.
// Feature-gated so default builds stay sqlx-free. When the `postgres`
// feature is off, this module and the `PostgresRepository` re-export
// are absent from the dep graph entirely.
#[cfg(feature = "postgres")]
pub mod postgres_repository;

#[cfg(feature = "postgres")]
pub use postgres_repository::{NamedViewRow, PostgresRepository};

#[cfg(test)]
mod store_contract_tests;
