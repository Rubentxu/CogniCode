//! Generic AST extractor — walks a tree-sitter AST using a `LanguageConfig`
//! and produces `GraphNode`s + `ExtractionEdge`s (ADR-018).
//!
//! This replaces the match-arm-based extraction in `AnalysisService` with a
//! single generic walker that consumes `LanguageConfig` data.

use std::path::Path;

use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
use crate::domain::aggregates::{GraphNode, NodeId};
use crate::domain::value_objects::{DependencyType, NodeKind, Provenance, SymbolKind};
use crate::infrastructure::parser::LanguageConfig;

use tree_sitter::{Node, Parser};

/// Extract structural information from a source file using a `LanguageConfig`.
///
/// Walks the tree-sitter AST, collecting:
/// - Function, class, and variable symbols as `GraphNode`s
/// - `Calls` edges (same-file, `Provenance::Extracted`)
/// - `Imports` edges (`Provenance::Extracted`)
/// - `Contains` edges (file → symbol)
///
/// Cross-file call resolution is deferred to the Resolve stage — unresolved
/// callees are emitted as `TargetRef::Unresolved(name)`.
pub fn extract_file(
    config: &LanguageConfig,
    path: &Path,
    source: &str,
    content_hash: &str,
) -> ExtractionResult {
    let mut parser = Parser::new();
    let ts_lang = (config.ts_language)();
    parser.set_language(&ts_lang).expect("failed to set tree-sitter language");

    let tree = match parser.parse(source.as_bytes(), None) {
        Some(t) => t,
        None => {
            return ExtractionResult::failed(
                path.to_path_buf(),
                content_hash.to_string(),
                "tree-sitter returned None (parse failure)".to_string(),
            );
        }
    };

    let root = tree.root_node();
    let source_bytes = source.as_bytes();
    let source_path_str = path.to_string_lossy().into_owned();

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<ExtractionEdge> = Vec::new();
    let mut symbol_ids: Vec<(String, String)> = Vec::new(); // (id, name)

    // ── File-level node ────────────────────────────────────────────────
    let file_node_id = NodeId::new(&source_path_str);
    let file_node = GraphNode::builder(file_node_id.clone(), NodeKind::Symbol(SymbolKind::File))
        .label(path.file_name().unwrap_or_default().to_string_lossy().into_owned())
        .source_path(path.to_path_buf())
        .build();
    nodes.push(file_node);

    // ── Iterative DFS over the AST ─────────────────────────────────────
    let mut stack: Vec<Node> = vec![root];
    let mut seen = std::collections::HashSet::new();

    while let Some(node) = stack.pop() {
        let node_type = node.kind();

        // ── Function nodes ─────────────────────────────────────────────
        if config.function_types.contains(&node_type) {
            if let Some(name) = extract_name(&node, source_bytes) {
                let (symbol_node, symbol_id) = make_symbol_node(
                    &name,
                    SymbolKind::Function,
                    &source_path_str,
                    (node.start_position().row + 1) as u32,
                    (node.start_position().column + 1) as u32,
                );
                // Contains edge: file → symbol
                edges.push(contains_edge(&file_node_id, &symbol_id, &source_path_str));
                nodes.push(symbol_node.clone());
                symbol_ids.push((symbol_id.clone(), name.clone()));

                // Find calls within this function body
                extract_calls_from_node(
                    &node,
                    source_bytes,
                    &symbol_id,
                    &config.call_types,
                    config.call_has_function_field,
                    &source_path_str,
                    &mut edges,
                );
            }
        }

        // ── Class/type nodes ───────────────────────────────────────────
        if config.class_types.contains(&node_type) {
            let kind = classify_class_type(node_type);
            if let Some(name) = extract_name(&node, source_bytes) {
                let (symbol_node, symbol_id) = make_symbol_node(
                    &name,
                    kind,
                    &source_path_str,
                    (node.start_position().row + 1) as u32,
                    (node.start_position().column + 1) as u32,
                );
                edges.push(contains_edge(&file_node_id, &symbol_id, &source_path_str));
                nodes.push(symbol_node.clone());
                symbol_ids.push((symbol_id.clone(), name.clone()));
            }
        }

        // ── Import nodes ───────────────────────────────────────────────
        if config.import_types.contains(&node_type) {
            if let Some(module_name) = extract_import_target(&node, source_bytes) {
                edges.push(ExtractionEdge {
                    source: file_node_id.as_str().to_string(),
                    target_ref: TargetRef::Unresolved(module_name),
                    kind: format!("dependency.{}", DependencyType::Imports),
                    provenance: Provenance::Extracted,
                    confidence: 1.0,
                    line: Some(node.start_position().row as u32 + 1),
                });
            }
        }

        // Push children (dedup by byte range to avoid revisiting)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let key = (child.start_byte(), child.end_byte());
            if seen.insert(key) {
                stack.push(child);
            }
        }
    }

    ExtractionResult::ok(path.to_path_buf(), content_hash.to_string(), nodes, edges)
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract the `name` field text from a node, falling back to the first
/// named child if no `name` field exists.
fn extract_name<'a>(node: &Node, source: &'a [u8]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(node_text(&name_node, source));
    }
    // Fallback: first named child that is an identifier
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.is_named() && child.kind() == "identifier" {
                return Some(node_text(&child, source));
            }
        }
    }
    None
}

/// Extract the imported module/package name from an import node.
fn extract_import_target(node: &Node, source: &[u8]) -> Option<String> {
    // Try the `source` field (JS/TS), `module_name` field (Python),
    // or the `name` field (Rust use, Go import, Java import).
    for field in &["source", "module_name", "name", "module"] {
        if let Some(child) = node.child_by_field_name(field) {
            let text = node_text(&child, source);
            // Strip quotes from string literals (JS/TS)
            let cleaned = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    // Fallback: first string literal child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string" || child.kind() == "string_content" {
            let text = node_text(&child, source);
            let cleaned = text.trim_matches(|c| c == '"' || c == '\'' || c == '`').to_string();
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }
    None
}

/// Walk the subtree of a function node looking for call expressions.
fn extract_calls_from_node(
    func_node: &Node,
    source: &[u8],
    caller_id: &str,
    call_types: &[&str],
    call_has_function_field: bool,
    source_path: &str,
    edges: &mut Vec<ExtractionEdge>,
) {
    let mut stack = vec![*func_node];
    let func_start = func_node.start_byte();
    let func_end = func_node.end_byte();

    while let Some(node) = stack.pop() {
        let nt = node.kind();
        if call_types.contains(&nt) {
            let callee_name = if call_has_function_field {
                node.child_by_field_name("function")
                    .map(|n| node_text(&n, source))
            } else {
                // First named child is the callee
                let mut cursor = node.walk();
                node.children(&mut cursor).next().map(|n| node_text(&n, source))
            };

            if let Some(callee) = callee_name {
                let callee_clean = clean_callee_name(&callee);
                if !callee_clean.is_empty() {
                    edges.push(ExtractionEdge {
                        source: caller_id.to_string(),
                        target_ref: TargetRef::Unresolved(callee_clean),
                        kind: format!("dependency.{}", DependencyType::Calls),
                        provenance: Provenance::Extracted,
                        confidence: 1.0,
                        line: Some(node.start_position().row as u32 + 1),
                    });
                }
            }
        }

        // Only descend into children that are within the function body
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.start_byte() >= func_start && child.end_byte() <= func_end {
                stack.push(child);
            }
        }
    }
}

/// Map a tree-sitter class-like node type to a `SymbolKind`.
fn classify_class_type(node_type: &str) -> SymbolKind {
    match node_type {
        "struct_item" | "struct_declaration" => SymbolKind::Struct,
        "enum_item" | "enum_declaration" => SymbolKind::Enum,
        "trait_item" | "interface_declaration" => SymbolKind::Trait,
        "impl_item" => SymbolKind::Class, // Rust impl maps to Class
        "union_item" => SymbolKind::Class,
        "class_declaration" | "class_definition" => SymbolKind::Class,
        "type_declaration" => SymbolKind::Type,
        _ => SymbolKind::Class,
    }
}

/// Build a `GraphNode` for a symbol + its ID string.
fn make_symbol_node(
    name: &str,
    kind: SymbolKind,
    file_path: &str,
    line: u32,
    column: u32,
) -> (GraphNode, String) {
    let id = format!("{}:{}:{}", file_path, name, line);
    let node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(kind))
        .label(name.to_string())
        .source_path(std::path::PathBuf::from(file_path))
        .property("line".to_string(), line.to_string())
        .property("column".to_string(), column.to_string())
        .build();
    (node, id)
}

/// Create a `Contains` edge from parent to child.
fn contains_edge(parent_id: &NodeId, child_id: &str, source_path: &str) -> ExtractionEdge {
    ExtractionEdge {
        source: parent_id.as_str().to_string(),
        target_ref: TargetRef::Resolved(child_id.to_string()),
        kind: format!("dependency.{}", DependencyType::Contains),
        provenance: Provenance::Extracted,
        confidence: 1.0,
        line: None,
    }
}

/// Strip method-call syntax from a callee name.
/// `obj.method` → `method`, `Foo::bar` → `bar`, `self.save` → `save`.
fn clean_callee_name(raw: &str) -> String {
    // Take the last segment after `.` or `::`
    let cleaned = raw
        .split("::")
        .last()
        .unwrap_or(raw)
        .split('.')
        .last()
        .unwrap_or(raw);
    cleaned.trim().to_string()
}

/// Get the UTF-8 text of a node from the source bytes.
fn node_text(node: &Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    String::from_utf8_lossy(&source[start..end]).into_owned()
}
