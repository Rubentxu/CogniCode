//! cognicode-store-traits
//!
//! Shared traits and types for graph storage.
//! This crate is at the bottom of the dependency graph to break circular dependencies
//! between cognicode-core and cognicode-db.
//!
//! Key types:
//! - [`GraphStore`] trait for persisting call graphs
//! - [`CallGraph`] aggregate root
//! - [`FileManifest`] for tracking indexed files
//! - Domain types: [`Symbol`], [`Location`], [`SymbolKind`], [`DependencyType`]

pub mod call_graph;
pub mod dependency_type;
pub mod file_manifest;
pub mod graph_store;
pub mod location;
pub mod symbol;
pub mod symbol_kind;
pub mod value_objects;

// Re-export commonly used types
pub use call_graph::{CallGraph, CallEntry, CallGraphError, MermaidOptions, SymbolId};
pub use dependency_type::DependencyType;
pub use file_manifest::{FileEntry, FileManifest};
pub use graph_store::{GraphStore, StoreError};
pub use location::Location;
pub use symbol::{FunctionSignature, Parameter, Symbol};
pub use symbol_kind::SymbolKind;
pub use value_objects::{Location as LocationVo, SymbolKind as SymbolKindVo, DependencyType as DependencyTypeVo};
