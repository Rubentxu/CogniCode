//! Persistence layer implementations for GraphStore
//!
//! This module provides concrete implementations of the GraphStore trait
//! for different storage backends.

#[cfg(feature = "persistence")]
pub mod memory_graph_store;

#[cfg(feature = "persistence")]
pub mod cached_graph_store;

#[cfg(feature = "persistence")]
pub use memory_graph_store::InMemoryGraphStore;

#[cfg(feature = "persistence")]
pub use cached_graph_store::CachedGraphStore;

// PostgreSQL-backed implementation of the async `Repository` trait.
// Feature-gated so default builds stay sqlx-free. When the `postgres`
// feature is off, this module and the `PostgresRepository` re-export
// are absent from the dep graph entirely.
#[cfg(feature = "postgres")]
pub mod postgres_repository;
#[cfg(feature = "postgres")] pub mod postgres_iac_repository;

#[cfg(feature = "postgres")]
pub use postgres_repository::{NamedViewRow, PostgresRepository, ScanManifestRow, ViewSpecRow};

// IaC repository stub — re-export the concrete PG-backed implementation
// alongside the trait so callers can `use ...::persistence::PostgresIacRepository`
// without reaching through the submodule path. Feature-gated for parity
// with `postgres_repository` (the `IacRepository` trait itself is always
// available; only the PG implementation is gated).
#[cfg(feature = "postgres")]
pub use postgres_iac_repository::PostgresIacRepository;

#[cfg(test)]
mod store_contract_tests;
