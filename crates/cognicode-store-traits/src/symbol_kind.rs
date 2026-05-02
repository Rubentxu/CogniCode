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
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
