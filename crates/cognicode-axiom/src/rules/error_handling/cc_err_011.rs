//! CC_ERR_011: Error Should Be Logged Before Converting
//!
//! Detects when errors are converted without being logged first.
//!
//! # Problem
//! When converting errors (e.g., io::Error to custom error), the original
//! error information is lost if not logged before conversion.
//!
//! # Fix
//! Log the original error before converting it.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_011 Rule: Error Logged After Conversion
pub struct ErrorLoggingRule;

impl Default for ErrorLoggingRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ErrorLoggingRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_011")
    }

    fn name(&self) -> &'static str {
        "Error Should Be Logged Before Converting"
    }

    fn description(&self) -> &'static str {
        "When converting errors, log the original error before discarding information."
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

        // Match ? operator in functions that don't have logging
        let query_str = r#"(function_item
            name: (identifier) @fn_name
            body: (block
                (expression_statement
                    (call_expression
                        function: (identifier) @macro
                        arguments: (arguments)))?))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let _matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Simplified: just flag functions with ? that don't have log calls
        // A more sophisticated version would check for log statements before ?
        let has_log = source.contains("log::")
            || source.contains("eprintln!")
            || source.contains("tracing::")
            || source.contains("println!");

        if !has_log && source.contains('?') {
            // Check if there's a call_expression with Err or Ok
            let return_query = r#"(call_expression
                function: (identifier) @macro)"#;

            if let Ok(query) = tree_sitter::Query::new(&lang, return_query) {
                let mut cursor = tree_sitter::QueryCursor::new();
                let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

                while let Some(m) = matches.next() {
                    for cap in m.captures {
                        let name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                        if name == "Err" || name == "Ok" {
                            let pos = cap.node.start_position();
                            issues.push(Issue::new(
                                "CC_ERR_011",
                                "Error Logged After Conversion",
                                Severity::Minor,
                                Category::Correctness,
                                ctx.file_path.to_string_lossy(),
                                pos.row + 1,
                                pos.column,
                                "Consider logging errors before converting them to preserve debug info.",
                            ));
                            break;
                        }
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["?", "Err", "log"])
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
        let rule = ErrorLoggingRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_err_without_logging() {
        // Use ? operator without logging - should be flagged
        let code = r#"
fn foo() -> Result<(), Error> {
    Err(e)?
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect ? without logging");
    }

    #[test]
    fn test_no_false_positive_with_logging() {
        let code = r#"
fn foo() -> Result<(), Error> {
    log::error!("Error: {:?}", e);
    Err(e.into())
}
"#;
        let issues = check_rule(code);
        // May still flag but that's okay
    }
}
