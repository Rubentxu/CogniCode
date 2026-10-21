//! What a consumer reads when the build did not cover everything.
//!
//! A reference that the graph could not tie to a declaration is one fact
//! about the code, and it has one shape for every consumer. The MCP and the
//! CLI publish it in different transports, but they publish the same thing,
//! so the projection lives here — once — instead of twice.
//!
//! The fact itself lives in the domain ([`PendingReference`]): this module is
//! only the way of writing it down.
use crate::domain::value_objects::{PendingClass, PendingReference};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A reference the graph could not tie to a declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct PendingReferenceInfo {
    /// The file that writes the reference.
    pub file: String,
    /// The line the reference is written on, counting from one.
    pub line: Option<u32>,
    /// The name the reference names.
    pub target: String,
    /// What the reference is.
    pub kind: String,
    /// Why the far end could not be identified: `unresolved` when no
    /// declaration carries the name, `ambiguous` when several do.
    pub reason: PendingReason,
    /// How many declarations carry the name, when there are several.
    pub candidates: Option<usize>,
}

/// Why the far end of a reference could not be identified.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingReason {
    /// No declaration carries the name.
    Unresolved,
    /// More than one does, and the graph cannot prove which one.
    Ambiguous,
}

impl PendingReferenceInfo {
    /// The one projection of the fact, for every consumer that publishes it.
    pub fn from_reference(reference: &PendingReference) -> Self {
        let (reason, candidates) = match &reference.class {
            PendingClass::Unresolved => (PendingReason::Unresolved, None),
            PendingClass::Ambiguous { candidates } => (PendingReason::Ambiguous, Some(*candidates)),
        };
        Self {
            file: reference.origin_file.clone(),
            line: reference.origin_line,
            target: reference.target.clone(),
            kind: reference.kind.to_string(),
            reason,
            candidates,
        }
    }
}
