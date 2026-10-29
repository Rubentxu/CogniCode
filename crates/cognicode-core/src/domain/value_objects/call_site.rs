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
}
