//! Generic AST extractor — walks a tree-sitter AST using a `LanguageConfig`
//! and produces `GraphNode`s + `ExtractionEdge`s (ADR-018).
//!
//! This replaces the match-arm-based extraction in `AnalysisService` with a
//! single generic walker that consumes `LanguageConfig` data.

use std::path::Path;

use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
// Un-gated: the identity grammar is shared by the legacy path and the fact
// path (E38.1 CP-1), so `SymbolFqn` compiles unconditionally.
use crate::domain::aggregates::{GraphNode, NodeId};
use crate::domain::evidence_kernel::SymbolFqn;
use crate::domain::value_objects::{DependencyType, NodeKind, Provenance, SymbolKind};
use crate::infrastructure::parser::LanguageConfig;
use cognicode_graph_algos::algorithms::Statement;

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
    if let Err(err) = parser.set_language(&ts_lang) {
        return ExtractionResult::failed(
            path.to_path_buf(),
            content_hash.to_string(),
            format!(
                "tree-sitter language init failed for {:?}: {err}",
                config.language
            ),
        );
    }

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
    let mut symbol_ids: Vec<(String, String)> = Vec::new();
    let mut statements_map: std::collections::BTreeMap<String, Vec<Statement>> =
        std::collections::BTreeMap::new();

    // ── File-level node ────────────────────────────────────────────────
    let file_node_id = NodeId::new(&source_path_str);
    let file_node = GraphNode::builder(file_node_id.clone(), NodeKind::Symbol(SymbolKind::File))
        .label(
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )
        .source_path(path.to_path_buf())
        .build();
    nodes.push(file_node);

    // ── Iterative DFS over the AST ─────────────────────────────────────
    let mut stack: Vec<Node> = vec![root];
    let mut seen = std::collections::HashSet::new();

    while let Some(node) = stack.pop() {
        let node_type = node.kind();

        // ── Function nodes ─────────────────────────────────────────────
        if config.function_types.contains(&node_type)
            && let Some(name) = extract_name(&node, source_bytes)
        {
            let (symbol_node, symbol_id) = make_symbol_node(
                &name,
                SymbolKind::Function,
                &source_path_str,
                // 1-based fact-side line (E38.1 CP-1): `start.row + 1` feeds
                // `SymbolFqn::from_fact_side` inside `make_symbol_node`.
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
                config.call_types,
                config.call_has_function_field,
                &source_path_str,
                &mut edges,
            );

            // Extract type references if walker is configured
            extract_type_refs(
                config,
                &node,
                source_bytes,
                &symbol_id,
                &source_path_str,
                &mut edges,
            );

            // Extract statements from the function body (M5.1b).
            // Wrap in catch_unwind per ADR-023: malformed body → empty Vec,
            // rest of file proceeds.
            let stmts: Vec<Statement> =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    extract_statements_from_node(&node, source_bytes)
                }))
                .unwrap_or_default();

            if !stmts.is_empty() {
                statements_map.insert(symbol_id.clone(), stmts);
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

                // Extract type references for class/struct definitions
                extract_type_refs(
                    config,
                    &node,
                    source_bytes,
                    &symbol_id,
                    &source_path_str,
                    &mut edges,
                );
            }
        }

        // ── Import nodes ───────────────────────────────────────────────
        if config.import_types.contains(&node_type)
            && let Some(module_name) = extract_import_target(&node, source_bytes)
        {
            edges.push(ExtractionEdge {
                source: file_node_id.as_str().to_string(),
                target_ref: TargetRef::Unresolved(module_name),
                kind: format!("dependency.{}", DependencyType::Imports),
                provenance: Provenance::Extracted,
                confidence: 1.0,
                line: Some(node.start_position().row as u32 + 1),
            });
        }

        // ── Variable nodes (const / static / let) ──────────────────
        // UAT 2026-08-10 DEFECT-5: variable_types was declared on the
        // LanguageConfig but never visited in the DFS, so `const_item`,
        // `static_item`, and `let_declaration` silently dropped out of
        // the graph. They now flow through the same path as
        // function_types and class_types.
        if config.variable_types.contains(&node_type)
            && let Some(name) = extract_name(&node, source_bytes)
        {
            let kind = match node_type {
                "const_item" | "static_item" => SymbolKind::Constant,
                _ => SymbolKind::Variable,
            };
            let (symbol_node, symbol_id) = make_symbol_node(
                &name,
                kind,
                &source_path_str,
                (node.start_position().row + 1) as u32,
                (node.start_position().column + 1) as u32,
            );
            edges.push(contains_edge(&file_node_id, &symbol_id, &source_path_str));
            nodes.push(symbol_node);
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

    let result = ExtractionResult::ok_with_statements(
        path.to_path_buf(),
        content_hash.to_string(),
        nodes,
        edges,
        statements_map,
    );

    // ── Semantic handler post-parse pass (ADR-024 / ADR-036) ────────────
    // If a semantic handler is configured (e.g., Ansible, Terraform),
    // run it to enrich the result with domain-specific nodes and edges.
    if let Some(handler) = config.semantic_handler {
        let source_path_str = path.to_string_lossy();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handler(&source_path_str, content_hash, &result)
        }))
        .unwrap_or_else(|_| {
            // semantic_handler panicked — return the original result
            // (error isolation per ADR-023)
            result.clone()
        })
    } else {
        result
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Extract the `name` field text from a node, falling back to the first
/// named child if no `name` field exists.
fn extract_name(node: &Node, source: &[u8]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        return Some(node_text(&name_node, source));
    }
    // Fallback: first named child that is an identifier
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i)
            && child.is_named()
            && child.kind() == "identifier"
        {
            return Some(node_text(&child, source));
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
            let cleaned = text
                .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                .to_string();
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
            let cleaned = text
                .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                .to_string();
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
    _source_path: &str,
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
                node.children(&mut cursor)
                    .next()
                    .map(|n| node_text(&n, source))
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
///
/// The ID follows the canonical identity grammar `"{file}:{name}:{line}"`
/// (E38.1 CP-1, centralized in [`SymbolFqn`]); `line` arrives 1-BASED
/// (`start.row + 1`, the fact-side convention), so this site constructs via
/// [`SymbolFqn::from_fact_side`] and renders it verbatim — byte-identical to
/// the historical `format!`.
fn make_symbol_node(
    name: &str,
    kind: SymbolKind,
    file_path: &str,
    line: u32,
    column: u32,
) -> (GraphNode, String) {
    let id = SymbolFqn::from_fact_side(file_path, name, line).assemble();
    let node = GraphNode::builder(NodeId::new(&id), NodeKind::Symbol(kind))
        .label(name.to_string())
        .source_path(std::path::PathBuf::from(file_path))
        .property("line".to_string(), line.to_string())
        .property("column".to_string(), column.to_string())
        .build();
    (node, id)
}

/// Create a `Contains` edge from parent to child.
fn contains_edge(parent_id: &NodeId, child_id: &str, _source_path: &str) -> ExtractionEdge {
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
        .next_back()
        .unwrap_or(raw);
    cleaned.trim().to_string()
}

/// Get the UTF-8 text of a node from the source bytes.
fn node_text(node: &Node, source: &[u8]) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    String::from_utf8_lossy(&source[start..end]).into_owned()
}

/// Call the type-ref walker (if configured) and emit `References` edges.
fn extract_type_refs(
    config: &LanguageConfig,
    node: &Node,
    source: &[u8],
    symbol_id: &str,
    _source_path: &str,
    edges: &mut Vec<ExtractionEdge>,
) {
    let walker = match config.type_ref_walker {
        Some(w) => w,
        None => return,
    };

    let type_refs = walker(node, source);
    for tr in type_refs {
        edges.push(ExtractionEdge {
            source: symbol_id.to_string(),
            target_ref: TargetRef::Unresolved(tr.target_name),
            kind: format!("dependency.{}", DependencyType::References),
            provenance: Provenance::Extracted,
            confidence: 1.0,
            line: Some(tr.line),
        });
    }
}

// ============================================================================
// Statement extraction (M5.1b)
// ============================================================================

/// Extract statement-level def/use facts from a function body (M5.1b).
///
/// Returns a `Vec<Statement>` indexed 0..N-1 in DFS/source order.
/// Handles Rust tree-sitter node kinds:
///   - `let_declaration`     → def: pattern identifiers
///   - `assignment_expression` → def: lhs identifier; uses: rhs identifiers
///   - `expression_statement`  → uses: all identifiers in the expression
///   - `return_expression`     → uses: all identifiers in the return value
///   - `if_expression`        → recurse; no own Statement emitted
///   - `match_expression` / `match_arm` → recurse; no own Statement emitted
///   - `for_expression` / `while_expression` → recurse into body; uses: iterable/condition
///   - `loop_expression`      → recurse into body
///   - `block`                → recurse into children
///
/// `defs` and `uses` are sorted and deduplicated per Statement.
/// The function node itself is NOT walked (its outer fields like `parameters`
/// and `return_type` are not statements). Only the body subtree is traversed.
fn extract_statements_from_node(function_node: &Node, source: &[u8]) -> Vec<Statement> {
    let mut statements: Vec<Statement> = Vec::new();
    let mut next_id: usize = 0;

    // Find the block child that contains the function body.
    let body_node = find_body_node(function_node);
    if let Some(body) = body_node {
        walk_block(&body, source, &mut statements, &mut next_id);
    }

    statements
}

/// Find the body node of a function (the `block` child, or for expression bodies
/// the body itself).
fn find_body_node<'a>(func_node: &'a Node) -> Option<Node<'a>> {
    let mut cursor = func_node.walk();
    for child in func_node.children(&mut cursor) {
        let kind = child.kind();
        // block = fn body { ... }
        // expression_body = fn() => expr
        if kind == "block" || kind == "expression_body" {
            return Some(child);
        }
    }
    None
}

/// Recursively walk a block node collecting statements.
fn walk_block(node: &Node, source: &[u8], statements: &mut Vec<Statement>, next_id: &mut usize) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            // Emit a Statement
            "let_declaration" => {
                let defs = collect_pattern_identifiers(&child, source);
                let uses = child
                    .child_by_field_name("value")
                    .map(|v| collect_identifiers(&v, source))
                    .unwrap_or_default();
                if !defs.is_empty() || !uses.is_empty() {
                    statements.push(Statement {
                        id: *next_id,
                        kind: "let_declaration".to_string(),
                        defs: sorted_unique(&defs),
                        uses: sorted_unique(&uses),
                    });
                    *next_id += 1;
                }
            }
            "assignment_expression" => {
                let defs = extract_assignment_defs(&child, source);
                let uses = collect_identifiers(&child, source);
                // Remove defs from uses
                let uses: Vec<String> = uses.into_iter().filter(|u| !defs.contains(u)).collect();
                statements.push(Statement {
                    id: *next_id,
                    kind: "assignment_expression".to_string(),
                    defs: sorted_unique(&defs),
                    uses: sorted_unique(&uses),
                });
                *next_id += 1;
            }
            "expression_statement" => {
                let uses = collect_identifiers(&child, source);
                if !uses.is_empty() {
                    statements.push(Statement {
                        id: *next_id,
                        kind: "expression_statement".to_string(),
                        defs: Vec::new(),
                        uses: sorted_unique(&uses),
                    });
                    *next_id += 1;
                }
            }
            "return_expression" => {
                let uses = collect_identifiers(&child, source);
                statements.push(Statement {
                    id: *next_id,
                    kind: "return_expression".to_string(),
                    defs: Vec::new(),
                    uses: sorted_unique(&uses),
                });
                *next_id += 1;
            }
            // Recurse but do NOT emit own Statement
            "if_expression" => {
                // Recurse into condition, consequence, and alternative
                let mut inner = child.walk();
                for c in child.children(&mut inner) {
                    let ck = c.kind();
                    if ck == "block" || ck == "expression_body" {
                        walk_block(&c, source, statements, next_id);
                    } else {
                        // condition — also recurse to pick up any statements inside
                        walk_block(&c, source, statements, next_id);
                    }
                }
            }
            "match_expression" => {
                // Recurse into match arms
                let mut inner = child.walk();
                for c in child.children(&mut inner) {
                    let ck = c.kind();
                    if ck == "match_arm" {
                        walk_match_arm(&c, source, statements, next_id);
                    } else {
                        walk_block(&c, source, statements, next_id);
                    }
                }
            }
            "for_expression" => {
                // uses: iterable expression; recurse into body
                let uses: Vec<String> = child
                    .child_by_field_name("value")
                    .map(|v| collect_identifiers(&v, source))
                    .unwrap_or_default();
                if !uses.is_empty() {
                    statements.push(Statement {
                        id: *next_id,
                        kind: "for_expression".to_string(),
                        defs: Vec::new(),
                        uses: sorted_unique(&uses),
                    });
                    *next_id += 1;
                }
                // Recurse into body
                if let Some(body) = child.child_by_field_name("body") {
                    walk_block(&body, source, statements, next_id);
                }
            }
            "while_expression" => {
                // uses: condition; recurse into body
                let uses: Vec<String> = child
                    .child_by_field_name("condition")
                    .map(|c| collect_identifiers(&c, source))
                    .unwrap_or_default();
                if !uses.is_empty() {
                    statements.push(Statement {
                        id: *next_id,
                        kind: "while_expression".to_string(),
                        defs: Vec::new(),
                        uses: sorted_unique(&uses),
                    });
                    *next_id += 1;
                }
                if let Some(body) = child.child_by_field_name("body") {
                    walk_block(&body, source, statements, next_id);
                }
            }
            "loop_expression" => {
                // Recurse into body (no header uses)
                if let Some(body) = child.child_by_field_name("body") {
                    walk_block(&body, source, statements, next_id);
                }
            }
            "block" => {
                walk_block(&child, source, statements, next_id);
            }
            _ => {
                // Skip: parameters, return_type, type_annotation, etc.
            }
        }
    }
}

/// Walk a match arm and recurse into its body.
fn walk_match_arm(
    arm_node: &Node,
    source: &[u8],
    statements: &mut Vec<Statement>,
    next_id: &mut usize,
) {
    let mut cursor = arm_node.walk();
    for child in arm_node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "block" || kind == "expression_body" {
            walk_block(&child, source, statements, next_id);
        } else if kind == "identifier" || kind == "choice_expression" {
            // Arm pattern — not a statement
        } else {
            // Recurse into any nested structures
            walk_block(&child, source, statements, next_id);
        }
    }
}

/// Collect all identifier text from a node and its descendants (DFS).
fn collect_identifiers(node: &Node, source: &[u8]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut stack = vec![*node];
    while let Some(n) = stack.pop() {
        let kind = n.kind();
        if kind == "identifier" {
            let text = node_text(&n, source).trim().to_string();
            if !text.is_empty() && text != "_" {
                ids.push(text);
            }
        } else if kind == "field_identifier" {
            // field access: `obj.field` — collect the field name
            let text = n
                .child_by_field_name("field")
                .map(|f| node_text(&f, source).trim().to_string())
                .filter(|t| !t.is_empty() && *t != "_")
                .unwrap_or_else(|| node_text(&n, source).trim().to_string());
            if !text.is_empty() && text != "_" {
                ids.push(text);
            }
        } else if kind == "scoped_identifier" {
            // `module::Item` — take the last segment
            let text = n
                .named_children(&mut n.walk())
                .last()
                .map(|c| node_text(&c, source).trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| node_text(&n, source).trim().to_string());
            if !text.is_empty() && text != "_" {
                ids.push(text);
            }
        } else {
            let mut cursor = n.walk();
            for child in n.children(&mut cursor) {
                stack.push(child);
            }
        }
    }
    ids
}

/// Collect identifiers from the pattern side of a let declaration
/// (handles destructuring: `let (a, b) = ...` or `let [x, y] = ...`).
fn collect_pattern_identifiers(let_node: &Node, source: &[u8]) -> Vec<String> {
    let pattern = match let_node.child_by_field_name("pattern") {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut ids = Vec::new();
    collect_pattern_ids_recursive(&pattern, source, &mut ids);
    ids
}

fn collect_pattern_ids_recursive(node: &Node, source: &[u8], out: &mut Vec<String>) {
    let kind = node.kind();
    match kind {
        "identifier" => {
            let text = node_text(node, source).trim().to_string();
            if !text.is_empty() && text != "_" {
                out.push(text);
            }
        }
        "tuple_pattern" | "slice_pattern" | "rest_pattern" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_pattern_ids_recursive(&child, source, out);
            }
        }
        _ => {
            // ignore other pattern kinds
        }
    }
}

/// Extract the lhs identifiers from an assignment expression.
/// Handles `identifier = ...` and `field = ...`.
fn extract_assignment_defs(node: &Node, source: &[u8]) -> Vec<String> {
    let mut ids = Vec::new();
    let mut stack = vec![*node];
    let mut found_eq = false;
    while let Some(n) = stack.pop() {
        let kind = n.kind();
        if kind == "=" {
            found_eq = true;
            continue;
        }
        if !found_eq {
            // We're still in the lhs
            if kind == "identifier" {
                let text = node_text(&n, source).trim().to_string();
                if !text.is_empty() && text != "_" {
                    ids.push(text);
                }
            } else if kind == "field_identifier" {
                let text = n
                    .child_by_field_name("field")
                    .map(|f| node_text(&f, source).trim().to_string())
                    .filter(|t| !t.is_empty() && *t != "_")
                    .unwrap_or_else(|| node_text(&n, source).trim().to_string());
                if !text.is_empty() && text != "_" {
                    ids.push(text);
                }
            } else {
                let mut cursor = n.walk();
                for child in n.children(&mut cursor) {
                    stack.push(child);
                }
            }
        } else {
            // RHS — stop descending
        }
    }
    ids
}

/// Sort a Vec<String> and remove duplicates.
fn sorted_unique(v: &[String]) -> Vec<String> {
    let mut v = v.to_vec();
    v.sort();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression test for UAT 2026-08-10 DEFECT-5.
    /// A real production Rust file mixes top-level items with methods inside
    /// `impl` blocks, items inside `mod` modules, and module-level
    /// `const`/`static` declarations. We feed a small but structurally
    /// representative source to the extractor and assert that every kind
    /// declared in `LanguageConfig` is picked up — if `variable_types`
    /// ever regresses the count drops below 11 and the test fails.
    /// same gap the UAT flagged.
    #[test]
    fn test_extractor_covers_all_declared_kinds_rust() {
        let source = r#"
//! Doc comment
pub use std::collections::HashMap;

pub struct Foo {
    pub bar: u32,
}

pub enum Color {
    Red,
    Green,
    Blue,
}

pub trait Greet {
    fn hello(&self) -> String;
    fn goodbye(&self) -> String;
}

pub fn standalone() {}

impl Foo {
    pub fn new() -> Self { Foo { bar: 0 } }
    pub fn get_bar(&self) -> u32 { self.bar }
    fn private_helper(&self) -> u32 { self.bar + 1 }
}

impl Greet for Foo {
    fn hello(&self) -> String { format!("hi {}", self.bar) }
    fn goodbye(&self) -> String { format!("bye {}", self.bar) }
}

const MY_CONST: u32 = 42;
static MY_STATIC: u32 = 0;
"#;
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let result = extract_file(
            &cfg,
            std::path::Path::new("/tmp/synthetic.rs"),
            source,
            "test_content_hash",
        );
        assert!(result.is_ok(), "extract_file failed: {:?}", result.error);
        let names: Vec<&str> = result
            .nodes
            .iter()
            .map(|n| n.label.as_str())
            .filter(|n| !n.is_empty() && *n != "synthetic.rs")
            .collect();
        eprintln!("EXTRACTED {} symbol-like nodes: {:?}", names.len(), names);
        // Expected at minimum:
        //   - struct Foo
        //   - enum Color
        //   - trait Greet
        //   - standalone (fn)
        //   - new, get_bar, private_helper (fn inside impl Foo)
        //   - hello, goodbye (fn inside impl Greet for Foo)
        //   - MY_CONST, MY_STATIC (Constant, was missing before DEFECT-5 fix)
        // If variable_types extraction regresses the count drops below 11.
        assert!(
            names.len() >= 11,
            "expected at least 11 symbols from this synthetic Rust file, got {}: {:?}",
            names.len(),
            names
        );
    }

    // ---- Statement extraction (M5.1b) ----

    /// SCN-STMT-01: empty function body → Vec::new()
    #[test]
    fn statement_extraction_empty_body() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let result = extract_file(
            &cfg,
            std::path::Path::new("empty.rs"),
            "fn empty() {}",
            "hash",
        );
        assert!(result.is_ok());
        // No statements extracted (empty body)
        assert!(result.statements_by_function.values().all(|v| v.is_empty()));
    }

    /// SCN-STMT-02: `let x = 1;` → 1 Statement with defs=[x], uses=[]
    #[test]
    fn statement_extraction_let_single() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let result = extract_file(
            &cfg,
            std::path::Path::new("test.rs"),
            "fn f() { let x = 1; }",
            "hash",
        );
        assert!(result.is_ok());
        let stmts: Vec<_> = result.statements_by_function.values().flatten().collect();
        assert_eq!(stmts.len(), 1, "expected 1 statement, got {:?}", stmts);
        assert_eq!(stmts[0].defs, vec!["x"]);
        assert!(stmts[0].uses.is_empty());
        assert_eq!(stmts[0].id, 0);
    }

    /// SCN-STMT-03: two-statement chain → correct defs/uses per statement
    #[test]
    fn statement_extraction_two_statement_chain() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let result = extract_file(
            &cfg,
            std::path::Path::new("test.rs"),
            "fn f() { let x = 1; let y = x; }",
            "hash",
        );
        assert!(result.is_ok());
        let stmts: Vec<_> = result.statements_by_function.values().flatten().collect();
        assert_eq!(stmts.len(), 2, "expected 2 statements, got {:?}", stmts);
        // `let x = 1;`
        assert_eq!(stmts[0].id, 0);
        assert_eq!(stmts[0].defs, vec!["x"]);
        // `let y = x;`
        assert_eq!(stmts[1].id, 1);
        assert_eq!(stmts[1].defs, vec!["y"]);
        assert_eq!(stmts[1].uses, vec!["x"]);
    }

    /// SCN-STMT-04: `return x + y;` → defs=[], uses=[x, y] (sorted)
    #[test]
    fn statement_extraction_return() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        // Use identifiers (not literals) so the walker can extract them.
        // Note: tree-sitter wraps `return x + y;` in an expression_statement node.
        let result = extract_file(
            &cfg,
            std::path::Path::new("test.rs"),
            "fn f() -> i32 { let a = 1; let b = 2; return a + b; }",
            "hash",
        );
        assert!(result.is_ok());
        let stmts: Vec<_> = result.statements_by_function.values().flatten().collect();
        // Expect 3 statements: two lets + one expression_statement (wrapping return)
        assert_eq!(stmts.len(), 3, "expected 3 statements, got {:?}", stmts);
        // Last statement is the return expression (tree-sitter wraps it in expression_statement)
        let ret = &stmts[2];
        assert_eq!(ret.kind, "expression_statement");
        assert!(ret.defs.is_empty());
        // a and b should appear in the return's uses
        assert!(
            ret.uses.contains(&"a".to_string()),
            "expected a in return uses, got {:?}",
            ret.uses
        );
        assert_eq!(ret.id, 2);
    }

    /// SCN-STMT-05: `if` branch → statements inside the if; the `if` itself emits nothing
    #[test]
    fn statement_extraction_if_branch() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let result = extract_file(
            &cfg,
            std::path::Path::new("test.rs"),
            "fn f() { if true { let x = 1; } }",
            "hash",
        );
        assert!(result.is_ok());
        let stmts: Vec<_> = result.statements_by_function.values().flatten().collect();
        // At least one statement from inside the if block
        assert!(
            !stmts.is_empty(),
            "expected at least one statement from inside if, got {:?}",
            stmts
        );
        // Statement ids are 0..N-1 (no gaps)
        for (i, s) in stmts.iter().enumerate() {
            assert_eq!(s.id, i, "statement id should be monotonic 0..N-1");
        }
    }

    /// SCN-STMT-06: Statement.id runs 0..N-1 in source order
    #[test]
    fn statement_extraction_id_monotonic() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let result = extract_file(
            &cfg,
            std::path::Path::new("test.rs"),
            "fn f() { let a = 1; let b = a; let c = b; let d = c; }",
            "hash",
        );
        assert!(result.is_ok());
        let stmts: Vec<_> = result.statements_by_function.values().flatten().collect();
        assert_eq!(stmts.len(), 4);
        for (i, s) in stmts.iter().enumerate() {
            assert_eq!(s.id, i, "statement id {} mismatch at position {}", s.id, i);
        }
    }

    /// SCN-STMT-07: Two extractions of same source → byte-identical Vec<Statement>
    #[test]
    fn statement_extraction_deterministic() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        let src = "fn f() { let x = 1; let y = x; }";
        let r1 = extract_file(&cfg, std::path::Path::new("test.rs"), src, "h1");
        let r2 = extract_file(&cfg, std::path::Path::new("test.rs"), src, "h2");
        let j1 = serde_json::to_string(&r1.statements_by_function).unwrap();
        let j2 = serde_json::to_string(&r2.statements_by_function).unwrap();
        assert_eq!(
            j1, j2,
            "statements must be deterministic across extractions"
        );
    }

    /// SCN-STMT-09: ADR-023 error isolation — a syntactically malformed function body
    /// does not panic; that function gets empty statements and the rest proceeds.
    /// We test the catch_unwind path by feeding a function with a curly-brace
    /// mismatch inside the body (which tree-sitter should handle gracefully).
    #[test]
    fn statement_extraction_error_isolation() {
        let cfg = crate::infrastructure::parser::language_config::RUST_CONFIG;
        // Two functions: second has broken body
        let result = extract_file(
            &cfg,
            std::path::Path::new("test.rs"),
            "fn ok() { let x = 1; }\nfn broken() { let y = x; { // unmatched brace",
            "hash",
        );
        // Even with the malformed second function, extraction must succeed
        assert!(
            result.is_ok(),
            "extract_file must not panic on malformed body"
        );
        // The first function's statements should be present
        let stmts: Vec<_> = result.statements_by_function.values().flatten().collect();
        // The 'ok' function should have at least its statements
        assert!(
            stmts.iter().any(|s| s.kind == "let_declaration"),
            "expected let_declaration from the ok function, got {:?}",
            stmts
        );
    }
}
