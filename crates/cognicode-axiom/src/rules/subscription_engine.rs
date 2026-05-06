//! Deterministic Rule Engine — Subscription-based AST visitor pattern
//!
//! Implements SonarQube's SubscriptionVisitor pattern:
//! - Rules subscribe to specific AST node types
//! - Engine only calls visit_node() for subscribed nodes
//! - NEVER scans comments, strings, or docstrings (parser excludes them)
//!
//! Usage:
//! ```rust
//! use crate::rules::subscription_engine::{SubscriptionRule, SubscriptionEngine};
//!
//! struct MyRule;
//! impl SubscriptionRule for MyRule {
//!     fn id(&self) -> &str { "SXXXX" }
//!     fn name(&self) -> &str { "My Rule" }
//!     fn subscribed_nodes(&self) -> Vec<&'static str> {
//!         vec!["let_declaration"]
//!     }
//!     fn visit_node(&self, node: tree_sitter::Node, ctx: &RuleContext) -> Vec<Issue> {
//!         // Only called for real let_declarations, never comments
//!         vec![]
//!     }
//! }
//! ```

use crate::{Issue, RuleContext};
use tree_sitter::Node;

/// A rule that uses the SubscriptionVisitor pattern — subscribes to AST node types.
/// This is the SonarQube-approved approach. Rules that use this NEVER get false positives
/// from comments, docstrings, or string literals because the parser excludes those.
pub trait SubscriptionRule: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn severity(&self) -> crate::Severity;
    fn category(&self) -> crate::Category;
    
    /// The tree-sitter node type names this rule wants to visit.
    /// Example: ["let_declaration", "call_expression", "string_literal"]
    fn subscribed_nodes(&self) -> Vec<&'static str>;
    
    /// Called ONLY for nodes matching subscribed_nodes(). 
    /// The node is GUARANTEED to be real code, never a comment.
    fn visit_node(&self, node: Node<'_>, ctx: &RuleContext<'_>) -> Vec<Issue>;
}

/// Deterministic engine that walks the AST and calls rules only for subscribed nodes.
/// Equivalent to SonarQube's IssuableSubscriptionVisitor pattern.
pub struct SubscriptionEngine;

impl SubscriptionEngine {
    /// Run a subscription rule against the AST.
    pub fn check(rule: &dyn SubscriptionRule, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut issues = Vec::new();
        let subscribed = rule.subscribed_nodes();
        let root = ctx.tree.root_node();
        
        let mut callback = |node: Node<'_>| {
            issues.extend(rule.visit_node(node, ctx));
        };
        
        Self::walk(root, &subscribed, &mut callback);
        
        issues
    }
    
    fn walk<'a>(node: Node<'a>, subscribed: &[&str], callback: &mut dyn FnMut(Node<'a>)) {
        if subscribed.contains(&node.kind()) {
            callback(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::walk(child, subscribed, callback);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: Skip comments when scanning source lines (for legacy regex rules)
// ─────────────────────────────────────────────────────────────────────────────

/// Returns true if the line at the given index is a comment.
/// Use this in legacy line-scanning rules to avoid false positives.
pub fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with('#')
}

/// Iterator over source lines that skips comments.
/// Use this in legacy rules instead of ctx.source.lines().
pub fn non_comment_lines(source: &str) -> impl Iterator<Item = (usize, &str)> + '_ {
    source.lines()
        .enumerate()
        .filter(|(_, line)| !is_comment_line(line))
        .map(|(i, line)| (i, line.trim()))
        .filter(|(_, line)| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_comment_line_detects_comments() {
        assert!(is_comment_line("// this is a comment"));
        assert!(is_comment_line("/// doc comment"));
        assert!(is_comment_line("//! module doc"));
        assert!(is_comment_line("/* block comment"));
        assert!(is_comment_line("    // indented comment"));
        assert!(is_comment_line("# python comment"));
        assert!(is_comment_line(""));  // empty
    }
    
    #[test]
    fn test_is_comment_line_allows_code() {
        assert!(!is_comment_line("let x = 42;"));
        assert!(!is_comment_line("fn main() {"));
        assert!(!is_comment_line("    password = \"secret\";"));
        assert!(!is_comment_line("use std::io;"));
    }
    
    #[test]
    fn test_non_comment_lines_filters() {
        let source = "// comment\nlet x = 42;\n\n/// doc\nlet y = 43;";
        let lines: Vec<_> = non_comment_lines(source).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].1, "let x = 42;");
        assert_eq!(lines[1].1, "let y = 43;");
    }
}
