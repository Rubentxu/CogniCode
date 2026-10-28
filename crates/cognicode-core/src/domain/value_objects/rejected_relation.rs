//! A relation the graph could not attach.
//!
//! The code states a relation and the repository refuses it: the two
//! ends are known, the kind is known, and the graph still does not
//! carry the relation. That is a hole in what the graph knows, and a
//! hole nobody names is indistinguishable from a relation the code
//! never stated.
//!
//! This value object is that fact: which two symbols the relation
//! joins, what kind of relation it is, and what the repository said
//! when it refused.
//!
//! It is not a [`PendingReference`](crate::domain::value_objects::PendingReference):
//! that one is a reference whose far end could not be *identified*,
//! and this one is a relation whose ends are known and that the
//! repository still did not take. Two different holes, two different
//! names.

use crate::domain::aggregates::call_graph::SymbolId;
use crate::domain::value_objects::DependencyType;
use serde::{Deserialize, Serialize};

/// A relation the repository refused to attach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedRelation {
    /// The symbol that holds the relation.
    pub source: SymbolId,
    /// The symbol the relation points at.
    pub target: SymbolId,
    /// What the relation is.
    pub kind: DependencyType,
    /// What the repository said when it refused the relation.
    pub reason: String,
}
