//! cognicode-graph-algos — pure graph algorithms compiled to native + wasm32.
//!
//! Algorithms live here as **free functions** over flat slices
//! (`&[Vec<usize>]` for in-neighbors, `&[usize]` for out-degree). The hot
//! loops have zero trait-method dispatch and zero domain-type coupling.
//! Per ADR-048, this is the single source of truth for graph analytics;
//! both `cognicode-core` (native) and `cognicode-graph-wasm` (browser)
//! consume the same compiled artifact.
//!
//! ## Trait surface
//!
//! [`GraphBuilder`] is a one-method trait that extracts adjacency structure
//! from whatever input type the caller has (petgraph-backed projection in
//! native, JSON DTOs in WASM). Algorithms call `build_adjacency()` once at
//! the start, then operate on plain slices.
//!
//! ## WASM compatibility
//!
//! `petgraph` is gated behind the `petgraph-adapter` feature (default off).
//! The default build has no platform deps and compiles to `wasm32-unknown-unknown`.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod algorithms;
pub mod adapters;
pub mod error;
pub mod graph_builder;

pub use algorithms::{god_nodes, page_rank};
pub use error::AnalyticsError;
pub use graph_builder::GraphBuilder;