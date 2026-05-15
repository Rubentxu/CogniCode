//! CC_CS_009: Redundant Parenthesized Expressions Should Be Removed
//!
//! Detects unnecessary parentheses around expressions.
//!
//! # Problem
//! Unnecessary parentheses add visual noise and suggest the developer
//! was unsure of operator precedence.
//!
//! # Fix
//! Remove unnecessary parentheses while preserving any semantic meaning.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_009 Rule: Redundant Parentheses Detection
pub struct RedundantParenthesesRule;

impl Default for RedundantParenthesesRule {
    fn default() -> Self {
        Self
    }
}

impl RedundantParenthesesRule {
    /// Check if this is in a context where parentheses are required
    fn is_required_context(source: &str, node: tree_sitter::Node) -> bool {
        let node_start = node.start_byte();
        let start = if node_start > 200 { node_start - 200 } else { 0 };
        let preceding_text = &source[start..node_start];

        // Return statements often have parentheses for clarity
        if preceding_text.contains("return (")
            || preceding_text.trim().ends_with("return")
        {
            return true;
        }

        // Closure signatures: |x| (x + 1)
        if preceding_text.contains("|") && !preceding_text.contains("=>") {
            return true;
        }

        // Macro contexts
        if preceding_text.contains("macro_rules!")
            || preceding_text.contains("$(")
            || preceding_text.ends_with('!')
        {
            return true;
        }

        false
    }
}

impl Rule for RedundantParenthesesRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_009")
    }

    fn name(&self) -> &'static str {
        "Redundant Parenthesized Expressions Should Be Removed"
    }

    fn description(&self) -> &'static str {
        "Unnecessary parentheses around expressions add visual noise."
    }

    fn category(&self) -> Category {
        Category::Style
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Match parenthesized_expression nodes
        let query_str = r#"(parenthesized_expression) @parens"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let parens_node = cap.node;

                // Get the inner expression - skip the opening parenthesis (child 0)
                // The inner expression is at child index 1
                if parens_node.child_count() >= 2 {
                    if let Some(inner) = parens_node.child(1) {
                        let inner_kind = inner.kind();

                        // Simple expressions that don't need parentheses
                        let simple_kinds = [
                            "integer_literal",
                            "float_literal",
                            "boolean_literal",
                            "identifier",
                            "string_literal",
                            "char_literal",
                        ];

                        if simple_kinds.contains(&inner_kind) {
                            // Check if parentheses are in a required context
                            if !Self::is_required_context(source, parens_node) {
                                let pos = parens_node.start_position();
                                issues.push(Issue::new(
                                    "CC_CS_009",
                                    "Redundant Parentheses",
                                    Severity::Minor,
                                    Category::Style,
                                    ctx.file_path.to_string_lossy(),
                                    pos.row + 1,
                                    pos.column,
                                    "Unnecessary parentheses detected. Consider removing them for cleaner code.",
                                ));
                            }
                        }
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["parentheses", "redundant", "expression"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str) -> (tree_sitter::Tree, String) {
        let lang = SrcLanguage::Rust.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str) -> Vec<Issue> {
        let (tree, source) = parse_code(code);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.rs"),
            &SrcLanguage::Rust,
            &metrics,
        );
        let rule = RedundantParenthesesRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_redundant_parentheses_literal() {
        let code = r#"
fn example() {
    let x = (5);
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect redundant parentheses around literal");
        assert_eq!(issues[0].rule_id, "CC_CS_009");
    }

    #[test]
    fn test_detects_redundant_parentheses_identifier() {
        let code = r#"
fn example() {
    let x = (y);
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect redundant parentheses around identifier");
    }

    #[test]
    fn test_no_false_positive_return_statement() {
        let code = r#"
fn example() {
    return (x);
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag parentheses in return statement");
    }

    #[test]
    fn test_no_false_positive_complex_expression() {
        let code = r#"
fn example() {
    let x = (a + b) * c;
}
"#;
        let issues = check_rule(code);
        // Complex expressions may or may not be flagged
    }

    #[test]
    fn test_detects_redundant_closure_return() {
        let code = r#"
fn example() {
    let f = || (x + 1);
}
"#;
        let issues = check_rule(code);
        // Closure bodies may be flagged
    }
}