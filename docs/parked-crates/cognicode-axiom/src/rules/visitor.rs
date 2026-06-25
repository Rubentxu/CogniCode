//! Visitor trait and traversal utilities for AST walking
//!
//! This module provides a reusable visitor pattern for tree-sitter ASTs.
//! Unlike the SubscriptionRule pattern which is pull-based (rules ask for specific nodes),
//! this visitor is push-based (walks the entire tree and notifies of interesting nodes).
//!
//! # Design
//!
//! - `Visitor` trait defines hooks for different node types
//! - `DepthFirst` traversal is provided
//! - Visitors are composable - you can chain visitors together
//!
//! # Example
//!
//! ```rust
//! use tree_sitter::Parser;
//! use cognicode_axiom::rules::visitor::{Visitor, DepthFirst};
//!
//! struct MyVisitor {
//!     functions: Vec<String>,
//! }
//!
//! impl Visitor for MyVisitor {
//!     fn on_function(&mut self, node: tree_sitter::Node, source: &str) {
//!         if let Some(name) = node.child_by_field_name("name") {
//!             let text = &source[name.start_byte()..name.end_byte()];
//!             self.functions.push(text.to_string());
//!         }
//!     }
//! }
//!
//! fn analyze(source: &str) {
//!     let mut parser = Parser::new();
//!     parser.set_language(&tree_sitter::LANGUAGE_JSON).unwrap();
//!     let tree = parser.parse(source, None).unwrap();
//!
//!     let mut visitor = MyVisitor { functions: vec![] };
//!     DepthFirst::new().walk(&tree.root_node(), source, &mut visitor);
//! }
//! ```

use tree_sitter::Node;
use std::collections::HashSet;

/// A visitor that receives callbacks during AST traversal.
///
/// Implement this trait to define behavior for different node types.
/// Each `on_*` method corresponds to a node type of interest.
///
/// Default implementations are empty (no-op).
pub trait Visitor {
    /// Called for function/method definitions
    fn on_function(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for let/const/var declarations
    fn on_binding(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for parameter definitions
    fn on_parameter(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for import/use statements
    fn on_import(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for type definitions (struct, enum, trait, impl)
    fn on_type(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for struct field definitions
    fn on_field(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for function/method calls
    fn on_call(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for identifier references (not definitions)
    fn on_identifier(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for struct/enum variant definitions
    fn on_variant(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for module declarations (mod foo; or mod foo {})
    fn on_module(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for any node (callback before children)
    fn on_enter(&mut self, _node: Node<'_>, _source: &str) {}

    /// Called for any node (callback after children)
    fn on_exit(&mut self, _node: Node<'_>, _source: &str) {}

    /// Returns which node kinds this visitor cares about.
    /// Used for optimization - if empty, visits all nodes.
    /// If not empty, only visits matching nodes (but still calls on_enter/on_exit for all).
    fn interested_in(&self) -> HashSet<&'static str> {
        HashSet::new()
    }
}

/// Extract text from a node using source bytes
pub fn node_text(node: Node<'_>, source: &str) -> String {
    source[node.start_byte()..node.end_byte()].to_string()
}

/// Depth-first AST traversal with visitor callbacks
pub struct DepthFirst;

impl DepthFirst {
    pub fn new() -> Self {
        Self
    }

    /// Walk the AST rooted at `root`, calling visitor methods.
    pub fn walk(&self, root: &Node<'_>, source: &str, visitor: &mut dyn Visitor) {
        self.walk_node(root.clone(), source, visitor);
    }

    fn walk_node(&self, node: Node<'_>, source: &str, visitor: &mut dyn Visitor) {
        let interested = visitor.interested_in();

        // Check if we should process this node
        let kind = node.kind();
        let should_notify = interested.is_empty() || interested.contains(kind);

        if should_notify {
            visitor.on_enter(node.clone(), source);

            // Dispatch to specific callbacks based on node kind
            match kind {
                "function_item" | "function_declaration" | "method_declaration" | "closure_expression" => {
                    visitor.on_function(node.clone(), source);
                }
                "let_declaration" | "const_declaration" | "const_item" | "variable_declaration" => {
                    visitor.on_binding(node.clone(), source);
                }
                "parameter" | "self_parameter" => {
                    visitor.on_parameter(node.clone(), source);
                }
                "use_declaration" | "import_declaration" | "extern_declaration" | "module_declaration" | "mod_declaration" => {
                    visitor.on_import(node.clone(), source);
                }
                "struct_declaration" | "struct_item" | "enum_declaration" | "enum_item" | "trait_declaration" | "trait_item" | "impl_declaration" | "impl_item" => {
                    visitor.on_type(node.clone(), source);
                }
                "field_declaration" | "field_identifier" => {
                    visitor.on_field(node.clone(), source);
                }
                "call_expression" | "gate_expression" => {
                    visitor.on_call(node.clone(), source);
                }
                "identifier" | "field_access_expression" => {
                    visitor.on_identifier(node.clone(), source);
                }
                "enum_variant" | "variant" => {
                    visitor.on_variant(node.clone(), source);
                }
                "mod_item" => {
                    visitor.on_module(node.clone(), source);
                }
                _ => {}
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.walk_node(child, source, visitor);
        }

        if should_notify {
            visitor.on_exit(node, source);
        }
    }
}

impl Default for DepthFirst {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::infrastructure::parser::Language;

    // Test utilities
    fn parse_rust(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&Language::Rust.to_ts_language())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_visitor_function_detection() {
        let source = r#"
fn hello() {
    println!("world");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let tree = parse_rust(source);
        let mut visitor = FunctionCounter { count: 0 };

        DepthFirst::new().walk(&tree.root_node(), source, &mut visitor);

        assert_eq!(visitor.count, 2);
    }

    #[test]
    fn test_visitor_import_detection() {
        let source = r#"
use std::collections::HashMap;
use std::io::{self, Read};
use crate::SomeTrait;
"#;
        let tree = parse_rust(source);
        let mut visitor = ImportCollector { imports: vec![] };

        DepthFirst::new().walk(&tree.root_node(), source, &mut visitor);

        assert_eq!(visitor.imports.len(), 3);
    }

    #[test]
    fn test_visitor_binding_detection() {
        let source = r#"
fn main() {
    let x = 5;
    let y = 10;
    const Z: i32 = 20;
}
"#;
        let tree = parse_rust(source);
        let mut visitor = BindingCollector { bindings: vec![] };

        DepthFirst::new().walk(&tree.root_node(), source, &mut visitor);

        assert_eq!(visitor.bindings.len(), 3); // x, y, Z
    }

    #[test]
    fn test_node_text() {
        let source = "fn hello() {}";
        let tree = parse_rust(source);
        let root = tree.root_node();

        let text = node_text(root, source);
        assert!(text.contains("hello"));
    }

    // Test visitors
    struct FunctionCounter { count: usize }
    struct ImportCollector { imports: Vec<String> }
    struct BindingCollector { bindings: Vec<String> }

    impl Visitor for FunctionCounter {
        fn on_function(&mut self, node: Node<'_>, source: &str) {
            self.count += 1;
            let _ = node_text(node, source); // Verify we can access source
        }
    }

    impl Visitor for ImportCollector {
        fn on_import(&mut self, node: Node<'_>, source: &str) {
            self.imports.push(node_text(node, source));
        }
    }

    impl Visitor for BindingCollector {
        fn on_binding(&mut self, node: Node<'_>, _source: &str) {
            // Extract pattern name
            if let Some(pattern) = node.child_by_field_name("pattern") {
                self.bindings.push(node_text(pattern, _source));
            } else if let Some(pattern) = node.child_by_field_name("declarator") {
                // For const_item, the declarator is the name
                self.bindings.push(node_text(pattern, _source));
            } else if node.kind() == "const_item" {
                // For const_item, try to find identifier directly
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    let child_kind = child.kind();
                    if child_kind == "identifier" || child_kind == "type_identifier" {
                        self.bindings.push(node_text(child, _source));
                        break; // Only take the first identifier
                    }
                }
            }
        }
    }
}
