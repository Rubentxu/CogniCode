//! Aggregates module
//!
//! Domain aggregates that encapsulate entity clusters and their invariants.

pub mod call_graph;
pub mod refactor;
pub mod symbol;

pub use call_graph::{CallEntry, CallGraph, CallGraphError, SymbolId};
pub use refactor::{
    BreakingChange, Refactor, RefactorKind, RefactorParameters, TextEdit, ValidationResult,
    ValidationSeverity,
};
pub use symbol::{FunctionSignature, Parameter, Symbol};
