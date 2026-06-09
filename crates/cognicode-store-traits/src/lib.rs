//! # DEPRECATED
//!
//! This crate is **deprecated** as of the `explorer-graph-repository-bridge`
//! slice (Phase 2 of the Explorer Graph roadmap). The canonical home for the
//! graph storage traits and domain types is now the `cognicode_core::domain::traits`
//! module (specifically `graph_store` for the synchronous blob persistence
//! trait and `repository` for the async read-side seam that the PostgreSQL
//! adapter will implement).
//!
//! The crate is **kept in the workspace** for this slice so that
//! `cargo check --workspace` remains green while downstream consumers
//! migrate. Removal is scheduled for a follow-up slice after the
//! PostgreSQL adapter is delivered. Until then, treat every `pub` item
//! below as frozen — do NOT add new dependents in this crate.
//!
//! ## Migration map
//!
//! | Deprecated item                     | Replacement                                              |
//! |-------------------------------------|----------------------------------------------------------|
//! | `cognicode_store_traits::GraphStore`| `cognicode_core::domain::traits::graph_store::GraphStore`|
//! | `cognicode_store_traits::StoreError`| `cognicode_core::domain::traits::graph_store::StoreError`|
//! | `cognicode_store_traits::CallGraph` | `cognicode_core::domain::aggregates::CallGraph` (now carries `Provenance` and `confidence` per edge — the deprecated copy is stale) |
//! | `cognicode_store_traits::Symbol`    | `cognicode_core::domain::aggregates::Symbol`             |
//! | `cognicode_store_traits::Location`  | `cognicode_core::domain::value_objects::Location`        |
//! | `cognicode_store_traits::SymbolKind`| `cognicode_core::domain::value_objects::SymbolKind`      |
//! | `cognicode_store_traits::DependencyType` | `cognicode_core::domain::value_objects::DependencyType` |
//!
//! ## Original crate description (kept for archaeology)
//!
//! Shared traits and types for graph storage.
//! This crate was at the bottom of the dependency graph to break circular
//! dependencies between cognicode-core and cognicode-db.
//!
//! Key types (now superseded — see migration map above):
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
