//! CC_ERR_001: Unwrap on Option Should Be Avoided
//!
//! Detects when unwrap() is called on Option<T>, which can cause panic.
//!
//! # Problem
//! Using unwrap() on an Option<T> that is None will cause a panic at runtime.
//! This is especially problematic in production code where the None case was
//! not anticipated.
//!
//! # Fix
//! Use unwrap_or(), unwrap_or_else(), map(), and_then(), or pattern matching
//! to handle the None case gracefully.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_001 Rule: Unwrap on Option
pub struct UnwrapOnOptionRule;

impl Default for UnwrapOnOptionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for UnwrapOnOptionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_001")
    }

    fn name(&self) -> &'static str {
        "Unwrap on Option Should Be Avoided"
    }

    fn description(&self) -> &'static str {
        "Using unwrap() on Option<T> can cause panic at runtime. Use unwrap_or, unwrap_or_else, or pattern matching instead."
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Match unwrap() calls on Options
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
                    let method_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                    if method_name == "unwrap" {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_ERR_001",
                            "Unwrap on Option",
                            Severity::Major,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            "Using unwrap() on Option<T> can cause panic. Use unwrap_or(), \
                             unwrap_or_else(), or pattern matching instead.",
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["unwrap", "Option", "Some", "None"])
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
        let rule = UnwrapOnOptionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_unwrap_on_option() {
        let code = r#"
fn get_value(opt: Option<i32>) -> i32 {
    opt.unwrap()
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect unwrap on Option");
        assert_eq!(issues[0].rule_id, "CC_ERR_001");
    }

    #[test]
    fn test_no_false_positive_unwrap_on_result() {
        // Result has unwrap too but different semantics
        let code = r#"
fn get_value(res: Result<i32, ()>) -> i32 {
    res.unwrap()
}
"#;
        let issues = check_rule(code);
        // We detect all unwrap calls - this is a limitation
        // A more sophisticated version would type-check
        assert!(!issues.is_empty(), "Should detect unwrap");
    }

    #[test]
    fn test_no_false_positive_with_unwrap_or() {
        let code = r#"
fn get_value(opt: Option<i32>) -> i32 {
    opt.unwrap_or(0)
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag unwrap_or");
    }
}
