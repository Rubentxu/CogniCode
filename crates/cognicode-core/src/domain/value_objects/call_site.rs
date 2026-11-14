//! A call, as the reader saw it, before anyone tried to resolve it.
//!
//! A call is a fact about what the code says: a declaration holds it, a
//! file writes it, a line carries it, and it names something. What it
//! names may still turn out to be one declaration, several of them, or
//! none — so a call is not a relation yet, and it travels as its own
//! value until the resolution decides what it becomes.
//!
//! It exists because the identity of a call is not recoverable from the
//! name it names. Two calls in the same file can name the same thing, and
//! a consumer that is told "a call could not be tied" without knowing
//! which one cannot do anything with the fact.

use crate::domain::aggregates::call_graph::SymbolId;

/// A call, as the reader saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    /// The declaration that holds the call.
    pub source: SymbolId,
    /// The file that writes the call.
    pub origin_file: String,
    /// The line the call is written on, counting from one.
    pub line: u32,
    /// The name the call names, as it was written.
    pub target: String,
    /// The type the call is qualified by, when the path spells one:
    /// `Duration` in `Duration::from_millis(10)`.
    ///
    /// `target` keeps the last segment of the path, so on its own it cannot
    /// tell the project's `EventTime::from_millis` from the standard
    /// library's `Duration::from_millis`. The qualifier is what can, and a
    /// resolver that finds no declaration of it in the project knows the call
    /// is somebody else's. `None` when the call is written without a path,
    /// through a value, or in a language whose spelling does not say which
    /// segment is a type.
    pub type_qualifier: Option<String>,
    /// The head of the path the call is written through: the crate, module
    /// or package the path starts at, `serde_json` in `serde_json::to_value(1)`.
    ///
    /// The same name can belong to the project and to a crate the project
    /// pulls: `to_value` is the project's own function and `serde_json`'s,
    /// and the last segment of the path cannot tell them apart. The head of
    /// the path can — a package that pulls `serde_json` from a registry does
    /// not own what that crate calls, so a call written through it is not a
    /// call to the project's declaration of the same name.
    ///
    /// `None` when the call is written without a path, through a value, or
    /// through a type: `Duration::from_millis` starts with a type, and the
    /// type qualifier carries that call, not this.
    pub path_head: Option<String>,
}
