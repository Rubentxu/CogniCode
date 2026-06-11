//! Hexagonal "driven" ports for the Generic Graph Layer.
//!
//! Hosts the [`GraphRepository`] trait and the [`GraphError`] type
//! that the trait's methods return. Adapters (the in-memory mock,
//! the PostgreSQL adapter) implement the trait; domain + service
//! code depend on the trait, not on the concrete adapters.
//!
//! Every module in this tree is feature-gated behind the
//! `multimodal` Cargo feature. The default build of `cognicode-core`
//! has no `ports` symbol and no `GraphRepository` / `GraphError`
//! items, so the byte-level shape of the public surface is
//! unchanged for default-feature consumers.

#[cfg(feature = "multimodal")]
pub mod graph_error;
#[cfg(feature = "multimodal")]
pub mod graph_repository;

#[cfg(feature = "multimodal")]
pub use graph_error::{GraphError, GraphResult};
#[cfg(feature = "multimodal")]
pub use graph_repository::{GraphRepository, SearchPage};
