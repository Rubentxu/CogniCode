//! Symbol Code Retrieval - Get full source code of a symbol
//!
//! This module provides functionality to retrieve the complete source code
//! of a symbol including its docstrings/comments.

use crate::domain::value_objects::Location;
use crate::infrastructure::parser::{Language, TreeSitterParser};
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Cached symbol code entry
#[derive(Debug, Clone)]
pub struct CachedSymbolCode {
    /// The full source code of the symbol
    pub code: String,
    /// Docstring/comment above the symbol (if found)
    pub docstring: Option<String>,
    /// Starting line number (1-indexed)
    pub start_line: u32,
    /// Ending line number (1-indexed)
    pub end_line: u32,
}

impl CachedSymbolCode {
    /// Creates a new cached symbol code entry
    pub fn new(code: String, docstring: Option<String>, start_line: u32, end_line: u32) -> Self {
        Self {
            code,
            docstring,
            start_line,
            end_line,
        }
    }
}

/// Cache key for symbol code
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolCodeKey {
    file_path: String,
    line: u32,
    column: u32,
}

impl SymbolCodeKey {
    /// Creates a new cache key from a location
    pub fn from_location(location: &Location) -> Self {
        Self {
            file_path: location.file().to_string(),
            line: location.line(),
            column: location.column(),
        }
    }

    /// Creates a new cache key from file path and coordinates
    pub fn new(file_path: &str, line: u32, column: u32) -> Self {
        Self {
            file_path: file_path.to_string(),
            line,
            column,
        }
    }
}

/// In-memory cache for symbol code retrieval
pub struct SymbolCodeCache {
    cache: DashMap<SymbolCodeKey, Arc<CachedSymbolCode>>,
}

impl SymbolCodeCache {
    /// Creates a new symbol code cache
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Gets cached symbol code if available
    pub fn get(&self, key: &SymbolCodeKey) -> Option<Arc<CachedSymbolCode>> {
        self.cache.get(key).map(|e| e.clone())
    }

    /// Stores symbol code in cache
    pub fn insert(&self, key: SymbolCodeKey, value: CachedSymbolCode) {
        self.cache.insert(key, Arc::new(value));
    }

    /// Clears the cache
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Returns the number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns true if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for SymbolCodeCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for retrieving symbol source code
pub struct SymbolCodeService {
    cache: Arc<SymbolCodeCache>,
    parsers: Mutex<HashMap<Language, TreeSitterParser>>,
}

impl SymbolCodeService {
    /// Creates a new symbol code service
    pub fn new() -> Self {
        Self {
            cache: Arc::new(SymbolCodeCache::new()),
            parsers: Mutex::new(HashMap::new()),
        }
    }

    /// Returns a reference to the cache
    pub fn cache(&self) -> &SymbolCodeCache {
        &self.cache
    }

    /// Gets the source code of a symbol at the given location
    pub fn get_symbol_code(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<CachedSymbolCode, String> {
        let key = SymbolCodeKey::new(file_path, line, column);

        if let Some(cached) = self.cache.get(&key) {
            return Ok((*cached).clone());
        }

        let source = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

        let path = Path::new(file_path);
        let extension = path.extension().and_then(|e| e.to_str());

        let language =
            Language::from_extension(extension.as_ref().map(|s| std::ffi::OsStr::new(s)))
                .ok_or_else(|| "Unsupported file type".to_string())?;

        let parser = {
            let mut parsers = self.parsers.lock().unwrap();
            parsers
                .entry(language)
                .or_insert_with(|| TreeSitterParser::new(language).unwrap())
                .clone()
        };

        let tree = parser
            .parse_tree(&source)
            .map_err(|e| format!("Failed to parse: {}", e))?;

        let target_line = line.saturating_sub(1);
        let node = find_node_at_position(tree.root_node(), target_line as u32, column);

        if let Some(node) = node {
            // Walk ancestors to find enclosing function/class/method instead of
            // returning the smallest AST node (e.g., an identifier or literal)
            let interesting_node = find_enclosing_symbol(node);

            let start_line = interesting_node.start_position().row as u32 + 1;
            let end_line = interesting_node.end_position().row as u32 + 1;

            let code = interesting_node
                .utf8_text(source.as_bytes())
                .map(|s| s.to_string())
                .unwrap_or_default();

            let docstring = extract_docstring(&source, start_line);

            let result = CachedSymbolCode::new(code, docstring, start_line, end_line);

            self.cache.insert(key, result.clone());

            Ok(result)
        } else {
            Err(format!(
                "No symbol found at {}:{}:{}",
                file_path, line, column
            ))
        }
    }

    /// Gets symbol code by Location
    pub fn get_symbol_code_by_location(
        &self,
        location: &Location,
    ) -> Result<CachedSymbolCode, String> {
        self.get_symbol_code(location.file(), location.line(), location.column())
    }

    /// Clears the cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

impl Default for SymbolCodeService {
    fn default() -> Self {
        Self::new()
    }
}

/// Finds a tree-sitter node at the given position
fn find_node_at_position(
    root: tree_sitter::Node,
    line: u32,
    column: u32,
) -> Option<tree_sitter::Node> {
    let mut result = None;

    fn search<'a>(
        node: tree_sitter::Node<'a>,
        line: u32,
        column: u32,
        result: &mut Option<tree_sitter::Node<'a>>,
    ) {
        let start = node.start_position();
        let end = node.end_position();

        // Check if this node contains the position
        if start.row <= line as usize && line as usize <= end.row {
            // For single-line nodes, also check column
            if start.row == line as usize && end.row == line as usize {
                if column < start.column as u32 || column > end.column as u32 {
                    return;
                }
            }

            // This node contains the position, but we want the most specific node
            // So continue searching children
            let mut found_child = false;
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    let child_start = child.start_position();
                    let child_end = child.end_position();

                    if child_start.row <= line as usize && line as usize <= child_end.row {
                        found_child = true;
                        search(child, line, column, result);
                    }
                }
            }

            // If no child contains the position, this is the node
            if !found_child || result.is_none() {
                *result = Some(node);
            }
        }
    }

    search(root, line, column, &mut result);
    result
}

/// Node kinds that represent meaningful symbols (functions, classes, methods, etc.)
const INTERESTING_NODE_KINDS: &[&str] = &[
    // Rust
    "function_item",
    "impl_item",
    "struct_item",
    "enum_item",
    "trait_item",
    "type_item",
    "const_item",
    "static_item",
    "mod_item",
    // JavaScript / TypeScript
    "function_declaration",
    "function_expression",
    "arrow_function",
    "class_declaration",
    "class_expression",
    "method_definition",
    "variable_declarator",
    // Python
    "function_definition",
    "class_definition",
    "decorated_definition",
];

/// Walks ancestors from the given node to find the first "interesting" enclosing symbol.
/// If no interesting ancestor is found, returns the original node (backward compatible).
fn find_enclosing_symbol(node: tree_sitter::Node) -> tree_sitter::Node {
    let mut current = node;
    let mut best_interesting = None;

    while let Some(parent) = current.parent() {
        if let Some(kind) = INTERESTING_NODE_KINDS.iter().find(|k| parent.kind() == **k) {
            let _ = kind; // used for matching
            best_interesting = Some(parent);
            break;
        }
        current = parent;
    }

    best_interesting.unwrap_or(node)
}

/// Extracts docstring/comments above a symbol
fn extract_docstring(source: &str, symbol_line: u32) -> Option<String> {
    if symbol_line <= 1 {
        return None;
    }

    let lines: Vec<&str> = source.lines().collect();
    let symbol_idx = (symbol_line - 1) as usize;

    if symbol_idx >= lines.len() {
        return None;
    }

    let mut idx = symbol_idx;

    // Skip blank lines between docstring/comments and symbol
    while idx > 0 && lines[idx].trim().is_empty() {
        idx -= 1;
    }

    // Now idx points to the first non-blank line (0-indexed)
    // Check this line first - it might be a docstring or comment directly above the symbol
    let curr_line = lines[idx].trim();

    // Check for triple-quoted docstrings (Python) - might be at idx (docstring position)
    if curr_line.starts_with("\"\"\"") || curr_line.starts_with("'''") {
        let quote = if curr_line.starts_with("\"\"\"") {
            "\"\"\""
        } else {
            "'''"
        };

        // Single line docstring: """content""" or '''content'''
        if curr_line.ends_with(quote) && curr_line.len() > 6 {
            let start = quote.len();
            let end = curr_line.len() - quote.len();
            return Some(curr_line[start..end].trim().to_string());
        }

        // Multi-line docstring - search backwards for closing quote
        let mut doc_content = Vec::new();
        let mut doc_idx = idx;
        while doc_idx > 0 {
            let line = lines[doc_idx].trim();
            if line.ends_with(quote) {
                doc_content.push(line[..line.len() - quote.len()].to_string());
                doc_content.reverse();
                return Some(doc_content.join("\n"));
            }
            doc_content.push(line.to_string());
            doc_idx -= 1;
        }
        // If we didn't find closing quote, return what we have
        if !doc_content.is_empty() {
            doc_content.reverse();
            return Some(doc_content.join("\n"));
        }
    }

    // Check for single-line comment directly above (// or #)
    if curr_line.starts_with("//") || curr_line.starts_with("#") {
        let comment = if curr_line.starts_with("//") {
            &curr_line[2..]
        } else {
            &curr_line[1..]
        };
        return Some(comment.trim().to_string());
    }

    // Check the line below idx (idx-1 in 0-indexed terms) - might be a docstring/comment
    // This handles cases where the docstring/comment is one line above with no blanks
    if idx > 0 {
        let prev_line = lines[idx - 1].trim();

        // Check for triple-quoted docstrings on the line above
        if prev_line.starts_with("\"\"\"") || prev_line.starts_with("'''") {
            let quote = if prev_line.starts_with("\"\"\"") {
                "\"\"\""
            } else {
                "'''"
            };

            // Single line docstring
            if prev_line.ends_with(quote) && prev_line.len() > 6 {
                let start = quote.len();
                let end = prev_line.len() - quote.len();
                return Some(prev_line[start..end].trim().to_string());
            }

            // Multi-line docstring
            let mut doc_content = Vec::new();
            let mut doc_idx = idx - 1;
            while doc_idx > 0 {
                let line = lines[doc_idx].trim();
                if line.ends_with(quote) {
                    doc_content.push(line[..line.len() - quote.len()].to_string());
                    doc_content.reverse();
                    return Some(doc_content.join("\n"));
                }
                doc_content.push(line.to_string());
                doc_idx -= 1;
            }
            if !doc_content.is_empty() {
                doc_content.reverse();
                return Some(doc_content.join("\n"));
            }
        }

        // Single-line comment check on line above
        if prev_line.starts_with("//") || prev_line.starts_with("#") {
            let comment = if prev_line.starts_with("//") {
                &prev_line[2..]
            } else {
                &prev_line[1..]
            };
            return Some(comment.trim().to_string());
        }
    }

    // Look for consecutive comment lines (Rust /// doc comments)
    let mut comment_block: Vec<&str> = Vec::new();
    let mut check_idx = idx;

    // Collect consecutive comment lines going backwards
    while check_idx > 0 {
        let line = lines[check_idx - 1].trim();
        if line.starts_with("//") || line.starts_with("#") {
            // Skip shebang
            if line.starts_with("#!") {
                check_idx -= 1;
                continue;
            }
            let comment_content = if line.starts_with("//") {
                &line[2..]
            } else {
                &line[1..]
            };
            comment_block.push(comment_content);
            check_idx -= 1;
        } else {
            break;
        }
    }

    if !comment_block.is_empty() {
        comment_block.reverse();
        return Some(comment_block.join("\n"));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = SymbolCodeCache::new();
        let key = SymbolCodeKey::new("test.rs", 10, 5);
        let value = CachedSymbolCode::new("fn foo() {}".to_string(), None, 10, 10);

        cache.insert(key.clone(), value.clone());

        assert_eq!(cache.get(&key).unwrap().code, "fn foo() {}");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_clear() {
        let cache = SymbolCodeCache::new();
        let key = SymbolCodeKey::new("test.rs", 10, 5);
        let value = CachedSymbolCode::new("fn foo() {}".to_string(), None, 10, 10);

        cache.insert(key, value);
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_symbol_code_key_equality() {
        let key1 = SymbolCodeKey::new("test.rs", 10, 5);
        let key2 = SymbolCodeKey::new("test.rs", 10, 5);
        let key3 = SymbolCodeKey::new("test.rs", 10, 6);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_extract_python_docstring() {
        let source = r#"
def foo():
    '''This is a docstring'''
    pass
"#;

        let docstring = extract_docstring(source, 3);
        assert!(docstring.is_some());
        assert!(docstring.unwrap().contains("docstring"));
    }

    #[test]
    fn test_extract_rust_docstring() {
        let source = r#"
/// This is a doc comment
fn foo() {
    // code
}
"#;

        let docstring = extract_docstring(source, 2);
        assert!(docstring.is_some());
        assert!(docstring.unwrap().contains("doc comment"));
    }

    #[test]
    fn test_extract_single_line_comment() {
        let source = r#"
// Single line comment
fn foo() {}
"#;

        let docstring = extract_docstring(source, 2);
        assert!(docstring.is_some());
        assert!(docstring.unwrap().contains("Single line comment"));
    }

    #[test]
    fn test_extract_no_docstring() {
        let source = r#"
def foo():
    pass
"#;

        let docstring = extract_docstring(source, 2);
        // May or may not have docstring depending on blank lines
    }

    #[test]
    fn test_symbol_code_key_from_location() {
        let location = Location::new("test.rs", 10, 5);
        let key = SymbolCodeKey::from_location(&location);

        assert_eq!(key.file_path, "test.rs");
        assert_eq!(key.line, 10);
        assert_eq!(key.column, 5);
    }
}
