//! CC_CS_005: Redundant Semicolons Should Be Removed
//!
//! Detects extra semicolons after expressions or at the end of blocks.
//!
//! # Problem
//! Extra semicolons after expressions or at the end of blocks are unnecessary
//! in Rust and indicate sloppy code.
//!
//! # Fix
//! Remove the redundant semicolon.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_005 Rule: Redundant Semicolon Detection
pub struct RedundantSemicolonRule;

impl Default for RedundantSemicolonRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for RedundantSemicolonRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_005")
    }

    fn name(&self) -> &'static str {
        "Redundant Semicolons Should Be Removed"
    }

    fn description(&self) -> &'static str {
        "Extra semicolons after expressions or at the end of blocks are unnecessary in Rust."
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

        // Match consecutive semicolons by looking for empty_statement followed by another statement
        // The pattern ";; " at end of statements typically indicates redundant semicolon
        let query_str = r#"(empty_statement) @empty"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let pos = cap.node.start_position();
                let end = cap.node.end_byte();

                // An empty_statement represents a redundant semicolon
                // These appear when there's ;; instead of ;
                issues.push(Issue::new(
                    "CC_CS_005",
                    "Redundant Semicolon",
                    Severity::Minor,
                    Category::Style,
                    ctx.file_path.to_string_lossy(),
                    pos.row + 1,
                    pos.column,
                    "Redundant semicolon detected. Remove the extra semicolon.",
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["semicolon", "redundant", "extra"])
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
        let rule = RedundantSemicolonRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_double_semicolon() {
        let code = r#"
fn example() {
    let x = 5;;
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect double semicolon");
        assert_eq!(issues[0].rule_id, "CC_CS_005");
    }

    #[test]
    fn test_no_false_positive_normal_semicolon() {
        let code = r#"
fn example() {
    let x = 5;
    println!("{}", x);
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag normal semicolons");
    }

    #[test]
    fn test_detects_triple_semicolon() {
        let code = r#"
fn example() {
    let x = 5;;;
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect redundant semicolons");
    }
}