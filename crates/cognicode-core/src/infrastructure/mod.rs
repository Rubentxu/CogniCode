//! Infrastructure Layer - Concrete implementations of domain traits
//!
//! This module implements the domain traits using concrete technologies
//! such as tree-sitter for parsing and petgraph for dependency graphs.

pub mod avc;
#[cfg(feature = "multimodal")]
pub mod extraction;
pub mod git;
#[cfg(feature = "multimodal")]
pub mod github;
pub mod graph;
pub mod lsp;
pub mod mermaid;
pub mod parser;
pub mod persistence;
pub mod refactor;
pub mod safety;
pub mod semantic;
pub mod telemetry;
pub mod testing;
pub mod vfs;
