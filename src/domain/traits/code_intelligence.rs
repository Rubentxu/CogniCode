//! Trait for code intelligence operations
//!
//! Provides methods for extracting and analyzing code symbols and their relationships.

use crate::domain::aggregates::Symbol;
use crate::domain::value_objects::{Location, SymbolKind};
use async_trait::async_trait;

/// Provider for code intelligence operations
#[async_trait]
pub trait CodeIntelligenceProvider: Send + Sync {
    /// Gets all symbols in a file or directory
    async fn get_symbols(&self, path: &std::path::Path) -> Result<Vec<Symbol>, CodeIntelligenceError>;

    /// Finds all references to a symbol at the given location
    async fn find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError>;

    /// Gets the type hierarchy for a symbol
    async fn get_hierarchy(
        &self,
        location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError>;

    /// Gets the definition location for a reference
    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError>;

    /// Gets document symbols for a file
    async fn get_document_symbols(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError>;

    /// Gets hover information (type + docs) for a symbol at the given location
    async fn hover(
        &self,
        location: &Location,
    ) -> Result<Option<HoverInfo>, CodeIntelligenceError>;
}

/// Represents a reference to a symbol
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Reference {
    /// The location of the reference
    pub location: Location,
    /// The kind of reference (read, write, call, etc.)
    pub reference_kind: ReferenceKind,
    /// Optional container context (e.g., enclosing function)
    pub container: Option<String>,
}

/// Kind of reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReferenceKind {
    /// Reading a variable or calling a function
    Read,
    /// Writing to a variable
    Write,
    /// Calling a function or method
    Call,
    /// Type reference (using a class, struct, etc.)
    Type,
    /// Import statement
    Import,
}

impl ReferenceKind {
    /// Returns true if this is a read-like reference
    pub fn is_read(&self) -> bool {
        matches!(self, ReferenceKind::Read | ReferenceKind::Call | ReferenceKind::Type)
    }

    /// Returns true if this is a write-like reference
    pub fn is_write(&self) -> bool {
        matches!(self, ReferenceKind::Write)
    }
}

/// Type hierarchy information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchy {
    /// The symbol this hierarchy is for
    pub symbol: Symbol,
    /// Parents/super types (for inheritance)
    pub parents: Vec<TypeHierarchyNode>,
    /// Children/sub types
    pub children: Vec<TypeHierarchyNode>,
}

/// A node in the type hierarchy
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchyNode {
    /// The symbol at this node
    pub symbol: Symbol,
    /// Distance from the root (0 = immediate parent/child)
    pub distance: u32,
}

/// A document symbol extracted from source
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    /// The symbol
    pub symbol: Symbol,
    /// The kind of document symbol
    pub document_kind: DocumentSymbolKind,
    /// Range in the source
    pub range: crate::domain::value_objects::SourceRange,
    /// Children (for nested symbols)
    pub children: Vec<DocumentSymbol>,
}

/// Kind of document symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Event,
    Operator,
    TypeParameter,
}

impl DocumentSymbolKind {
    /// Converts to a SymbolKind
    pub fn to_symbol_kind(&self) -> SymbolKind {
        match self {
            DocumentSymbolKind::Class => SymbolKind::Class,
            DocumentSymbolKind::Method => SymbolKind::Method,
            DocumentSymbolKind::Function => SymbolKind::Function,
            DocumentSymbolKind::Variable => SymbolKind::Variable,
            DocumentSymbolKind::Constant => SymbolKind::Constant,
            DocumentSymbolKind::Field => SymbolKind::Property,
            DocumentSymbolKind::Enum => SymbolKind::Enum,
            DocumentSymbolKind::Interface => SymbolKind::Interface,
            DocumentSymbolKind::Constructor => SymbolKind::Constructor,
            DocumentSymbolKind::Module | DocumentSymbolKind::Namespace => SymbolKind::Module,
            DocumentSymbolKind::TypeParameter => SymbolKind::Type,
            _ => SymbolKind::Variable,
        }
    }
}

/// Error type for code intelligence operations
#[derive(Debug, thiserror::Error)]
pub enum CodeIntelligenceError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid location: {0}")]
    InvalidLocation(String),

    #[error("Language not supported: {0}")]
    LanguageNotSupported(String),

    #[error("LSP server unavailable for {language}: {message}")]
    LspUnavailable {
        language: String,
        message: String,
        install_command: String,
    },

    #[error("LSP server error: {0}")]
    LspError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Hover information for a symbol
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HoverInfo {
    /// The content (type signature, documentation, etc.)
    pub content: String,
    /// Optional documentation string
    pub documentation: Option<String>,
    /// The kind of hover result
    pub kind: HoverKind,
}

/// Kind of hover information
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HoverKind {
    /// Type information
    Type,
    /// Documentation only
    Documentation,
    /// Mixed type and documentation
    Mixed,
    /// Source code snippet (tree-sitter fallback)
    Snippet,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::SourceRange;

    struct MockCodeIntelligence;

    impl MockCodeIntelligence {
        fn new() -> Self {
            MockCodeIntelligence
        }

        fn create_test_location(file: &str, line: u32, col: u32) -> Location {
            Location::new(file, line, col)
        }

        fn create_test_symbol(name: &str, kind: SymbolKind, loc: Location) -> Symbol {
            Symbol::new(name, kind, loc)
        }

        fn create_test_source_range(file: &str, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> SourceRange {
            SourceRange::new(
                Location::new(file, start_line, start_col),
                Location::new(file, end_line, end_col),
            )
        }
    }

    #[async_trait::async_trait]
    impl CodeIntelligenceProvider for MockCodeIntelligence {
        async fn get_symbols(&self, path: &std::path::Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            let loc = Location::new(path.to_str().unwrap_or("test.rs"), 0, 0);
            Ok(vec![
                Symbol::new("main", SymbolKind::Function, loc.clone()),
                Symbol::new("MyStruct", SymbolKind::Class, loc),
            ])
        }

        async fn find_references(
            &self,
            location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            Ok(vec![
                Reference {
                    location: location.clone(),
                    reference_kind: ReferenceKind::Read,
                    container: Some("main".to_string()),
                },
                Reference {
                    location: Location::new("other.rs", location.line(), location.column()),
                    reference_kind: ReferenceKind::Write,
                    container: None,
                },
            ])
        }

        async fn get_hierarchy(
            &self,
            location: &Location,
        ) -> Result<TypeHierarchy, CodeIntelligenceError> {
            let symbol = Symbol::new("TestClass", SymbolKind::Class, location.clone());
            Ok(TypeHierarchy {
                symbol: symbol.clone(),
                parents: vec![TypeHierarchyNode {
                    symbol: Symbol::new("ParentClass", SymbolKind::Class, Location::new("parent.rs", 0, 0)),
                    distance: 1,
                }],
                children: vec![TypeHierarchyNode {
                    symbol: Symbol::new("ChildClass", SymbolKind::Class, Location::new("child.rs", 0, 0)),
                    distance: 1,
                }],
            })
        }

        async fn get_definition(
            &self,
            _location: &Location,
        ) -> Result<Option<Location>, CodeIntelligenceError> {
            Ok(Some(Location::new("definition.rs", 10, 5)))
        }

        async fn get_document_symbols(
            &self,
            path: &std::path::Path,
        ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            let loc = Location::new(path.to_str().unwrap_or("test.rs"), 5, 0);
            Ok(vec![
                DocumentSymbol {
                    symbol: Symbol::new("MyFunction", SymbolKind::Function, loc.clone()),
                    document_kind: DocumentSymbolKind::Function,
                    range: SourceRange::new(loc.clone(), Location::new(path.to_str().unwrap_or("test.rs"), 10, 0)),
                    children: vec![],
                },
                DocumentSymbol {
                    symbol: Symbol::new("MyClass", SymbolKind::Class, loc),
                    document_kind: DocumentSymbolKind::Class,
                    range: SourceRange::new(Location::new(path.to_str().unwrap_or("test.rs"), 15, 0), Location::new(path.to_str().unwrap_or("test.rs"), 25, 0)),
                    children: vec![],
                },
            ])
        }

        async fn hover(
            &self,
            _location: &Location,
        ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
            Ok(Some(HoverInfo {
                content: "fn main() -> ()".to_string(),
                documentation: Some("The entry point".to_string()),
                kind: HoverKind::Mixed,
            }))
        }
    }

    #[tokio::test]
    async fn test_mock_get_definition() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.get_definition(&loc).await.unwrap();
        assert!(result.is_some());
        let def = result.unwrap();
        assert_eq!(def.file(), "definition.rs");
        assert_eq!(def.line(), 10);
        assert_eq!(def.column(), 5);
    }

    #[tokio::test]
    async fn test_mock_hover() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.hover(&loc).await.unwrap();
        assert!(result.is_some());
        let hover = result.unwrap();
        assert!(hover.content.contains("main"));
        assert!(hover.documentation.is_some());
        assert_eq!(hover.kind, HoverKind::Mixed);
    }

    #[tokio::test]
    async fn test_mock_find_references() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.find_references(&loc, true).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].reference_kind, ReferenceKind::Read);
        assert_eq!(result[1].reference_kind, ReferenceKind::Write);
    }

    #[tokio::test]
    async fn test_mock_get_symbols() {
        let mock = MockCodeIntelligence::new();
        let path = std::path::Path::new("test.rs");
        let result = mock.get_symbols(path).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name(), "main");
        assert_eq!(result[1].name(), "MyStruct");
    }

    #[tokio::test]
    async fn test_mock_get_document_symbols() {
        let mock = MockCodeIntelligence::new();
        let path = std::path::Path::new("test.rs");
        let result = mock.get_document_symbols(path).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].symbol.name(), "MyFunction");
        assert_eq!(result[0].document_kind, DocumentSymbolKind::Function);
        assert_eq!(result[1].symbol.name(), "MyClass");
        assert_eq!(result[1].document_kind, DocumentSymbolKind::Class);
    }

    #[tokio::test]
    async fn test_mock_get_hierarchy() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.get_hierarchy(&loc).await.unwrap();
        assert_eq!(result.symbol.name(), "TestClass");
        assert_eq!(result.parents.len(), 1);
        assert_eq!(result.parents[0].symbol.name(), "ParentClass");
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].symbol.name(), "ChildClass");
    }

    #[tokio::test]
    async fn test_mock_none_responses() {
        struct MockNoneResponses;
        #[async_trait::async_trait]
        impl CodeIntelligenceProvider for MockNoneResponses {
            async fn get_symbols(&self, _path: &std::path::Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn find_references(&self, _location: &Location, _include_declaration: bool) -> Result<Vec<Reference>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn get_hierarchy(&self, _location: &Location) -> Result<TypeHierarchy, CodeIntelligenceError> {
                let loc = Location::new("empty.rs", 0, 0);
                Ok(TypeHierarchy { symbol: Symbol::new("Empty", SymbolKind::Class, loc.clone()), parents: vec![], children: vec![] })
            }
            async fn get_definition(&self, _location: &Location) -> Result<Option<Location>, CodeIntelligenceError> {
                Ok(None)
            }
            async fn get_document_symbols(&self, _path: &std::path::Path) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn hover(&self, _location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
                Ok(None)
            }
        }

        let mock = MockNoneResponses;
        let loc = Location::new("test.rs", 5, 10);
        let path = std::path::Path::new("test.rs");

        assert!(mock.get_definition(&loc).await.unwrap().is_none());
        assert!(mock.hover(&loc).await.unwrap().is_none());
        assert!(mock.get_symbols(path).await.unwrap().is_empty());
        assert!(mock.find_references(&loc, true).await.unwrap().is_empty());
        assert!(mock.get_document_symbols(path).await.unwrap().is_empty());
        let hierarchy = mock.get_hierarchy(&loc).await.unwrap();
        assert!(hierarchy.parents.is_empty());
        assert!(hierarchy.children.is_empty());
    }
}
