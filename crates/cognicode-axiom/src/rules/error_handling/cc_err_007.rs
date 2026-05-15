//! CC_ERR_007: Result Should Be Used Instead of Option for Error Handling
//!
//! Detects functions returning Option where None represents an error.
//!
//! # Problem
//! When a function can fail, using Option<T> instead of Result<T, E>
//! loses error information and context.
//!
//! # Fix
//! Change the return type to Result<T, E> to provide error context.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_007 Rule: Option Used Where Result Expected
pub struct OptionInsteadOfResultRule;

impl Default for OptionInsteadOfResultRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for OptionInsteadOfResultRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_007")
    }

    fn name(&self) -> &'static str {
        "Result Should Be Used Instead of Option for Error Handling"
    }

    fn description(&self) -> &'static str {
        "When a function can fail, use Result<T, E> instead of Option<T> to provide error context."
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

        // Match functions returning Option
        let query_str = r#"(function_item
            (generic_type
                (type_identifier) @type_name)
            (block))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                if *name == "type_name" {
                    let type_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                    if type_name == "Option" {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_ERR_007",
                            "Option Instead of Result",
                            Severity::Major,
                            Category::Correctness,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            "Function returning Option where it can fail. Use Result<T, E> instead.",
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["fn", "Option", "->"])
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
        let rule = OptionInsteadOfResultRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_option_return() {
        let code = r#"
fn find_user(id: u64) -> Option<User> {
    None
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect Option return type");
    }

    #[test]
    fn test_no_false_positive_result() {
        let code = r#"
fn find_user(id: u64) -> Result<User, Error> {
    Ok(User)
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag Result return type");
    }
}
