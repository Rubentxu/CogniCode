//! CC_ERR_003: Unwrap on Result Should Provide Context
//!
//! Detects when unwrap() is called on Result<T, E> without context.
//!
//! # Problem
//! Using unwrap() on a Result that is Err will panic without providing
//! useful error information for debugging.
//!
//! # Fix
//! Use unwrap_err(), map_err(), or proper error handling with ? operator.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_003 Rule: Unwrap on Result
pub struct UnwrapOnResultRule;

impl Default for UnwrapOnResultRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for UnwrapOnResultRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_003")
    }

    fn name(&self) -> &'static str {
        "Unwrap on Result Should Provide Context"
    }

    fn description(&self) -> &'static str {
        "Using unwrap() on Result<T, E> loses error information. Use unwrap_err(), unwrap_unchecked(), or proper error handling with ? operator."
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

        // Match result.unwrap() pattern
        let query_str = r#"(call_expression
            function: (field_expression
                field: (field_identifier) @method_name))"#;

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
                    let name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                    if name == "unwrap" {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_ERR_003",
                            "Unwrap on Result",
                            Severity::Minor,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            "Using unwrap() on Result loses error information. \
                             Use unwrap_err(), unwrap_unchecked(), or proper error handling with ?.",
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["unwrap", "Result", "Ok", "Err"])
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
        let rule = UnwrapOnResultRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_result_unwrap() {
        let code = r#"
fn get_value() -> i32 {
    result.unwrap()
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect unwrap on Result");
    }

    #[test]
    fn test_no_false_positive_with_question_mark() {
        let code = r#"
fn get_value() -> i32 {
    result?
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag ? operator");
    }
}
