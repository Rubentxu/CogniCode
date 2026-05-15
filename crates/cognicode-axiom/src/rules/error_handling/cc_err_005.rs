//! CC_ERR_005: Error Chain Should Be Propagated With ? Operator
//!
//! Detects when errors are wrapped without using the ? operator.
//!
//! # Problem
//! When catching an error and returning it, using Err(X.into()) instead of ?.
//! loses the error chain information.
//!
//! # Fix
//! Use the ? operator to preserve the full error chain.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_005 Rule: Error Chain Breaking
pub struct ErrorChainRule;

impl Default for ErrorChainRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ErrorChainRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_005")
    }

    fn name(&self) -> &'static str {
        "Error Chain Should Be Propagated With ? Operator"
    }

    fn description(&self) -> &'static str {
        "When catching an error and returning it, use ? instead of match or if let to preserve the error chain."
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

        // Match Err().into() pattern - call_expression "Err" with a call_expression "into" as argument
        // Works for both explicit return statements and implicit returns (last expression in block)
        let query_str = r#"(call_expression
            function: (identifier) @macro
            arguments: (arguments
                (call_expression
                    function: (field_expression
                        field: (field_identifier) @method))))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                if name == "Err" {
                    let pos = cap.node.start_position();
                    issues.push(Issue::new(
                        "CC_ERR_005",
                        "Error Chain Breaking",
                        Severity::Minor,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        "Use ? operator to preserve error chain instead of Err(x.into()).",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["Err", "into", "?"])
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
        let rule = ErrorChainRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_err_into() {
        let code = r#"
fn foo() -> Result<(), Box<dyn std::error::Error>> {
    Err(e.into())
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect Err().into() pattern");
    }

    #[test]
    fn test_no_false_positive_question_mark() {
        let code = r#"
fn foo() -> Result<(), Box<dyn std::error::Error>> {
    some_result?;
    Ok(())
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag ? operator");
    }
}
