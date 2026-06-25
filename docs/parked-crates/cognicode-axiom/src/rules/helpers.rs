//! Helper functions for rule implementations
//!
//! Provides shared utility functions used across multiple rules:
//! - `calculate_cognitive_complexity`: SonarSource cognitive complexity algorithm
//! - `collect_string_literals`: Collects string literal locations for S1192
//! - `count_branches_impl`: Counts branch nodes for S1541

use std::collections::HashMap;
use tree_sitter::Node;

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Calculate cognitive complexity (SonarSource algorithm)
// ─────────────────────────────────────────────────────────────────────────────

pub fn calculate_cognitive_complexity(node: Node, source: &[u8]) -> i32 {
    let mut complexity = 0;
    compute_complexity_impl(node, source, 0, &mut complexity, false);
    complexity
}

fn compute_complexity_impl(
    node: Node,
    source: &[u8],
    depth: usize,
    complexity: &mut i32,
    _in_loop: bool,
) {
    let kind = node.kind();

    // Increment for control structures
    if matches!(kind,
        "if_expression" | "match_expression" | "match_arm" |
        "for_expression" | "while_expression" | "loop_expression"
    ) {
        *complexity += 1 + depth as i32;
    }

    // Increment for boolean operators in binary expressions
    if kind == "binary_expression"
        && let Ok(text) = node.utf8_text(source)
            && (text.contains("&&") || text.contains("||")) {
                *complexity += 1;
            }

    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            let is_loop = matches!(kind,
                "for_expression" | "while_expression" | "loop_expression"
            );
            compute_complexity_impl(child, source, depth + 1, complexity, is_loop);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Collect string literals
// ─────────────────────────────────────────────────────────────────────────────

pub fn collect_string_literals(
    node: Node,
    source: &[u8],
    locations: &mut HashMap<String, Vec<(usize, usize)>>,
) {
    if node.kind() == "string_literal"
        && let Ok(text) = node.utf8_text(source) {
            let point = node.start_position();
            let row = point.row;
            let col = point.column;
            locations
                .entry(text.to_string())
                .or_default()
                .push((row + 1, col + 1));
        }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            collect_string_literals(child, source, locations);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Count branches in a node tree
// ─────────────────────────────────────────────────────────────────────────────

pub fn count_branches_impl(node: Node, count: &mut usize) {
    let branch_kinds = ["if_expression", "match_arm", "while_expression", "for_expression", "loop_expression"];
    if branch_kinds.contains(&node.kind()) {
        *count += 1;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            count_branches_impl(child, count);
        }
    }
}
