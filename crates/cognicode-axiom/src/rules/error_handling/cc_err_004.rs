//! CC_ERR_004: Expect on Result Should Include Error Context
//!
//! Detects when expect() is called on Result without meaningful context.
//!
//! # Problem
//! expect() message should describe the error context, not just repeat
//! the default 'called Result::unwrap() on an Err value'.
//!
//! # Fix
//! Provide a meaningful error message that helps with debugging.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_004 Rule: Expect on Result Without Context
pub struct ExpectOnResultRule;

impl Default for ExpectOnResultRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ExpectOnResultRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_004")
    }

    fn name(&self) -> &'static str {
        "Expect on Result Should Include Error Context"
    }

    fn description(&self) -> &'static str {
        "expect() message should describe the error context, not repeat the default message."
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

        // Match result.expect("...") with string argument
        let query_str = r#"(call_expression
            function: (field_expression
                field: (field_identifier) @field_name)
            arguments: (arguments
                (string_literal) @msg))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            let mut field_name = String::new();
            let mut msg = String::new();

            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                match *name {
                    "field_name" => {
                        field_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    }
                    "msg" => {
                        msg = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    }
                    _ => {}
                }
            }

            if field_name == "expect" {
                // Check if message is too short or generic
                let is_generic = msg.len() < 10
                    || msg.contains("unwrap")
                    || msg.contains("Result");

                if is_generic {
                    let pos = m.captures.iter()
                        .find(|c| c.node.kind() == "string_literal")
                        .map(|c| c.node.start_position())
                        .unwrap_or_else(|| tree_sitter::Point::new(0, 0));

                    issues.push(Issue::new(
                        "CC_ERR_004",
                        "Expect Without Context",
                        Severity::Minor,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        "expect() message should provide meaningful context, not generic text.",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["expect", "Result"])
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
        let rule = ExpectOnResultRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_generic_expect_message() {
        let code = r#"
fn get_value() -> i32 {
    result.expect("unwrap")
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect generic expect message");
    }

    #[test]
    fn test_no_false_positive_meaningful_message() {
        let code = r#"
fn get_value() -> i32 {
    result.expect("Failed to parse configuration file")
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag meaningful message");
    }
}
