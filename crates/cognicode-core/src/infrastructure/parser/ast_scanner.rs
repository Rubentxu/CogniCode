//! AST Scanner implementation using tree-sitter

use crate::domain::traits::{AstScanner, ParseResult, ScannedNode};
use crate::domain::value_objects::{Location, SourceRange};

/// Tree-sitter based AST scanner implementation
pub struct TreeSitterAstScanner;

impl TreeSitterAstScanner {
    /// Creates a new AST scanner
    pub fn new() -> Self {
        Self
    }

    fn node_to_range_impl(&self, node: &tree_sitter::Node) -> SourceRange {
        let start = node.start_position();
        let end = node.end_position();

        let start_loc = Location::new("source", start.row as u32, start.column as u32);
        let end_loc = Location::new("source", end.row as u32, end.column as u32);

        SourceRange::new(start_loc, end_loc)
    }

    fn scan_recursive<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        parent_children: &mut Vec<ScannedNode<'a>>,
    ) {
        let range = self.node_to_range_impl(&node);
        let mut scanned = ScannedNode::new(node.kind(), range);
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.scan_recursive(child, &mut scanned.children);
            }
        }
        parent_children.push(scanned);
    }

    fn scan_by_type<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        node_type: &str,
        results: &mut Vec<ScannedNode<'a>>,
    ) {
        if node.kind() == node_type {
            results.push(ScannedNode::new(
                node_type.to_string(),
                self.node_to_range_impl(&node),
            ));
        }
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                self.scan_by_type(child, node_type, results);
            }
        }
    }
}

impl AstScanner for TreeSitterAstScanner {
    fn scan<'a>(
        &self,
        root: &'a tree_sitter::Tree,
        _source: &str,
    ) -> ParseResult<Vec<ScannedNode<'a>>> {
        let mut results = Vec::new();
        self.scan_recursive(root.root_node(), &mut results);
        Ok(results)
    }

    fn find_nodes_by_type<'a>(
        &self,
        root: &'a tree_sitter::Tree,
        node_type: &str,
    ) -> ParseResult<Vec<ScannedNode<'a>>> {
        let mut results = Vec::new();
        self.scan_by_type(root.root_node(), node_type, &mut results);
        Ok(results)
    }

    fn get_node_text(&self, node: &tree_sitter::Node, source: &str) -> String {
        node.utf8_text(source.as_bytes())
            .unwrap_or_default()
            .to_string()
    }

    fn node_to_range(&self, node: &tree_sitter::Node) -> SourceRange {
        self.node_to_range_impl(node)
    }
}

impl Default for TreeSitterAstScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
        parser.set_language(&language).unwrap();
        parser.parse(source, None).unwrap()
    }

    fn parse_python(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
        parser.set_language(&language).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_scan_rust_simple_code() {
        let scanner = TreeSitterAstScanner::new();
        let source = "fn main() {}";
        let tree = parse_rust(source);
        let result = scanner.scan(&tree, source);
        assert!(result.is_ok());
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());
        assert!(nodes[0].has_children());
    }

    #[test]
    fn test_scan_python_simple_code() {
        let scanner = TreeSitterAstScanner::new();
        let source = "def foo():\n    pass";
        let tree = parse_python(source);
        let result = scanner.scan(&tree, source);
        assert!(result.is_ok());
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_scan_empty_source() {
        let scanner = TreeSitterAstScanner::new();
        let source = "";
        let tree = parse_rust(source);
        let result = scanner.scan(&tree, source);
        assert!(result.is_ok());
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_find_nodes_by_type_function() {
        let scanner = TreeSitterAstScanner::new();
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let tree = parse_rust(source);
        let result = scanner.find_nodes_by_type(&tree, "function_item");
        assert!(result.is_ok());
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());
        assert_eq!(nodes[0].node_type, "function_item");
    }

    #[test]
    fn test_find_nodes_by_type_identifier() {
        let scanner = TreeSitterAstScanner::new();
        let source = "fn main() { let x = 1; }";
        let tree = parse_rust(source);
        let result = scanner.find_nodes_by_type(&tree, "identifier");
        assert!(result.is_ok());
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());
        for node in &nodes {
            assert_eq!(node.node_type, "identifier");
        }
    }

    #[test]
    fn test_scanned_node_fields() {
        let scanner = TreeSitterAstScanner::new();
        let source = "fn test() {}";
        let tree = parse_rust(source);
        let result = scanner.scan(&tree, source).unwrap();
        let node = &result[0];
        assert_eq!(node.node_type, "source_file");
        assert!(node.range.start().line() == 0);
        assert!(node.range.start().column() == 0);
        assert!(node.children.len() > 0);
        assert!(node.symbol.is_none());
    }

    #[test]
    fn test_get_node_text() {
        let scanner = TreeSitterAstScanner::new();
        let source = "fn hello() {}";
        let tree = parse_rust(source);
        let root = tree.root_node();
        let text = scanner.get_node_text(&root, source);
        assert!(text.contains("fn"));
    }

    #[test]
    fn test_node_to_range() {
        let scanner = TreeSitterAstScanner::new();
        let source = "fn foo() {}";
        let tree = parse_rust(source);
        let root = tree.root_node();
        let range = scanner.node_to_range(&root);
        assert!(range.start().line() == 0);
        assert!(range.start().column() == 0);
        assert!(range.end().line() == 0);
        assert!(range.end().column() == source.len() as u32);
    }
}
