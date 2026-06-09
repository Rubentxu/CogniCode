//! DependencyType - Value object representing the type of dependency between symbols

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

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

/// Parse a `DependencyType` from a string. Accepts BOTH the canonical
/// `Display` form (lowercase, e.g. `"calls"`, `"uses_generic"`) AND
/// the `Debug`/PascalCase form (e.g. `"Calls"`, `"UsesGeneric"`).
///
/// Returns `Err(())` on any other input. Callers that want a default
/// fallback compose `.unwrap_or(DependencyType::Calls)`.
impl FromStr for DependencyType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Calls" | "calls" => Ok(DependencyType::Calls),
            "Imports" | "imports" => Ok(DependencyType::Imports),
            "Inherits" | "inherits" => Ok(DependencyType::Inherits),
            "UsesGeneric" | "uses_generic" => Ok(DependencyType::UsesGeneric),
            "References" | "references" => Ok(DependencyType::References),
            "Defines" | "defines" => Ok(DependencyType::Defines),
            "AnnotatedBy" | "annotated_by" => Ok(DependencyType::AnnotatedBy),
            "Contains" | "contains" => Ok(DependencyType::Contains),
            _ => Err(()),
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

    #[test]
    fn from_str_round_trips_through_display() {
        for variant in [
            DependencyType::Calls,
            DependencyType::Imports,
            DependencyType::Inherits,
            DependencyType::UsesGeneric,
            DependencyType::References,
            DependencyType::Defines,
            DependencyType::AnnotatedBy,
            DependencyType::Contains,
        ] {
            let s = variant.to_string();
            let parsed: DependencyType = s.parse().expect("from_str must accept Display form");
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn from_str_accepts_debug_pascal_case_form() {
        // Debug-derived names like "Calls", "UsesGeneric" must also
        // parse, because some persistence paths use Debug strings.
        assert_eq!(
            "Calls".parse::<DependencyType>().unwrap(),
            DependencyType::Calls
        );
        assert_eq!(
            "UsesGeneric".parse::<DependencyType>().unwrap(),
            DependencyType::UsesGeneric
        );
        assert_eq!(
            "AnnotatedBy".parse::<DependencyType>().unwrap(),
            DependencyType::AnnotatedBy
        );
    }

    #[test]
    fn from_str_rejects_unknown_input() {
        let result: Result<DependencyType, ()> = "garbage".parse();
        assert!(result.is_err(), "unknown input must error");

        // Composition pattern: caller picks the fallback.
        let fallback: DependencyType = "garbage".parse().unwrap_or(DependencyType::Calls);
        assert_eq!(fallback, DependencyType::Calls);
    }
}
