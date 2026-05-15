//! CC_CS_004: Empty Statements Should Be Removed
//!
//! Detects standalone semicolons that indicate empty statements.
//!
//! # Problem
//! Empty statements add no value and indicate incomplete work or
//! copy-paste errors. In Rust, empty statements are almost always bugs.
//!
//! # Fix
//! Remove the empty statement or replace with meaningful code.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_004 Rule: Empty Statement Detection
pub struct EmptyStatementRule;

impl Default for EmptyStatementRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for EmptyStatementRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_004")
    }

    fn name(&self) -> &'static str {
        "Empty Statements Should Be Removed"
    }

    fn description(&self) -> &'static str {
        "Standalone semicolons or empty statements add no value and indicate incomplete work."
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

        // Match empty_statement nodes
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
                issues.push(Issue::new(
                    "CC_CS_004",
                    "Empty Statement",
                    Severity::Minor,
                    Category::Style,
                    ctx.file_path.to_string_lossy(),
                    pos.row + 1,
                    pos.column,
                    "Empty statement detected. Remove it or replace with meaningful code.",
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["empty", "statement", "semicolon"])
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
        let rule = EmptyStatementRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_standalone_semicolon() {
        let code = r#"
fn example() {
    let x = 5;
    ;
    let y = 10;
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect standalone semicolon");
        assert_eq!(issues[0].rule_id, "CC_CS_004");
    }

    #[test]
    fn test_no_false_positive_normal_code() {
        let code = r#"
fn example() {
    let x = 5;
    let y = 10;
    println!("{} {}", x, y);
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag normal code");
    }

    #[test]
    fn test_detects_multiple_empty_statements() {
        let code = r#"
fn example() {
    ;;
    ;;
}
"#;
        let issues = check_rule(code);
        // Each `;;` creates two empty_statement nodes in Rust AST
        assert!(issues.len() >= 2, "Should detect multiple empty statements");
    }
}