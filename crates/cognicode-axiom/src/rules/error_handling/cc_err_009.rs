//! CC_ERR_009: unwrap_or_default Should Provide Meaningful Default
//!
//! Detects unwrap_or_default() which can hide errors with meaningless defaults.
//!
//! # Problem
//! Using unwrap_or_default() provides a meaningless default value like 0 or
//! empty string, which can mask errors.
//!
//! # Fix
//! Use unwrap_or() with a meaningful default value.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_009 Rule: Meaningless Default
pub struct UnwrapOrDefaultRule;

impl Default for UnwrapOrDefaultRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for UnwrapOrDefaultRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_009")
    }

    fn name(&self) -> &'static str {
        "unwrap_or_default Should Provide Meaningful Default"
    }

    fn description(&self) -> &'static str {
        "Using unwrap_or_default() can hide errors by providing meaningless defaults like 0 or empty string."
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

        // Match call_expression unwrap_or_default
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
                let name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                if name == "unwrap_or_default" {
                    let pos = cap.node.start_position();
                    issues.push(Issue::new(
                        "CC_ERR_009",
                        "Meaningless Default",
                        Severity::Minor,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        "unwrap_or_default() can hide errors. Use unwrap_or() with a meaningful default.",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["unwrap_or_default"])
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
        let rule = UnwrapOrDefaultRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_unwrap_or_default() {
        let code = r#"
fn get_value() -> i32 {
    opt.unwrap_or_default()
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect unwrap_or_default");
    }

    #[test]
    fn test_no_false_positive_unwrap_or() {
        let code = r#"
fn get_value() -> i32 {
    opt.unwrap_or(42)
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag unwrap_or with value");
    }
}
