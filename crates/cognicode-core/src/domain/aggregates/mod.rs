//! Aggregates module
//!
//! Domain aggregates that encapsulate entity clusters and their invariants.

pub mod call_graph;
#[cfg(feature = "multimodal")]
pub mod generic_graph;
pub mod refactor;
pub mod symbol;

// `CallGraphV1` is intentionally deprecated; it is re-exported only so
// downstream migration code (e.g. `cognicode-db`) can import the
// shadow type from the canonical location. The re-export is *not* an
// API for new consumers.
#[allow(deprecated)]
pub use call_graph::{CallEntry, CallGraph, CallGraphError, CallGraphV1, SymbolId};
#[cfg(feature = "multimodal")]
pub use generic_graph::{GraphEdge, GraphEdgeError, GraphNode, GraphNodeBuilder, NodeId};
pub use refactor::{
    BreakingChange, Refactor, RefactorKind, RefactorParameters, TextEdit, ValidationResult,
    ValidationSeverity,
};
pub use symbol::{FunctionSignature, Parameter, Symbol};
