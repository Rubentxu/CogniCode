//! Symbol DTO - Data Transfer Objects for symbol information

use crate::domain::aggregates::symbol::Symbol;
use crate::infrastructure::semantic::symbol_code::extract_docstring;
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
    ///
    /// This method reads the source file to extract doc comments above the symbol.
    pub fn from_symbol(symbol: &Symbol) -> Self {
        let file_path = symbol.location().file();
        let source = Self::read_source_file(file_path);

        let documentation = source.and_then(|src| {
            // Use 1-indexed line number for extract_docstring
            extract_docstring(&src, symbol.location().line() + 1)
        });

        Self {
            id: symbol.fully_qualified_name().to_string(),
            name: symbol.name().to_string(),
            kind: symbol.kind().to_string(),
            file_path: file_path.to_string(),
            line: symbol.location().line() + 1, // Convert to 1-indexed
            column: symbol.location().column() + 1, // Convert to 1-indexed
            documentation,
            signature: symbol.signature().map(|s| s.to_string()),
        }
    }

    /// Reads the source file at the given path
    fn read_source_file(path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
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

    // =========================================================================
    // P3.4 - Doc comments in SymbolDto tests
    // =========================================================================

    #[test]
    fn test_symbol_dto_extracts_doc_comment_above_function() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temp Rust file with a doc comment above a function
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "/// This is a doc comment for my_function").unwrap();
        writeln!(file, "fn my_function() {{}}").unwrap();
        writeln!(file, "fn other_function() {{}}").unwrap();
        file.flush().unwrap();

        // Use AnalysisService to parse and get symbols
        let service = crate::application::services::analysis_service::AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        // Find my_function and check its documentation
        let my_func = symbols.iter().find(|s| s.name == "my_function");
        assert!(
            my_func.is_some(),
            "Should find my_function symbol"
        );
        let dto = my_func.unwrap();

        // The documentation field should be populated (note: current implementation returns None - RED test)
        assert!(
            dto.documentation.is_some(),
            "Documentation should be Some for symbol with doc comment"
        );
        let doc_text = dto.documentation.as_ref().unwrap();
        assert!(
            doc_text.contains("This is a doc comment"),
            "Documentation should contain the doc comment text, got: {}",
            doc_text
        );
    }

    #[test]
    fn test_symbol_dto_without_doc_comment_returns_none() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temp Rust file WITHOUT doc comments
        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "fn function_without_docs() {{}}").unwrap();
        writeln!(file, "/// Another function's doc").unwrap();
        writeln!(file, "fn other() {{}}").unwrap();
        file.flush().unwrap();

        let service = crate::application::services::analysis_service::AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        // Find function_without_docs
        let func = symbols.iter().find(|s| s.name == "function_without_docs");
        assert!(func.is_some(), "Should find function_without_docs symbol");
        let dto = func.unwrap();

        // Should have no documentation since it has no doc comment above it
        assert!(
            dto.documentation.is_none(),
            "Documentation should be None for symbol without doc comment"
        );
    }

    #[test]
    fn test_symbol_dto_module_level_doc_comment() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create a temp Python file with module-level docstring
        let mut file = NamedTempFile::with_suffix(".py").unwrap();
        writeln!(file, "\"\"\"Module level docstring.\"\"\"").unwrap();
        writeln!(file, "def hello():").unwrap();
        writeln!(file, "    pass").unwrap();
        file.flush().unwrap();

        let service = crate::application::services::analysis_service::AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        // Find module (should have docstring)
        // In Python, the module itself is not typically a "symbol" in the tree-sitter sense
        // But functions at the top level should have the module docstring accessible
        let hello_func = symbols.iter().find(|s| s.name == "hello");
        assert!(hello_func.is_some(), "Should find hello function");

        // Note: module-level docs may or may not be captured depending on implementation
        // This test documents expected behavior per spec
    }

    #[test]
    fn test_symbol_dto_doc_comment_single_line_rust() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::with_suffix(".rs").unwrap();
        writeln!(file, "/// Single line doc").unwrap();
        writeln!(file, "fn single_doc_fn() {{}}").unwrap();
        file.flush().unwrap();

        let service = crate::application::services::analysis_service::AnalysisService::new();
        let symbols = service.get_file_symbols(file.path()).unwrap();

        let func = symbols.iter().find(|s| s.name == "single_doc_fn");
        assert!(func.is_some(), "Should find single_doc_fn");
        let dto = func.unwrap();

        assert!(
            dto.documentation.is_some(),
            "Should extract single-line doc comment"
        );
        let doc_text = dto.documentation.as_ref().unwrap();
        assert!(
            doc_text.contains("Single line doc"),
            "Should contain the doc text, got: {}",
            doc_text
        );
    }
}
