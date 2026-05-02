//! DependencyType - Value object representing the type of dependency between symbols

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the type of dependency relationship between symbols in the code graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DependencyType {
    #[default]
    Calls,
    Imports,
    Inherits,
    UsesGeneric,
    References,
    Defines,
    AnnotatedBy,
    Contains,
}

impl fmt::Display for DependencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DependencyType::Calls => write!(f, "calls"),
            DependencyType::Imports => write!(f, "imports"),
            DependencyType::Inherits => write!(f, "inherits"),
            DependencyType::UsesGeneric => write!(f, "uses_generic"),
            DependencyType::References => write!(f, "references"),
            DependencyType::Defines => write!(f, "defines"),
            DependencyType::AnnotatedBy => write!(f, "annotated_by"),
            DependencyType::Contains => write!(f, "contains"),
        }
    }
}
