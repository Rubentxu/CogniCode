//! CC_CS_001: TODO and FIXME Comments Should Be Resolved
//!
//! Detects comments containing TODO, FIXME, XXX, or HACK markers.
//!
//! # Problem
//! TODO and FIXME comments indicate incomplete code that should be addressed.
//! Leaving them in production code creates technical debt.
//!
//! # Fix
//! Either implement the missing functionality, remove the TODO comment,
//! or create a GitHub issue to track the work.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_001 Rule: TODO/FIXME Comment Detection
pub struct TodoFixmeCommentRule;

impl Default for TodoFixmeCommentRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TodoFixmeCommentRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_001")
    }

    fn name(&self) -> &'static str {
        "TODO and FIXME Comments Should Be Resolved"
    }

    fn description(&self) -> &'static str {
        "TODO and FIXME comments indicate incomplete code. Either implement the missing functionality or create a tracking issue."
    }

    fn category(&self) -> Category {
        Category::Maintainability
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

        // Match line comments and block comments
        let query_str = r#"
            (line_comment) @comment
            (block_comment) @comment
        "#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let comment_text = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                let comment_upper = comment_text.to_uppercase();

                // Check for TODO, FIXME, XXX, HACK, UNDONE, DEPRECATED markers
                let markers = ["TODO", "FIXME", "XXX", "HACK", "UNDONE", "DEPRECATED"];
                let has_marker = markers.iter().any(|m| comment_upper.contains(m));

                // Exclude generated code markers
                let is_generated = comment_upper.contains("GENERATED")
                    || comment_upper.contains("DO NOT EDIT");

                // Exclude test files (often have intentional TODOs)
                let is_test_file = ctx.file_path.to_string_lossy().contains("_test.")
                    || ctx.file_path.to_string_lossy().contains("/tests/")
                    || ctx.source.contains("#[test]")
                    || ctx.source.contains("#[tokio::test]");

                if has_marker && !is_generated {
                    // For now, report all non-generated TODO/FIXME comments
                    // In test files, TODOs are often intentional placeholders
                    if !is_test_file || !comment_upper.contains("TODO") {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_CS_001",
                            "TODO/FIXME Comment",
                            Severity::Minor,
                            Category::Maintainability,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            &format!(
                                "Comment contains '{}' marker: \"{}\". Consider resolving or creating a tracking issue.",
                                markers.iter().find(|m| comment_upper.contains(*m)).unwrap_or(&"TODO"),
                                if comment_text.len() > 50 { &comment_text[..50] } else { comment_text }
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["todo", "fixme", "comment", "technical debt"])
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

    fn check_rule(code: &str, file_path: &str) -> Vec<Issue> {
        let (tree, source) = parse_code(code);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new(file_path),
            &SrcLanguage::Rust,
            &metrics,
        );
        let rule = TodoFixmeCommentRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_todo_comment() {
        let code = r#"
// TODO: implement this function
fn hello() {
    println!("Hello");
}
"#;
        let issues = check_rule(code, "src/main.rs");
        assert!(!issues.is_empty(), "Should detect TODO comment");
        assert_eq!(issues[0].rule_id, "CC_CS_001");
    }

    #[test]
    fn test_detects_fixme_comment() {
        let code = r#"
// FIXME: this is broken
fn hello() {
    println!("Hello");
}
"#;
        let issues = check_rule(code, "src/main.rs");
        assert!(!issues.is_empty(), "Should detect FIXME comment");
    }

    #[test]
    fn test_detects_xxx_comment() {
        let code = r#"
// XXX: deprecated
fn old_function() {}
"#;
        let issues = check_rule(code, "src/main.rs");
        assert!(!issues.is_empty(), "Should detect XXX comment");
    }

    #[test]
    fn test_detects_hack_comment() {
        let code = r#"
// HACK: temporary workaround
fn workaround() {}
"#;
        let issues = check_rule(code, "src/main.rs");
        assert!(!issues.is_empty(), "Should detect HACK comment");
    }

    #[test]
    fn test_no_false_positive_generated_code() {
        let code = r#"
// GENERATED CODE - DO NOT EDIT
fn generated() {}
"#;
        let issues = check_rule(code, "src/main.rs");
        assert!(issues.is_empty(), "Should not flag generated code");
    }

    #[test]
    fn test_no_false_positive_todo_in_string() {
        let code = r#"
fn main() {
    let msg = "TODO: implement this";
}
"#;
        let issues = check_rule(code, "src/main.rs");
        assert!(issues.is_empty(), "Should not flag TODO in string literals");
    }
}