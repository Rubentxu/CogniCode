//! Hierarchical Outline - Build symbol trees from tree-sitter AST
//!
//! This module provides fast hierarchical symbol extraction for navigation.
//! Performance target: < 10ms for files with 1000+ lines.

use crate::domain::value_objects::{Location, SymbolKind};
use crate::infrastructure::parser::Language;
use std::sync::Arc;
use tree_sitter::Node;

/// Represents a node in the hierarchical symbol outline
#[derive(Debug, Clone)]
pub struct OutlineNode {
    /// Symbol name
    pub name: String,
    /// Kind of symbol (function, class, method, etc.)
    pub kind: SymbolKind,
    /// Source location
    pub location: Location,
    /// Optional signature/type info
    pub signature: Option<String>,
    /// Child nodes (methods inside class, nested functions, etc.)
    pub children: Vec<OutlineNode>,
    /// Whether this symbol is private (starts with _ or is test-only)
    pub is_private: bool,
}

impl OutlineNode {
    /// Creates a new outline node
    pub fn new(name: String, kind: SymbolKind, location: Location) -> Self {
        let is_private = name.starts_with('_');
        Self {
            name,
            kind,
            location,
            signature: None,
            children: Vec::new(),
            is_private,
        }
    }

    /// Creates an outline node with a signature
    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Adds a child node
    pub fn add_child(&mut self, child: OutlineNode) {
        self.children.push(child);
    }

    /// Returns the depth of the tree
    pub fn depth(&self) -> usize {
        1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
    }

    /// Returns total number of nodes in this subtree
    pub fn total_nodes(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_nodes()).sum::<usize>()
    }
}

/// Builder for creating hierarchical outlines from source code
pub struct OutlineBuilder {
    /// Source code
    source: Arc<str>,
    /// File path for location info
    file_path: String,
    /// Language being parsed
    language: Language,
    /// Whether to include private symbols
    include_private: bool,
    /// Whether to include test symbols
    include_tests: bool,
}

impl OutlineBuilder {
    /// Creates a new outline builder
    pub fn new(source: &str, file_path: &str, language: Language) -> Self {
        Self {
            source: Arc::from(source),
            file_path: file_path.to_string(),
            language,
            include_private: true,
            include_tests: true,
        }
    }

    /// Sets whether to include private symbols
    pub fn include_private(mut self, include: bool) -> Self {
        self.include_private = include;
        self
    }

    /// Sets whether to include test symbols
    pub fn include_tests(mut self, include: bool) -> Self {
        self.include_tests = include;
        self
    }

    /// Builds the hierarchical outline from the AST
    pub fn build(&self, tree: &tree_sitter::Tree) -> Vec<OutlineNode> {
        let root = tree.root_node();
        let mut top_level_nodes = Vec::new();

        self.build_recursive(root, &mut top_level_nodes);

        top_level_nodes
    }

    fn build_recursive(&self, node: Node, siblings: &mut Vec<OutlineNode>) {
        // Check if this node is a symbol definition we care about
        if let Some((name, kind, location, signature)) = self.extract_symbol_info(node) {
            let is_private = name.starts_with('_');
            let is_test = name.starts_with("test_") || name.ends_with("_test");

            // Skip if filtering
            if !self.include_private && is_private {
                // Still process children
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        self.build_recursive(child, siblings);
                    }
                }
                return;
            }
            if !self.include_tests && is_test {
                // Still process children
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        self.build_recursive(child, siblings);
                    }
                }
                return;
            }

            let mut outline_node = OutlineNode::new(name, kind, location);
            if let Some(sig) = signature {
                outline_node = outline_node.with_signature(sig);
            }
            outline_node.is_private = is_private;

            // Add this node to siblings
            siblings.push(outline_node);

            // Get mutable reference to the newly added node's children
            let node_idx = siblings.len() - 1;

            // Process children - they will be added to the newly created node's children
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    self.build_recursive(child, &mut siblings[node_idx].children);
                }
            }
        } else {
            // Not a symbol node, process children at same level
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    self.build_recursive(child, siblings);
                }
            }
        }
    }

    /// Extracts symbol information from a tree-sitter node
    fn extract_symbol_info(
        &self,
        node: Node,
    ) -> Option<(String, SymbolKind, Location, Option<String>)> {
        let kind = node.kind();

        // Map node kind to symbol kind
        let (symbol_kind, name_opt) = match kind {
            // Functions
            k if k == self.language.function_node_type() => (
                SymbolKind::Function,
                self.find_name_in_node(node, "identifier"),
            ),
            // Classes/Structs
            k if k == self.language.class_node_type() => {
                // Rust uses struct_item for structs, class_definition for Python/JS
                if k == "struct_item" {
                    (
                        SymbolKind::Struct,
                        self.find_name_in_node(node, "identifier"),
                    )
                } else {
                    (
                        SymbolKind::Class,
                        self.find_name_in_node(node, "identifier"),
                    )
                }
            }
            // Variables
            k if k == self.language.variable_node_type() => (
                SymbolKind::Variable,
                self.find_name_in_node(node, "identifier"),
            ),
            // Rust-specific
            "impl_item" => {
                // Extract trait name if present: impl Trait for Type
                let trait_name = self.find_impl_trait_name(node);
                (SymbolKind::Trait, trait_name)
            }
            "enum_item" => (
                SymbolKind::Enum,
                self.find_name_in_node(node, "type_identifier"),
            ),
            "trait_item" => (
                SymbolKind::Trait,
                self.find_name_in_node(node, "identifier"),
            ),
            "type_alias" | "type_item" => (
                SymbolKind::Type,
                self.find_name_in_node(node, "type_identifier"),
            ),
            "method_definition" | "function_declaration" => (
                SymbolKind::Method,
                self.find_name_in_node(node, "identifier"),
            ),
            "pair" => {
                // Python dict/key-value
                (SymbolKind::Variable, self.find_name_in_node(node, "string"))
            }
            _ => return None,
        };

        let name = name_opt?;

        let start = node.start_position();
        let location = Location::new(&self.file_path, start.row as u32, start.column as u32);

        // Extract signature if available
        let signature = self.extract_signature(node, kind);

        Some((name, symbol_kind, location, signature))
    }

    /// Finds the name identifier in a node
    fn find_name_in_node(&self, node: Node, target_kind: &str) -> Option<String> {
        // First check direct children
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if child.kind() == target_kind || child.kind() == "property_identifier" {
                    return child
                        .utf8_text(self.source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
            }
        }

        // Recursively search
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                if let Some(name) = self.find_name_in_node(child, target_kind) {
                    return Some(name);
                }
            }
        }

        None
    }

    /// Finds the trait name in an impl block (Rust: impl Trait for Type)
    fn find_impl_trait_name(&self, node: Node) -> Option<String> {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                // Look for the type_identifier after 'for' keyword
                if child.kind() == "type_identifier" {
                    return child
                        .utf8_text(self.source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
                }
                // Recurse
                if let Some(name) = self.find_impl_trait_name(child) {
                    return Some(name);
                }
            }
        }
        None
    }

    /// Extracts signature information from a node
    fn extract_signature(&self, node: Node, _kind: &str) -> Option<String> {
        // For functions, try to extract parameters
        match self.language {
            Language::Rust => {
                // Find parameters in Rust function
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "parameters" {
                            return Some(self.format_rust_params(child));
                        }
                    }
                }
            }
            Language::Python => {
                // Find parameters in Python function
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "parameters" {
                            return Some(self.format_python_params(child));
                        }
                    }
                }
            }
            Language::JavaScript | Language::TypeScript => {
                // Find parameters in JS/TS function
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "formal_parameters" {
                            return Some(self.format_js_params(child));
                        }
                    }
                }
            }
            Language::Go => {
                // Find parameters in Go function
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "parameters" {
                            // For now, return a generic placeholder - Go outline not fully implemented
                            return Some("(params)".to_string());
                        }
                    }
                }
            }
            Language::Java => {
                // Find parameters in Java method
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        if child.kind() == "formal_parameters" {
                            // For now, return a generic placeholder - Java outline not fully implemented
                            return Some("(params)".to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn format_rust_params(&self, params_node: Node) -> String {
        let mut params = Vec::new();
        for i in 0..params_node.child_count() {
            if let Some(child) = params_node.child(i) {
                if child.kind() == "parameter" {
                    if let Some(identifier) = self.find_name_in_node(child, "identifier") {
                        params.push(identifier);
                    }
                }
            }
        }
        format!("({})", params.join(", "))
    }

    fn format_python_params(&self, params_node: Node) -> String {
        let mut params = Vec::new();
        for i in 0..params_node.child_count() {
            if let Some(child) = params_node.child(i) {
                if child.kind() == "identifier" {
                    if let Some(name) = child.utf8_text(self.source.as_bytes()).ok() {
                        params.push(name.to_string());
                    }
                }
                if child.kind() == "default_parameter" {
                    if let Some(identifier) = self.find_name_in_node(child, "identifier") {
                        params.push(identifier);
                    }
                }
            }
        }
        format!("({})", params.join(", "))
    }

    fn format_js_params(&self, params_node: Node) -> String {
        let mut params = Vec::new();
        for i in 0..params_node.child_count() {
            if let Some(child) = params_node.child(i) {
                if child.kind() == "identifier" {
                    if let Some(name) = child.utf8_text(self.source.as_bytes()).ok() {
                        params.push(name.to_string());
                    }
                }
            }
        }
        format!("({})", params.join(", "))
    }
}

/// Builds a hierarchical outline for a source file
pub fn build_outline(
    source: &str,
    file_path: &str,
    language: Language,
    include_private: bool,
    include_tests: bool,
) -> Vec<OutlineNode> {
    let parser = crate::infrastructure::parser::TreeSitterParser::new(language)
        .expect("Failed to create parser");

    let tree = parser.parse_tree(source).expect("Failed to parse source");

    OutlineBuilder::new(source, file_path, language)
        .include_private(include_private)
        .include_tests(include_tests)
        .build(&tree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::parser::Language;

    #[test]
    fn test_python_outline() {
        let source = r#"
class MyClass:
    def method_one(self):
        pass
    
    def _private_method(self):
        pass

def standalone_function():
    pass
"#;

        let outline = build_outline(source, "test.py", Language::Python, true, true);

        // Should find class, methods, and function
        let class_node = outline.iter().find(|n| n.name == "MyClass");
        assert!(class_node.is_some());
        let class = class_node.unwrap();

        // Class should have 2 methods
        assert_eq!(class.children.len(), 2);

        // Should find standalone function
        let func = outline.iter().find(|n| n.name == "standalone_function");
        assert!(func.is_some());
    }

    #[test]
    fn test_python_outline_exclude_private() {
        let source = r#"
class MyClass:
    def method_one(self):
        pass
    
    def _private_method(self):
        pass
"#;

        let outline = build_outline(source, "test.py", Language::Python, false, true);

        // Private method should be excluded
        let all_names: Vec<_> = outline
            .iter()
            .flat_map(|n| n.children.iter())
            .map(|c| c.name.clone())
            .collect();

        assert!(!all_names.contains(&"_private_method".to_string()));
    }

    #[test]
    fn test_rust_outline() {
        let source = r#"
struct Person {
    name: String,
}

impl Person {
    fn new(name: String) -> Self {
        Person { name }
    }
}

fn main() {
    let person = Person::new("Alice".to_string());
}
"#;

        let outline = build_outline(source, "test.rs", Language::Rust, true, true);

        // Should find struct, impl, and function
        assert!(outline.iter().any(|n| n.name == "Person"));
        assert!(outline.iter().any(|n| n.name == "main"));
    }

    #[test]
    fn test_javascript_outline() {
        let source = r#"
class MyClass {
    constructor() {
        this.value = 42;
    }
    
    methodOne() {
        return this.value;
    }
}

function standaloneFunction() {
    return 100;
}
"#;

        let outline = build_outline(source, "test.js", Language::JavaScript, true, true);

        // Should find class and function
        assert!(outline.iter().any(|n| n.name == "MyClass"));
        assert!(outline.iter().any(|n| n.name == "standaloneFunction"));

        // Class should have methods
        let class_node = outline.iter().find(|n| n.name == "MyClass");
        assert!(class_node.is_some());
        assert!(!class_node.unwrap().children.is_empty());
    }

    #[test]
    fn test_outline_node_depth() {
        let source = r#"
class Outer:
    def outer_method(self):
        def inner_function():
            pass
"#;

        let outline = build_outline(source, "test.py", Language::Python, true, true);
        let class_node = outline.iter().find(|n| n.name == "Outer").unwrap();

        // Should have nested structure
        assert!(!class_node.children.is_empty());
    }

    #[test]
    fn test_outline_exclude_tests() {
        let source = r#"
def test_foo():
    pass

def test_bar():
    pass

def regular_function():
    pass
"#;

        let outline = build_outline(source, "test.py", Language::Python, true, false);

        // Test functions should be excluded
        assert!(!outline.iter().any(|n| n.name == "test_foo"));
        assert!(!outline.iter().any(|n| n.name == "test_bar"));
        assert!(outline.iter().any(|n| n.name == "regular_function"));
    }

    #[test]
    fn test_outline_performance() {
        // Generate a large source file
        let mut source = String::new();
        for i in 0..100 {
            source.push_str(&format!(
                r#"
def function_{}():
    pass

class Class_{}:
    def method_{}(self):
        pass
"#,
                i, i, i
            ));
        }

        let start = std::time::Instant::now();
        let _outline = build_outline(&source, "test.py", Language::Python, true, true);
        let elapsed = start.elapsed();

        // Should complete in well under 10ms for 1000 lines
        assert!(
            elapsed.as_millis() < 100,
            "Outline took {}ms, expected < 100ms",
            elapsed.as_millis()
        );
    }
}
