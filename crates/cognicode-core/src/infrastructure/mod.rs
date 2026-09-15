//! Infrastructure Layer - Concrete implementations of domain traits
//!
//! This module implements the domain traits using concrete technologies
//! such as tree-sitter for parsing and petgraph for dependency graphs.

pub mod avc;
// LSI evidence kernel in-memory adapters (E36 M1, design D7). Hidden on
// default builds so the byte-level surface is unchanged (`multimodal`
// precedent). The LadybugDB adapter is deferred (D7).
#[cfg(feature = "evidence-kernel")]
pub mod evidence_kernel;
#[cfg(feature = "multimodal")]
pub mod extraction;
pub mod findings;
pub mod git;
#[cfg(feature = "multimodal")]
pub mod github;
pub mod graph;
// M7 — in-memory append-only Intelligence Event Log (reference oracle).
pub mod intelligence_log;
pub mod lsp;
pub mod mermaid;
pub mod parser;
pub mod persistence;
pub mod refactor;
pub mod safety;
pub mod semantic;
pub mod telemetry;
pub mod testing;
pub mod verification;
pub mod vfs;
