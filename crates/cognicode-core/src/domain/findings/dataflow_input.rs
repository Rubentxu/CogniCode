//! Dataflow view for detector backends (M6, cycle e60).
//!
//! A **pure M6 DTO**: it knows nothing about `TaintPath`, `Statement`,
//! `RunOutput`, `AlgorithmId` or any M5 JSON. The translation from this view
//! into the M5 engine request lives in the application adapter
//! (`application::findings`), which keeps the domain free of analysis-engine
//! types.
//!
//! Pure domain: no I/O.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::SubjectPattern;
use super::grounding::GroundingRef;

/// Where a statement sits in the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataflowLocation {
    /// File path.
    pub path: String,
    /// 1-based line.
    pub line: u32,
}

/// One statement in the dataflow view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataflowStatement {
    /// Stable statement id within the function.
    pub id: u64,
    /// Variables this statement defines.
    pub defs: Vec<String>,
    /// Variables this statement uses.
    pub uses: Vec<String>,
    /// Namespaced classifications observed at this statement
    /// (`security.user_input`, `security.sql_execution`, `security.sanitizer`, …).
    pub subjects: BTreeSet<SubjectPattern>,
    /// Source location (for messages and the causal chain).
    pub location: DataflowLocation,
    /// The canonical fact this statement was projected from, if the pipeline
    /// recorded one.
    ///
    /// Dataflow statements are synthesised from many observations, so this is
    /// frequently `None`. That is deliberate: a statement that cannot be
    /// grounded may still explain a path, but it cannot open the gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grounding: Option<GroundingRef>,
}

impl DataflowStatement {
    /// Whether this statement carries any of `subjects`.
    pub fn has_any_subject(&self, subjects: &[&SubjectPattern]) -> bool {
        subjects.iter().any(|s| self.subjects.contains(*s))
    }
}

/// One function and its statements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataflowFunction {
    /// Function identity (used as the M5 `function_id`).
    pub id: String,
    /// Statements, in program order.
    pub statements: Vec<DataflowStatement>,
}

/// The dataflow view handed to a dataflow backend.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataflowInput {
    /// Functions to analyse.
    pub functions: Vec<DataflowFunction>,
}
