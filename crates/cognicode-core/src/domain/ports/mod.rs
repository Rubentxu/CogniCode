//! Hexagonal "driven" ports for the Generic Graph Layer.
//!
//! Hosts the [`GraphRepository`] trait and the [`GraphError`] type
//! that the trait's methods return. Adapters (the in-memory mock,
//! the PostgreSQL adapter) implement the trait; domain + service
//! code depend on the trait, not on the concrete adapters.
//!
//! The `ports` module and its contents (`GraphRepository`, `GraphError`,
//! `GraphResult`, `SearchPage`) are available in the default build.
//! The `multimodal` feature gates the WRITE/exTRACTION path only.

pub mod graph_error;
pub mod graph_repository;
pub mod node_property_reader;

pub use graph_error::{GraphError, GraphResult};
pub use graph_repository::{GraphRepository, SearchPage};
pub use node_property_reader::NodePropertyReader;
