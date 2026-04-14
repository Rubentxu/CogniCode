//! DependencyType - Value object representing the type of dependency between symbols

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the type of dependency relationship between symbols in the code graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DependencyType {
    /// Direct function call
    Calls,
    /// Direct import or use statement
    Imports,
    /// Type inheritance or implementation
    Inherits,
    /// Generic type parameter dependency
    UsesGeneric,
    /// Variable reference dependency
    References,
    /// Module-level dependency
    Defines,
    /// Annotation or attribute dependency
    AnnotatedBy,
    /// Contains or is contained by
    Contains,
}

impl Default for DependencyType {
    fn default() -> Self {
        DependencyType::Calls
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_type_default() {
        assert_eq!(DependencyType::default(), DependencyType::Calls);
    }

    #[test]
    fn test_dependency_type_display() {
        assert_eq!(format!("{}", DependencyType::Calls), "calls");
        assert_eq!(format!("{}", DependencyType::Imports), "imports");
    }
}