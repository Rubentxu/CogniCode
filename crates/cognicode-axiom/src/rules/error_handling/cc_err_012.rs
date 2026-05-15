//! CC_ERR_012: to_string Should Not Be Used for Error Conversion
//!
//! Detects when to_string() is used for error conversion, losing type info.
//!
//! # Problem
//! Using to_string() on errors loses type information and makes error
//! handling less type-safe.
//!
//! # Fix
//! Use From/TryFrom traits or the ? operator for proper error conversion.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_012 Rule: to_string for Error Conversion
pub struct ToStringErrorRule;

impl Default for ToStringErrorRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for ToStringErrorRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_012")
    }

    fn name(&self) -> &'static str {
        "to_string Should Not Be Used for Error Conversion"
    }

    fn description(&self) -> &'static str {
        "Using to_string() on errors loses type information. Use from() or into() for proper error conversion."
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

        // Match Err(x.to_string()) pattern
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
                let name = &query.capture_names()[cap.index as usize];
                if *name == "macro" {
                    let macro_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                    if macro_name == "Err" {
                        // Check if any method is to_string
                        let has_to_string = m.captures.iter().any(|c| {
                            query.capture_names()[c.index as usize] == "method"
                                && c.node.utf8_text(source.as_bytes()).unwrap_or("") == "to_string"
                        });

                        if has_to_string {
                            let pos = cap.node.start_position();
                            issues.push(Issue::new(
                                "CC_ERR_012",
                                "to_string for Error",
                                Severity::Minor,
                                Category::Correctness,
                                ctx.file_path.to_string_lossy(),
                                pos.row + 1,
                                pos.column,
                                "Using to_string() for error conversion loses type info. Use into() instead.",
                            ));
                        }
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["to_string", "Error", "Err"])
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
        let rule = ToStringErrorRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_to_string_error() {
        let code = r#"
fn foo() -> Result<(), Error> {
    Err(e.to_string())
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect to_string for error");
    }

    #[test]
    fn test_no_false_positive_into() {
        let code = r#"
fn foo() -> Result<(), Error> {
    Err(e.into())
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag into()");
    }
}
