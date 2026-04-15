//! Symbol DTO - Data Transfer Objects for symbol information

use crate::domain::aggregates::symbol::Symbol;
use serde::{Deserialize, Serialize};

/// DTO for symbol information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDto {
    /// Unique identifier
    pub id: String,
    /// Name of the symbol
    pub name: String,
    /// Kind of symbol
    pub kind: String,
    /// File path
    pub file_path: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
    /// Documentation if available
    pub documentation: Option<String>,
    /// Signature for callable symbols
    pub signature: Option<String>,
}

impl SymbolDto {
    /// Creates a SymbolDto from a Symbol
    pub fn from_symbol(symbol: &Symbol) -> Self {
        Self {
            id: symbol.fully_qualified_name().to_string(),
            name: symbol.name().to_string(),
            kind: symbol.kind().to_string(),
            file_path: symbol.location().file().to_string(),
            line: symbol.location().line() + 1, // Convert to 1-indexed
            column: symbol.location().column() + 1, // Convert to 1-indexed
            documentation: None,                // Symbol doesn't have documentation field
            signature: symbol.signature().map(|s| s.to_string()),
        }
    }
}

/// DTO for symbol location information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolLocationDto {
    /// Symbol identifier
    pub id: String,
    /// Name of the symbol
    pub name: String,
    /// File path
    pub file_path: String,
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed)
    pub column: u32,
}

impl From<&Symbol> for SymbolLocationDto {
    fn from(symbol: &Symbol) -> Self {
        Self {
            id: symbol.fully_qualified_name().to_string(),
            name: symbol.name().to_string(),
            file_path: symbol.location().file().to_string(),
            line: symbol.location().line() + 1, // Convert to 1-indexed
            column: symbol.location().column() + 1, // Convert to 1-indexed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::{FunctionSignature, Parameter, Symbol};
    use crate::domain::value_objects::{Location, SymbolKind};

    #[test]
    fn test_symbol_dto_from_symbol() {
        let location = Location::new("test.rs", 10, 5);
        let symbol = Symbol::new("my_function", SymbolKind::Function, location);
        let dto = SymbolDto::from_symbol(&symbol);
        assert_eq!(dto.name, "my_function");
        assert_eq!(dto.kind, "function");
        assert_eq!(dto.file_path, "test.rs");
        assert_eq!(dto.line, 11);
        assert_eq!(dto.column, 6);
    }

    #[test]
    fn test_symbol_dto_from_symbol_with_signature() {
        let location = Location::new("test.rs", 5, 0);
        let signature = FunctionSignature::new(
            vec![Parameter::new("x", Some("i32".to_string()))],
            Some("i32".to_string()),
            false,
        );
        let symbol = Symbol::new("add", SymbolKind::Function, location).with_signature(signature);
        let dto = SymbolDto::from_symbol(&symbol);
        assert!(dto.signature.is_some());
        assert!(dto.signature.unwrap().contains("x: i32"));
    }

    #[test]
    fn test_symbol_dto_line_column_1_indexed() {
        let location = Location::new("mod.rs", 0, 0);
        let symbol = Symbol::new("start", SymbolKind::Function, location);
        let dto = SymbolDto::from_symbol(&symbol);
        assert_eq!(dto.line, 1);
        assert_eq!(dto.column, 1);
    }

    #[test]
    fn test_symbol_location_dto_from_symbol() {
        let location = Location::new("src/main.rs", 20, 15);
        let symbol = Symbol::new("main", SymbolKind::Function, location);
        let dto = SymbolLocationDto::from(&symbol);
        assert_eq!(dto.name, "main");
        assert_eq!(dto.file_path, "src/main.rs");
        assert_eq!(dto.line, 21);
        assert_eq!(dto.column, 16);
    }

    #[test]
    fn test_symbol_location_dto_id_is_fqn() {
        let location = Location::new("lib.rs", 100, 0);
        let symbol = Symbol::new("MyClass", SymbolKind::Class, location);
        let dto = SymbolLocationDto::from(&symbol);
        assert!(dto.id.contains("MyClass"));
        assert!(dto.id.contains("lib.rs"));
    }

    #[test]
    fn test_symbol_dto_class_symbol() {
        let location = Location::new("model.rs", 5, 10);
        let symbol = Symbol::new("User", SymbolKind::Class, location);
        let dto = SymbolDto::from_symbol(&symbol);
        assert_eq!(dto.kind, "class");
        assert!(dto.signature.is_none());
    }
}
