//! CC_ERR_002: Expect on Option Should Be Avoided
//!
//! Detects when expect() is called on Option<T>, which can cause panic.
//!
//! # Problem
//! Using expect() on an Option<T> that is None will cause a panic with
//! the provided message. The message is often not helpful for debugging.
//!
//! # Fix
//! Use unwrap_or_else() with proper error handling or convert to Result.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_002 Rule: Expect on Option
pub struct ExpectOnOptionRule;

impl Default for ExpectOnOptionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ExpectOnOptionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_002")
    }

    fn name(&self) -> &'static str {
        "Expect on Option Should Be Avoided"
    }

    fn description(&self) -> &'static str {
        "Using expect() on Option<T> can cause panic. Use unwrap_or_else or proper error handling instead."
    }

    fn category(&self) -> Category {
        Category::Correctness
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

        let query_str = r#"(call_expression
            function: (field_expression
                field: (field_identifier) @method_name)
            arguments: (arguments
                (string_literal) @msg))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                if *field_name == "method_name" {
                    let method_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                    if method_name == "expect" {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_ERR_002",
                            "Expect on Option",
                            Severity::Minor,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            "Using expect() on Option<T> can cause panic. Use unwrap_or_else \
                             with proper error handling instead.",
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["expect", "Option"])
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
        let rule = ExpectOnOptionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_expect_on_option() {
        let code = r#"
fn get_value(opt: Option<i32>) -> i32 {
    opt.expect("value should be present")
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect expect on Option");
    }

    #[test]
    fn test_no_false_positive_unwrap() {
        let code = r#"
fn get_value(opt: Option<i32>) -> i32 {
    opt.unwrap()
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag unwrap");
    }
}
