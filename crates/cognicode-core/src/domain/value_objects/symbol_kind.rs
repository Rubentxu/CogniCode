//! SymbolKind - Value object representing the kind of a code symbol

use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the kind of a code symbol (function, class, variable, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    /// Function definition
    Function,
    /// Class or struct definition
    Class,
    /// Module or namespace
    Module,
    /// Variable or constant
    Variable,
    /// Parameter
    Parameter,
    /// Type alias or interface
    Type,
    /// Method within a class
    Method,
    /// Property or field
    Property,
    /// Field (same as Property)
    Field,
    /// Import statement
    Import,
    /// Enum variant or value
    EnumVariant,
    /// Trait definition
    Trait,
    /// Generic type parameter
    Generic,
    /// Constant definition
    Constant,
    /// Constructor
    Constructor,
    /// Struct (same as Class)
    Struct,
    /// Enum (same as Type)
    Enum,
    /// Interface (same as Type)
    Interface,
    /// File
    File,
    /// Namespace
    Namespace,
    /// Package
    Package,
    /// Unknown or other
    Unknown,
}

impl SymbolKind {
    /// Returns true if this symbol kind represents a callable entity.
    pub fn is_callable(&self) -> bool {
        matches!(
            self,
            SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor
        )
    }

    /// Returns true if this symbol kind represents a type definition.
    pub fn is_type_definition(&self) -> bool {
        matches!(
            self,
            SymbolKind::Class
                | SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::Trait
                | SymbolKind::Type
                | SymbolKind::Interface
        )
    }

    /// Returns a human-readable name for this symbol kind.
    pub fn name(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Class => "class",
            SymbolKind::Module => "module",
            SymbolKind::Variable => "variable",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Type => "type",
            SymbolKind::Method => "method",
            SymbolKind::Property => "property",
            SymbolKind::Field => "field",
            SymbolKind::Import => "import",
            SymbolKind::EnumVariant => "enum variant",
            SymbolKind::Trait => "trait",
            SymbolKind::Generic => "generic",
            SymbolKind::Constant => "constant",
            SymbolKind::Constructor => "constructor",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Interface => "interface",
            SymbolKind::File => "file",
            SymbolKind::Namespace => "namespace",
            SymbolKind::Package => "package",
            SymbolKind::Unknown => "unknown",
        }
    }

    /// Converts from an LSP SymbolKind number to our SymbolKind
    pub fn from_lsp_kind(kind: u64) -> Self {
        match kind {
            1 => SymbolKind::File,
            2 => SymbolKind::Module,
            3 => SymbolKind::Namespace,
            4 => SymbolKind::Package,
            5 => SymbolKind::Class,
            6 => SymbolKind::Method,
            7 => SymbolKind::Property,
            8 => SymbolKind::Field,
            9 => SymbolKind::Constructor,
            10 => SymbolKind::Enum,
            11 => SymbolKind::Interface,
            12 => SymbolKind::Function,
            13 => SymbolKind::Variable,
            14 => SymbolKind::Constant,
            15 => SymbolKind::Type,
            16 => SymbolKind::Struct,
            17 => SymbolKind::Enum,
            18 => SymbolKind::Interface,
            19 => SymbolKind::Unknown,
            20 => SymbolKind::Unknown,
            21 => SymbolKind::Unknown,
            22 => SymbolKind::Unknown,
            23 => SymbolKind::Unknown,
            24 => SymbolKind::Unknown,
            25 => SymbolKind::Unknown,
            _ => SymbolKind::Unknown,
        }
    }
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_callable() {
        assert!(SymbolKind::Function.is_callable());
        assert!(SymbolKind::Method.is_callable());
        assert!(!SymbolKind::Class.is_callable());
    }

    #[test]
    fn test_symbol_kind_type_definition() {
        assert!(SymbolKind::Class.is_type_definition());
        assert!(SymbolKind::Trait.is_type_definition());
        assert!(!SymbolKind::Function.is_type_definition());
    }

    #[test]
    fn test_symbol_kind_display() {
        assert_eq!(format!("{}", SymbolKind::Function), "function");
        assert_eq!(format!("{}", SymbolKind::Class), "class");
    }

    // Task 4.5: SymbolKind from_lsp_kind tests

    #[test]
    fn test_from_lsp_kind() {
        assert_eq!(SymbolKind::from_lsp_kind(5), SymbolKind::Class);
        assert_eq!(SymbolKind::from_lsp_kind(12), SymbolKind::Function);
        assert_eq!(SymbolKind::from_lsp_kind(13), SymbolKind::Variable);
        assert_eq!(SymbolKind::from_lsp_kind(1), SymbolKind::File);
        assert_eq!(SymbolKind::from_lsp_kind(999), SymbolKind::Unknown);
    }

    #[test]
    fn test_from_lsp_kind_exhaustive() {
        // LSP spec: Method=6, Property=7, Field=8, Constructor=9
        assert_eq!(SymbolKind::from_lsp_kind(6), SymbolKind::Method);
        assert_eq!(SymbolKind::from_lsp_kind(7), SymbolKind::Property);
        assert_eq!(SymbolKind::from_lsp_kind(8), SymbolKind::Field);
        assert_eq!(SymbolKind::from_lsp_kind(9), SymbolKind::Constructor);
        // Module=2, Namespace=3, Package=4
        assert_eq!(SymbolKind::from_lsp_kind(2), SymbolKind::Module);
        assert_eq!(SymbolKind::from_lsp_kind(3), SymbolKind::Namespace);
        assert_eq!(SymbolKind::from_lsp_kind(4), SymbolKind::Package);
        // Constant=14, Type=15, Struct=16
        assert_eq!(SymbolKind::from_lsp_kind(14), SymbolKind::Constant);
        assert_eq!(SymbolKind::from_lsp_kind(15), SymbolKind::Type);
        assert_eq!(SymbolKind::from_lsp_kind(16), SymbolKind::Struct);
        // 0 and very large values → Unknown
        assert_eq!(SymbolKind::from_lsp_kind(0), SymbolKind::Unknown);
        assert_eq!(SymbolKind::from_lsp_kind(u64::MAX), SymbolKind::Unknown);
    }
}
