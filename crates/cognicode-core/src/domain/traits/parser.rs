//! Parser trait - Interface for code parsing implementations

use std::borrow::Cow;

use crate::domain::aggregates::Symbol;
use crate::domain::value_objects::SourceRange;
use thiserror::Error;

/// Errors that can occur during parsing operations.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Failed to parse code: {0}")]
    ParseFailed(String),

    #[error("Invalid source code: {0}")]
    InvalidSource(String),

    #[error("Language not supported: {0}")]
    UnsupportedLanguage(String),

    #[error("Query execution failed: {0}")]
    QueryFailed(String),

    #[error("Tree traversal failed: {0}")]
    TraversalFailed(String),

    #[error("Symbol extraction failed: {0}")]
    SymbolExtractionFailed(String),
}

/// Result type for parser operations.
pub type ParseResult<T> = Result<T, ParseError>;

/// Trait for code parsers that extract symbols and AST information from source code.
pub trait Parser: Send + Sync {
    /// Parses the given source code and returns a parse result containing the AST.
    fn parse(&self, source: &str) -> ParseResult<ParsedTree>;

    /// Finds all function definitions in the source code.
    fn find_function_definitions(&self, source: &str) -> ParseResult<Vec<Symbol>>;

    /// Finds all symbols (functions, classes, variables, etc.) in the source code.
    fn find_all_symbols(&self, source: &str) -> ParseResult<Vec<Symbol>>;

    /// Returns the language handled by this parser.
    fn language(&self) -> &str;
}

/// Trait for AST traversal and navigation.
pub trait AstScanner: Send + Sync {
    /// Traverses the AST starting from a node and applies the visitor pattern.
    fn scan<'a>(
        &self,
        root: &'a tree_sitter::Tree,
        source: &str,
    ) -> ParseResult<Vec<ScannedNode<'a>>>;

    /// Finds all nodes of a specific type in the AST.
    fn find_nodes_by_type<'a>(
        &self,
        root: &'a tree_sitter::Tree,
        node_type: &str,
    ) -> ParseResult<Vec<ScannedNode<'a>>>;

    /// Gets the source text for a node.
    fn get_node_text(&self, node: &tree_sitter::Node, source: &str) -> String;

    /// Converts a tree-sitter node to a SourceRange.
    fn node_to_range(&self, node: &tree_sitter::Node) -> SourceRange;
}

/// Represents a scanned node from the AST.
#[derive(Debug, Clone)]
pub struct ScannedNode<'a> {
    /// The type of the node (e.g., "function_definition", "class")
    pub node_type: Cow<'a, str>,
    /// The source range of this node
    pub range: SourceRange,
    /// Child nodes
    pub children: Vec<ScannedNode<'a>>,
    /// Optional: the symbol if this node represents a symbol definition
    pub symbol: Option<Symbol>,
}

impl<'a> ScannedNode<'a> {
    /// Creates a new scanned node.
    pub fn new(node_type: impl Into<Cow<'a, str>>, range: SourceRange) -> Self {
        Self {
            node_type: node_type.into(),
            range,
            children: Vec::new(),
            symbol: None,
        }
    }

    /// Sets the symbol for this node.
    pub fn with_symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Adds a child node.
    pub fn add_child(&mut self, child: ScannedNode<'a>) {
        self.children.push(child);
    }

    /// Returns true if this node has children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Result of a successful parse operation, containing the syntax tree and source.
#[derive(Debug, Clone)]
pub struct ParsedTree {
    /// The parsed syntax tree
    pub tree: tree_sitter::Tree,
    /// The original source code
    pub source: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};

    struct MockParser {
        dummy_tree: tree_sitter::Tree,
    }

    impl MockParser {
        fn new() -> Self {
            let mut parser = tree_sitter::Parser::new();
            let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
            parser.set_language(&language).unwrap();
            let source = "fn dummy() {}";
            let tree = parser.parse(source, None).unwrap();
            Self { dummy_tree: tree }
        }
    }

    impl Parser for MockParser {
        fn parse(&self, source: &str) -> ParseResult<ParsedTree> {
            Ok(ParsedTree {
                tree: self.dummy_tree.clone(),
                source: source.to_string(),
            })
        }

        fn find_function_definitions(&self, _source: &str) -> ParseResult<Vec<Symbol>> {
            let location = Location::new("test.rs", 0, 0);
            Ok(vec![Symbol::new(
                "mock_func",
                SymbolKind::Function,
                location,
            )])
        }

        fn find_all_symbols(&self, _source: &str) -> ParseResult<Vec<Symbol>> {
            let location1 = Location::new("test.rs", 0, 0);
            let location2 = Location::new("test.rs", 10, 0);
            Ok(vec![
                Symbol::new("mock_func", SymbolKind::Function, location1),
                Symbol::new("MockClass", SymbolKind::Class, location2),
            ])
        }

        fn language(&self) -> &str {
            "Mock"
        }
    }

    #[test]
    fn test_mock_parse_tree() {
        let mock = MockParser::new();
        let source = "fn test() {}";
        let result = mock.parse(source);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.source, source);
    }

    #[test]
    fn test_mock_find_symbols() {
        let mock = MockParser::new();
        let result = mock.find_all_symbols("fn test() {}");
        assert!(result.is_ok());
        let symbols = result.unwrap();
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name(), "mock_func");
        assert_eq!(symbols[1].name(), "MockClass");
    }

    #[test]
    fn test_mock_find_relationships() {
        let mock = MockParser::new();
        let result = mock.find_function_definitions("fn test() {}");
        assert!(result.is_ok());
        let symbols = result.unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name(), "mock_func");
    }
}
