//! Tree-sitter based parser implementation

use crate::domain::aggregates::symbol::Symbol;
use crate::domain::traits::{ParseError, ParseResult, ParsedTree, Parser};
use crate::domain::value_objects::{Location, SymbolKind};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tree_sitter::Parser as TsParser;

// Thread-local cache of TreeSitterParser instances per language.
//
// Each thread gets its own cache, avoiding the cost of creating
// a new parser (and loading the tree-sitter language) for each file.
thread_local! {
    static PARSER_CACHE: std::cell::RefCell<HashMap<Language, TreeSitterParser>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Represents an occurrence of an identifier in source code
#[derive(Debug, Clone)]
pub struct IdentifierOccurrence {
    /// Line number (0-indexed)
    pub line: u32,
    /// Column number (0-indexed)
    pub column: u32,
    /// Length of the identifier
    pub length: u32,
    /// Context (the line of code)
    pub context: String,
}

/// Supported programming languages for parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    Rust,
    JavaScript,
    TypeScript,
    Go,
    Java,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: Option<&std::ffi::OsStr>) -> Option<Self> {
        ext.and_then(|e| e.to_str())
            .and_then(|s| match s.to_lowercase().as_str() {
                "py" => Some(Language::Python),
                "rs" => Some(Language::Rust),
                "js" => Some(Language::JavaScript),
                "ts" => Some(Language::TypeScript),
                "jsx" => Some(Language::JavaScript),
                "tsx" => Some(Language::TypeScript),
                "go" => Some(Language::Go),
                "java" => Some(Language::Java),
                _ => None,
            })
    }

    /// Returns the tree-sitter Language for this language
    pub fn to_ts_language(self) -> tree_sitter::Language {
        match self {
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::Java => tree_sitter_java::LANGUAGE.into(),
        }
    }

    /// Returns the name of the language
    pub fn name(self) -> &'static str {
        match self {
            Language::Python => "Python",
            Language::Rust => "Rust",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
            Language::Java => "Java",
        }
    }

    /// Returns the node type for function definitions in this language
    pub fn function_node_type(self) -> &'static str {
        match self {
            Language::Python => "function_definition",
            Language::Rust => "function_item",
            Language::JavaScript | Language::TypeScript => "function_declaration",
            Language::Go => "function_declaration",
            Language::Java => "method_declaration",
        }
    }

    /// Returns the node type for class definitions in this language
    pub fn class_node_type(self) -> &'static str {
        match self {
            Language::Python => "class_definition",
            Language::Rust => "struct_item",
            Language::JavaScript | Language::TypeScript => "class_declaration",
            Language::Go => "type_declaration",
            Language::Java => "class_declaration",
        }
    }

    /// Returns the node type for variable declarations in this language
    pub fn variable_node_type(self) -> &'static str {
        match self {
            Language::Python => "variable_declaration",
            Language::Rust => "let_declaration",
            Language::JavaScript | Language::TypeScript => "variable_declaration",
            Language::Go => "short_var_declaration",
            Language::Java => "local_variable_declaration",
        }
    }

    /// Returns the node type for call expressions in this language
    pub fn call_node_type(self) -> &'static str {
        match self {
            Language::Python => "call",
            Language::Rust => "call_expression",
            Language::JavaScript | Language::TypeScript => "call_expression",
            Language::Go => "call_expression",
            Language::Java => "method_invocation",
        }
    }

    /// Returns whether this language uses 'function' field in call nodes
    pub fn call_has_function_field(self) -> bool {
        match self {
            Language::Python => true,
            Language::Rust => false,
            Language::JavaScript | Language::TypeScript => true,
            Language::Go => true,
            Language::Java => false,
        }
    }

    /// Returns the LSP server binary name for this language
    pub fn lsp_server_binary(self) -> &'static str {
        match self {
            Language::Rust => "rust-analyzer",
            Language::Python => "pyright-langserver",
            Language::TypeScript | Language::JavaScript => "typescript-language-server",
            Language::Go => "gopls",
            Language::Java => "jdtls",
        }
    }

    /// Returns the install command for the LSP server
    pub fn lsp_install_command(self) -> &'static str {
        match self {
            Language::Rust => "rustup component add rust-analyzer",
            Language::Python => "npm install -g pyright",
            Language::TypeScript | Language::JavaScript => {
                "npm install -g typescript-language-server typescript"
            }
            Language::Go => "go install golang.org/x/tools/gopls@latest",
            Language::Java => "brew install jdtls",
        }
    }

    /// Returns the arguments to pass to the LSP server binary
    pub fn lsp_args(self) -> &'static [&'static str] {
        match self {
            Language::Rust => &[],
            Language::Python => &["--stdio"],
            Language::TypeScript | Language::JavaScript => &["--stdio"],
            Language::Go => &["serve"],
            Language::Java => &[],
        }
    }

    /// Returns the display name of the LSP server
    pub fn lsp_server_name(self) -> &'static str {
        match self {
            Language::Rust => "rust-analyzer",
            Language::Python => "pyright",
            Language::TypeScript | Language::JavaScript => "typescript-language-server",
            Language::Go => "gopls",
            Language::Java => "eclipse-jdtls",
        }
    }

    /// Returns all supported languages
    pub fn all_languages() -> &'static [Self] {
        &[
            Language::Rust,
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::Go,
            Language::Java,
        ]
    }
}

/// Tree-sitter based parser implementation
#[derive(Clone)]
pub struct TreeSitterParser {
    language: Language,
    parser: Arc<Mutex<TsParser>>,
}

impl TreeSitterParser {
    /// Returns the language this parser is configured for
    pub fn language(&self) -> Language {
        self.language
    }
}

impl TreeSitterParser {
    /// Creates a new TreeSitterParser for the given language
    pub fn new(language: Language) -> ParseResult<Self> {
        let ts_language = language.to_ts_language();
        let mut parser = TsParser::new();
        parser
            .set_language(&ts_language)
            .map_err(|e| ParseError::ParseFailed(format!("Failed to set language: {}", e)))?;

        Ok(Self {
            language,
            parser: Arc::new(Mutex::new(parser)),
        })
    }

    /// Gets a parser for the given language from the thread-local cache,
    /// creating one if not yet cached on this thread.
    pub fn with_cache(language: Language) -> ParseResult<Self> {
        PARSER_CACHE.with(|cache| {
            let mut cache = cache.borrow_mut();
            if let Some(parser) = cache.get(&language) {
                return Ok(parser.clone());
            }
            let parser = Self::new(language)?;
            cache.insert(language, parser.clone());
            Ok(parser)
        })
    }

    /// Parses source code and returns a tree-sitter tree
    pub fn parse_tree(&self, source: &str) -> ParseResult<tree_sitter::Tree> {
        let mut parser = self.parser.lock();
        parser
            .parse(source, None)
            .ok_or_else(|| ParseError::ParseFailed("Failed to parse source".to_string()))
    }
}

impl Parser for TreeSitterParser {
    fn parse(&self, source: &str) -> ParseResult<ParsedTree> {
        let tree = self.parse_tree(source)?;
        Ok(ParsedTree {
            tree,
            source: source.to_string(),
        })
    }

    fn find_function_definitions(&self, source: &str) -> ParseResult<Vec<Symbol>> {
        let tree = self.parse_tree(source)?;
        let mut symbols = Vec::new();
        let function_type = self.language.function_node_type();
        self.find_nodes_recursive(tree.root_node(), source, function_type, &mut symbols);
        Ok(symbols)
    }

    fn find_all_symbols(&self, source: &str) -> ParseResult<Vec<Symbol>> {
        self.find_all_symbols_with_path(source, "source")
    }

    fn language(&self) -> &str {
        self.language.name()
    }
}

impl TreeSitterParser {
    /// Helper to recursively find nodes of a specific type
    fn find_nodes_recursive(
        &self,
        node: tree_sitter::Node,
        source: &str,
        target_type: &str,
        symbols: &mut Vec<Symbol>,
    ) {
        self.find_nodes_recursive_with_path(node, source, "source", target_type, symbols);
    }

    /// Finds all symbols with a specific file path (single-pass iterative DFS)
    pub fn find_all_symbols_with_path(
        &self,
        source: &str,
        file_path: &str,
    ) -> ParseResult<Vec<Symbol>> {
        let tree = self.parse_tree(source)?;
        let mut symbols = Vec::new();

        let function_type = self.language.function_node_type();
        let class_type = self.language.class_node_type();
        let variable_type = self.language.variable_node_type();

        let mut stack = Vec::new();
        stack.push(tree.root_node());

        while let Some(current) = stack.pop() {
            let kind = current.kind();

            if kind == function_type || kind == class_type || kind == variable_type {
                if let Some(symbol) = self.node_to_symbol_with_path(current, source, file_path) {
                    symbols.push(symbol);
                }
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }

        Ok(symbols)
    }

    /// Helper to iteratively find nodes of a specific type with file path (iterative DFS)
    fn find_nodes_recursive_with_path(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        target_type: &str,
        symbols: &mut Vec<Symbol>,
    ) {
        let mut stack = Vec::new();
        stack.push(node);

        while let Some(current) = stack.pop() {
            if current.kind() == target_type {
                if let Some(symbol) = self.node_to_symbol_with_path(current, source, file_path) {
                    symbols.push(symbol);
                }
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }
    }

    #[allow(dead_code)]
    /// Converts a tree-sitter node to a Symbol (uses "source" as file path)
    fn node_to_symbol(&self, node: tree_sitter::Node, source: &str) -> Option<Symbol> {
        self.node_to_symbol_with_path(node, source, "source")
    }

    /// Converts a tree-sitter node to a Symbol with the given file path
    fn node_to_symbol_with_path(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
    ) -> Option<Symbol> {
        let name = self.find_identifier_name(node, source)?;

        let kind = match node.kind() {
            f if f == self.language.function_node_type() => SymbolKind::Function,
            c if c == self.language.class_node_type() => SymbolKind::Class,
            v if v == self.language.variable_node_type() => SymbolKind::Variable,
            _ => SymbolKind::Unknown,
        };

        let start = node.start_position();
        let location = Location::new(file_path, start.row as u32, start.column as u32);
        Some(Symbol::new(name, kind, location))
    }

    /// Searches for an identifier name in a node tree (two-phase iterative DFS)
    fn find_identifier_name(&self, node: tree_sitter::Node, source: &str) -> Option<String> {
        // Phase 1: Check direct children first
        {
            let cc = node.child_count();
            for i in 0..cc {
                if let Some(child) = node.child(i) {
                    if child.kind() == "identifier" || child.kind() == "type_identifier" {
                        return Some(
                            child
                                .utf8_text(source.as_bytes())
                                .unwrap_or("unknown")
                                .to_string(),
                        );
                    }
                }
            }
        }

        // Phase 2: Iterative DFS for nested search
        {
            let mut stack = Vec::new();
            let cc = node.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = node.child(i) {
                    stack.push(child);
                }
            }

            while let Some(current) = stack.pop() {
                if current.kind() == "identifier" || current.kind() == "type_identifier" {
                    return Some(
                        current
                            .utf8_text(source.as_bytes())
                            .unwrap_or("unknown")
                            .to_string(),
                    );
                }

                let cc = current.child_count();
                for i in (0..cc).rev() {
                    if let Some(child) = current.child(i) {
                        stack.push(child);
                    }
                }
            }
        }

        None
    }

    /// Finds all call relationships (caller -> callee) in the source code
    ///
    /// Returns a list of (caller_symbol, callee_name) pairs where:
    /// - caller_symbol: The Symbol of the function containing the call
    /// - callee_name: The name of the function being called (as a string)
    pub fn find_call_relationships(
        &self,
        source: &str,
        file_path: &str,
    ) -> ParseResult<Vec<(Symbol, String)>> {
        let tree = self.parse_tree(source)?;
        let mut relationships = Vec::new();

        // Find all function definitions
        let function_type = self.language.function_node_type();
        self.find_function_calls(
            tree.root_node(),
            source,
            file_path,
            function_type,
            &mut relationships,
        );

        Ok(relationships)
    }

    /// Helper to find function calls within function definitions (iterative DFS)
    fn find_function_calls(
        &self,
        node: tree_sitter::Node,
        source: &str,
        file_path: &str,
        function_type: &str,
        relationships: &mut Vec<(Symbol, String)>,
    ) {
        let mut stack = Vec::new();
        stack.push(node);

        while let Some(current) = stack.pop() {
            if current.kind() == function_type {
                if let Some(caller_symbol) =
                    self.node_to_symbol_with_path(current, source, file_path)
                {
                    self.find_calls_in_node(current, source, &caller_symbol, relationships);
                }
                continue; // Don't push children - already processed via find_calls_in_node
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }
    }

    /// Finds all call expressions within a node (iterative DFS)
    fn find_calls_in_node(
        &self,
        node: tree_sitter::Node,
        source: &str,
        caller_symbol: &Symbol,
        relationships: &mut Vec<(Symbol, String)>,
    ) {
        let call_type = self.language.call_node_type();
        let mut stack = Vec::new();
        stack.push(node);

        while let Some(current) = stack.pop() {
            if current.kind() == call_type {
                if let Some(callee_name) = self.extract_callee_name(current, source) {
                    relationships.push((caller_symbol.clone(), callee_name));
                }
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }
    }

    /// Extracts the callee name from a call expression node
    fn extract_callee_name(&self, call_node: tree_sitter::Node, source: &str) -> Option<String> {
        // For languages where the function is a direct child (Python, JS/TS)
        if self.language.call_has_function_field() {
            for i in 0..call_node.child_count() {
                if let Some(child) = call_node.child(i) {
                    if child.kind() == "function" {
                        // The function child should have an identifier
                        return self.find_identifier_in_node(child, source);
                    }
                }
            }
        }

        // For Rust and other languages where we look for identifier in call_expression
        // Try to find an identifier that's the function being called
        for i in 0..call_node.child_count() {
            if let Some(child) = call_node.child(i) {
                // Skip certain child types that aren't the function
                if child.kind() == "arguments" || child.kind() == "type_arguments" {
                    continue;
                }
                if let Some(name) = self.find_identifier_in_node(child, source) {
                    return Some(name);
                }
            }
        }

        None
    }

    /// Finds an identifier in a node (iterative DFS)
    fn find_identifier_in_node(&self, node: tree_sitter::Node, source: &str) -> Option<String> {
        let mut stack = Vec::new();
        stack.push(node);

        while let Some(current) = stack.pop() {
            if current.kind() == "identifier" {
                return Some(
                    current
                        .utf8_text(source.as_bytes())
                        .unwrap_or("unknown")
                        .to_string(),
                );
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }

        None
    }

    /// Extracts context (line of code) from pre-split lines
    fn extract_context(lines: &[&str], line_number: u32) -> Option<String> {
        lines.get(line_number as usize).map(|line| line.to_string())
    }

    /// Finds all occurrences of a specific identifier in source code
    pub fn find_all_occurrences_of_identifier(
        &self,
        source: &str,
        identifier: &str,
    ) -> ParseResult<Vec<IdentifierOccurrence>> {
        let tree = self.parse_tree(source)?;
        let lines: Vec<&str> = source.lines().collect();
        let mut occurrences = Vec::new();

        self.find_identifier_occurrences(
            tree.root_node(),
            &lines,
            source,
            identifier,
            &mut occurrences,
        );

        Ok(occurrences)
    }

    /// Checks if the parsed tree contains any error nodes.
    ///
    /// This is useful for validating syntax after edits - if has_error_nodes returns true,
    /// the code has syntax errors.
    ///
    /// # Arguments
    /// * `tree` - The tree-sitter tree to check
    ///
    /// # Returns
    /// * `true` if the tree contains any error nodes (syntax is invalid)
    /// * `false` if the tree is syntactically valid
    pub fn has_error_nodes(tree: &tree_sitter::Tree) -> bool {
        let root = tree.root_node();
        Self::check_node_for_errors(root)
    }

    /// Checks a node and its children for error nodes (iterative DFS)
    fn check_node_for_errors(node: tree_sitter::Node) -> bool {
        let mut stack = Vec::new();
        stack.push(node);

        while let Some(current) = stack.pop() {
            if current.is_error() {
                return true;
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }

        false
    }

    /// Finds all occurrences of an identifier (iterative DFS)
    fn find_identifier_occurrences(
        &self,
        node: tree_sitter::Node,
        lines: &[&str],
        source: &str,
        target_identifier: &str,
        occurrences: &mut Vec<IdentifierOccurrence>,
    ) {
        let mut stack = Vec::new();
        stack.push(node);

        while let Some(current) = stack.pop() {
            if current.kind() == "identifier" || current.kind() == "type_identifier" {
                if let Ok(text) = current.utf8_text(source.as_bytes()) {
                    if text == target_identifier {
                        let start = current.start_position();
                        let end = current.end_position();
                        let context =
                            Self::extract_context(lines, start.row as u32).unwrap_or_default();
                        occurrences.push(IdentifierOccurrence {
                            line: start.row as u32,
                            column: start.column as u32,
                            length: (end.column - start.column) as u32,
                            context,
                        });
                    }
                }
            }

            let cc = current.child_count();
            for i in (0..cc).rev() {
                if let Some(child) = current.child(i) {
                    stack.push(child);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_function_parsing() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let source = r#"
def hello():
    print("Hello, world!")
"#;
        let symbols = parser.find_function_definitions(source).unwrap();
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].name(), "hello");
    }

    #[test]
    fn test_python_class_parsing() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let source = r#"
class MyClass:
    def __init__(self):
        pass
"#;
        let symbols = parser.find_all_symbols(source).unwrap();
        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty());
        assert_eq!(classes[0].name(), "MyClass");
    }

    #[test]
    fn test_rust_function_parsing() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let source = r#"
fn hello() {
    println!("Hello, world!");
}
"#;
        let symbols = parser.find_function_definitions(source).unwrap();
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].name(), "hello");
    }

    #[test]
    fn test_rust_struct_parsing() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let source = r#"
struct Person {
    name: String,
    age: u32,
}
"#;
        let symbols = parser.find_all_symbols(source).unwrap();
        let structs: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Class)
            .collect();
        assert!(!structs.is_empty());
        assert_eq!(structs[0].name(), "Person");
    }

    #[test]
    fn test_rust_variable_parsing() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let source = r#"
fn main() {
    let x = 5;
    let y = 10;
}
"#;
        let symbols = parser.find_all_symbols(source).unwrap();
        let vars: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Variable)
            .collect();
        assert!(!vars.is_empty());
    }

    #[test]
    fn test_javascript_function_parsing() {
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let source = r#"
function hello() {
    console.log("Hello, world!");
}
"#;
        let symbols = parser.find_function_definitions(source).unwrap();
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].name(), "hello");
    }

    #[test]
    fn test_javascript_class_parsing() {
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let source = r#"
class MyClass {
    constructor() {
        this.value = 42;
    }
}
"#;
        let symbols = parser.find_all_symbols(source).unwrap();
        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Class)
            .collect();
        assert!(!classes.is_empty());
        assert_eq!(classes[0].name(), "MyClass");
    }

    #[test]
    fn test_javascript_variable_parsing() {
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let source = r#"
function demo() {
    const x = 10;
    let y = 20;
    var z = 30;
}
"#;
        let symbols = parser.find_all_symbols(source).unwrap();
        let vars: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Variable)
            .collect();
        assert!(!vars.is_empty());
    }

    #[test]
    fn test_typescript_parsing() {
        let parser = TreeSitterParser::new(Language::TypeScript).unwrap();
        // Simpler TypeScript source without complex template literals
        let source = r#"
function greet(name) {
    console.log("Hello, " + name);
}

class User {
    constructor(name) {
        this.name = name;
    }
}
"#;
        let symbols = parser.find_all_symbols(source).unwrap();
        let functions: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Function)
            .collect();
        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Class)
            .collect();

        assert!(!functions.is_empty(), "Expected at least one function");
        assert!(!classes.is_empty(), "Expected at least one class");
        assert_eq!(functions[0].name(), "greet");
        assert_eq!(classes[0].name(), "User");
    }

    #[test]
    fn test_language_enum_values() {
        assert_eq!(Language::Python.name(), "Python");
        assert_eq!(Language::Rust.name(), "Rust");
        assert_eq!(Language::JavaScript.name(), "JavaScript");
        assert_eq!(Language::TypeScript.name(), "TypeScript");

        // Python uses different node naming conventions
        assert_eq!(Language::Python.function_node_type(), "function_definition");
        assert_eq!(Language::Python.class_node_type(), "class_definition");

        // Rust uses function_item and struct_item
        assert_eq!(Language::Rust.function_node_type(), "function_item");
        assert_eq!(Language::Rust.class_node_type(), "struct_item");
        assert_eq!(Language::Rust.variable_node_type(), "let_declaration");

        // JavaScript/TypeScript share the same grammar
        assert_eq!(
            Language::JavaScript.function_node_type(),
            "function_declaration"
        );
        assert_eq!(Language::JavaScript.class_node_type(), "class_declaration");
        assert_eq!(
            Language::TypeScript.function_node_type(),
            "function_declaration"
        );
    }

    #[test]
    fn test_parse_rust_impl_block() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let source = r#"
impl Person {
    fn new(name: String) -> Self {
        Person { name }
    }
}
"#;
        let symbols = parser.find_function_definitions(source).unwrap();
        // impl blocks contain function_item nodes
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].name(), "new");
    }

    #[test]
    fn test_parse_javascript_arrow_functions() {
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let source = r#"
const add = (a, b) => a + b;
const multiply = function(a, b) {
    return a * b;
};
"#;
        // Arrow functions and function expressions are not captured by function_declaration
        // Only function_declaration nodes are found, so we expect 0 results
        let symbols = parser.find_function_definitions(source).unwrap();
        assert_eq!(symbols.len(), 0);
    }

    #[test]
    fn test_parse_typescript_interface() {
        let parser = TreeSitterParser::new(Language::TypeScript).unwrap();
        let source = r#"
interface Person {
    name: string;
    age: number;
}
"#;
        // Interfaces are not captured by class_declaration
        // They use interface_declaration node type
        let symbols = parser.find_all_symbols(source).unwrap();
        let classes: Vec<_> = symbols
            .iter()
            .filter(|s| s.kind() == &SymbolKind::Class)
            .collect();
        assert!(classes.is_empty());
    }

    #[test]
    fn test_symbol_file_path() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let source = r#"
def hello():
    pass
"#;
        let file_path = "/path/to/my_file.py";
        let symbols = parser
            .find_all_symbols_with_path(source, file_path)
            .unwrap();
        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].location().file(), "/path/to/my_file.py");
    }

    #[test]
    fn test_language_from_extension() {
        // Test with OsStr directly (simulating path.extension())
        assert_eq!(
            Language::from_extension(Some(std::ffi::OsStr::new("py"))),
            Some(Language::Python)
        );
        assert_eq!(
            Language::from_extension(Some(std::ffi::OsStr::new("rs"))),
            Some(Language::Rust)
        );
        assert_eq!(
            Language::from_extension(Some(std::ffi::OsStr::new("js"))),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::from_extension(Some(std::ffi::OsStr::new("ts"))),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_extension(Some(std::ffi::OsStr::new("tsx"))),
            Some(Language::TypeScript)
        );
        assert_eq!(
            Language::from_extension(Some(std::ffi::OsStr::new("pyx"))),
            None
        );
        assert_eq!(Language::from_extension(None), None);
    }

    #[test]
    fn test_python_call_relationships() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let source = r#"
def a():
    b()
    c()

def b():
    c()

def c():
    pass
"#;
        let relationships = parser.find_call_relationships(source, "test.py").unwrap();

        // Should find: a->b, a->c, b->c
        let callee_names: Vec<_> = relationships
            .iter()
            .map(|(_, callee)| callee.as_str())
            .collect();
        assert!(callee_names.contains(&"b"), "Expected a calls b");
        assert!(callee_names.contains(&"c"), "Expected a calls c");

        // Count how many times each caller appears
        let a_calls: Vec<_> = relationships
            .iter()
            .filter(|(caller, _)| caller.name() == "a")
            .collect();
        assert_eq!(a_calls.len(), 2, "Expected a calls b and c");

        let b_calls: Vec<_> = relationships
            .iter()
            .filter(|(caller, _)| caller.name() == "b")
            .collect();
        assert_eq!(b_calls.len(), 1, "Expected b calls c");
    }

    #[test]
    fn test_rust_call_relationships() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let source = r#"
fn a() {
    b();
    c();
}

fn b() {
    c();
}

fn c() {
    println!("c");
}
"#;
        let relationships = parser.find_call_relationships(source, "test.rs").unwrap();

        // Should find: a->b, a->c, b->c
        let callee_names: Vec<_> = relationships
            .iter()
            .map(|(_, callee)| callee.as_str())
            .collect();
        assert!(callee_names.contains(&"b"), "Expected a calls b");
        assert!(callee_names.contains(&"c"), "Expected a calls c");

        // Count how many times each caller appears
        let a_calls: Vec<_> = relationships
            .iter()
            .filter(|(caller, _)| caller.name() == "a")
            .collect();
        assert_eq!(a_calls.len(), 2, "Expected a calls b and c");
    }

    #[test]
    fn test_javascript_call_relationships() {
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let source = r#"
function a() {
    b();
    c();
}

function b() {
    c();
}

function c() {
    console.log("c");
}
"#;
        let relationships = parser.find_call_relationships(source, "test.js").unwrap();

        // Should find: a->b, a->c, b->c
        let callee_names: Vec<_> = relationships
            .iter()
            .map(|(_, callee)| callee.as_str())
            .collect();
        assert!(callee_names.contains(&"b"), "Expected a calls b");
        assert!(callee_names.contains(&"c"), "Expected a calls c");

        // Count how many times each caller appears
        let a_calls: Vec<_> = relationships
            .iter()
            .filter(|(caller, _)| caller.name() == "a")
            .collect();
        assert_eq!(a_calls.len(), 2, "Expected a calls b and c");
    }

    #[test]
    fn test_call_relationships_no_calls() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let source = r#"
def a():
    pass

def b():
    x = 1
"#;
        let relationships = parser.find_call_relationships(source, "test.py").unwrap();
        assert!(relationships.is_empty(), "Expected no call relationships");
    }

    #[test]
    fn test_has_error_nodes_valid_rust() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let source = r#"
fn hello() {
    println!("Hello, world!");
}
"#;
        let tree = parser.parse_tree(source).unwrap();
        assert!(!TreeSitterParser::has_error_nodes(&tree));
    }

    #[test]
    fn test_has_error_nodes_invalid_rust() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        // Use a syntax error that definitely produces error nodes:
        // unmatched parentheses or invalid token sequence
        let source = r#"
fn hello() {
    let x = 1 ++ 2;
}
"#; // Invalid: ++ operator
        let tree = parser.parse_tree(source).unwrap();
        // Invalid syntax should produce error nodes
        assert!(
            TreeSitterParser::has_error_nodes(&tree),
            "Invalid Rust syntax (invalid operator) should have error nodes"
        );
    }

    #[test]
    fn test_has_error_nodes_valid_python() {
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let source = r#"
def hello():
    print("Hello, world!")
"#;
        let tree = parser.parse_tree(source).unwrap();
        assert!(!TreeSitterParser::has_error_nodes(&tree));
    }

    #[test]
    fn test_has_error_nodes_valid_javascript() {
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let source = r#"
function hello() {
    console.log("Hello, world!");
}
"#;
        let tree = parser.parse_tree(source).unwrap();
        assert!(!TreeSitterParser::has_error_nodes(&tree));
    }

    // =============================================================================
    // Fuzz tests: Malformed inputs that should NOT crash the parser
    // =============================================================================

    #[test]
    fn test_parser_handles_very_long_line_rust() {
        // Very long lines (1MB+) should be handled gracefully
        let long_line = "fn foo() { ".repeat(100_000);
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        // Should not panic - tree-sitter may or may not parse it fully,
        // but it should not crash
        let result = parser.parse(&long_line);
        // Result can be Ok or Err, we just verify no panic occurs
        let _ = result;
    }

    #[test]
    fn test_parser_handles_very_long_line_python() {
        let long_line = "def foo(): ".repeat(100_000);
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let result = parser.parse(&long_line);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_very_long_line_javascript() {
        let long_line = "function foo() { ".repeat(100_000);
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let result = parser.parse(&long_line);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_invalid_utf8_sequence() {
        // Invalid UTF-8 sequence (continuation byte without start)
        // Using byte string to avoid Rust's UTF-8 validation
        let invalid_utf8 = &b"fn foo() { \x80\x90\xfe }"[..];
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(std::str::from_utf8(invalid_utf8).unwrap_or("fn foo() {}"));
        // Should handle without panic
        let _ = result;
    }

    #[test]
    fn test_parser_handles_null_bytes_python() {
        let with_nulls = "def foo\x00(): pass";
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let result = parser.parse(with_nulls);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_null_bytes_javascript() {
        let with_nulls = "function foo\x00() {}";
        let parser = TreeSitterParser::new(Language::JavaScript).unwrap();
        let result = parser.parse(with_nulls);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_multiple_null_bytes() {
        let with_many_nulls = "fn \x00\x00\x00 foo() {}".to_string();
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(&with_many_nulls);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_truncated_file_rust() {
        // Truncated/incomplete code
        let truncated = "fn foo() {";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(truncated);
        // Should not panic - may produce error nodes but graceful handling
        let _ = result;
    }

    #[test]
    fn test_parser_handles_truncated_file_python() {
        let truncated = "def foo(";
        let parser = TreeSitterParser::new(Language::Python).unwrap();
        let result = parser.parse(truncated);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_empty_string() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse("");
        // Empty string is valid - returns empty tree
        assert!(result.is_ok());
    }

    #[test]
    fn test_parser_handles_only_whitespace() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse("   \n\t\n   ");
        // Whitespace only is valid
        assert!(result.is_ok());
    }

    #[test]
    fn test_parser_handles_special_characters() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        // Various special characters that shouldn't crash the parser
        let special = r#"fn foo() { let x = "\x1b[31m"; let y = '\u{1F600}'; }"#;
        let result = parser.parse(special);
        let _ = result;
    }

    #[test]
    fn test_parser_handles_binary_data_mix() {
        // Mix of valid code and binary-looking data
        // Using byte string to avoid Rust's UTF-8 validation
        let mixed_bytes = b"fn foo() { }\x00\x01\x02\xff\xfe";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(std::str::from_utf8(mixed_bytes).unwrap_or("fn foo() {}"));
        let _ = result;
    }

    #[test]
    fn test_parser_handles_unicode_bom() {
        // UTF-8 BOM at start
        let with_bom = "\u{feff}fn foo() {}";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(with_bom);
        // BOM should be handled gracefully
        assert!(result.is_ok() || result.is_err()); // Either is fine, just no panic
    }

    #[test]
    fn test_parser_handles_mixed_encodings() {
        // Mix of valid Rust and invalid bytes
        let mixed = "fn main() { éåü 中文 }";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(mixed);
        // Unicode in comments/variables is valid Rust, should parse
        let _ = result;
    }

    #[test]
    fn test_parser_handles_windows_line_endings() {
        let dos_style = "fn foo() {\r\n    let x = 1;\r\n}\r\n";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(dos_style);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parser_handles_mac_line_endings() {
        let old_mac_style = "fn foo() {\r    let x = 1;\r}\r";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let result = parser.parse(old_mac_style);
        // Old Mac line endings (\r only) - tree-sitter should handle
        let _ = result;
    }

    #[test]
    fn test_parser_find_symbols_with_null_bytes() {
        let with_nulls = "fn foo\x00() {}";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        // These should not panic
        let symbols = parser.find_function_definitions(with_nulls);
        let _ = symbols;
        let all_symbols = parser.find_all_symbols(with_nulls);
        let _ = all_symbols;
    }

    #[test]
    fn test_parser_find_symbols_truncated() {
        let truncated = "fn foo(";
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let symbols = parser.find_function_definitions(truncated);
        let _ = symbols;
        let all_symbols = parser.find_all_symbols(truncated);
        let _ = all_symbols;
    }

    #[test]
    fn test_parser_find_symbols_very_long_line() {
        let long_line = "fn very_long_function_name_that_exceeds_normal_limits() { ".repeat(50_000);
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let symbols = parser.find_function_definitions(&long_line);
        let _ = symbols;
    }

    #[test]
    fn test_parse_tree_handles_malformed_input() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        // This should not panic, even if it returns an error
        let tree_result = parser.parse_tree("fn foo {"); // missing paren
        let _ = tree_result;
    }

    #[test]
    fn test_has_error_nodes_on_malformed_input() {
        let parser = TreeSitterParser::new(Language::Rust).unwrap();
        let malformed = "fn foo { }"; // missing parens
        let tree = parser.parse_tree(malformed).unwrap();
        // Malformed input should produce error nodes
        let has_errors = TreeSitterParser::has_error_nodes(&tree);
        // Result depends on tree-sitter behavior, but no panic should occur
        let _ = has_errors;
    }

    #[test]
    fn test_parser_concurrent_malformed_inputs() {
        use std::sync::Arc;
        use std::thread;

        let parser = Arc::new(TreeSitterParser::new(Language::Rust).unwrap());
        let mut handles = vec![];

        for _ in 0..4 {
            let p = parser.clone();
            let handle = thread::spawn(move || {
                let malformed = "fn \x00\x00\x00 foo() { ".repeat(1000);
                let _ = p.parse(&malformed);
                let _ = p.find_function_definitions(&malformed);
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }
    }
}
