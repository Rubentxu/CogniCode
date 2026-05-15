//! CC_ERR_006: Custom Error Type Should Implement std::error::Error
//!
//! Detects custom error types that derive Error but don't implement it properly.
//!
//! # Problem
//! Custom error types should implement std::error::Error trait for proper
//! error handling and debugging capabilities.
//!
//! # Fix
//! Implement std::error::Error for custom error types, or use thiserror crate.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_006 Rule: Custom Error Missing Trait
pub struct CustomErrorTraitRule;

impl Default for CustomErrorTraitRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for CustomErrorTraitRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_006")
    }

    fn name(&self) -> &'static str {
        "Custom Error Type Should Implement std::error::Error"
    }

    fn description(&self) -> &'static str {
        "Custom error types should implement std::error::Error trait for proper error handling."
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

        // Match attribute item with derive and Error identifier
        let query_str = r#"(attribute_item
            (attribute
                (identifier) @derive_name
                (token_tree
                    (identifier) @error_name)))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            let mut derive_name = String::new();
            let mut error_name = String::new();

            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                match *name {
                    "derive_name" => {
                        derive_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    }
                    "error_name" => {
                        error_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    }
                    _ => {}
                }
            }

            if derive_name == "derive" && error_name == "Error" {
                let pos = m.captures.iter()
                    .find(|c| query.capture_names()[c.index as usize] == "error_name")
                    .map(|c| c.node.start_position())
                    .unwrap_or_default();

                issues.push(Issue::new(
                    "CC_ERR_006",
                    "Custom Error Missing Trait",
                    Severity::Minor,
                    Category::Correctness,
                    ctx.file_path.to_string_lossy(),
                    pos.row + 1,
                    pos.column,
                    "Custom error type with #[derive(Error)] should implement std::error::Error.",
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Error", "derive", "struct"])
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
        let rule = CustomErrorTraitRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_derive_error() {
        let code = r#"
#[derive(Error)]
pub struct MyError {
    message: String,
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect #[derive(Error)]");
    }

    #[test]
    fn test_no_false_positive_without_error() {
        let code = r#"
pub struct MyError {
    message: String,
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag without Error derive");
    }
}
