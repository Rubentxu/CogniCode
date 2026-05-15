//! CC_ERR_008: Panic Should Not Be Used for Validation
//!
//! Detects panic!() macro used for input validation.
//!
//! # Problem
//! Using panic!() or unwrap() for input validation can cause unexpected
//! crashes in production instead of graceful error handling.
//!
//! # Fix
//! Use Result or explicit validation with clear error messages.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_ERR_008 Rule: Panic for Validation
pub struct PanicForValidationRule;

impl Default for PanicForValidationRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for PanicForValidationRule {
    fn id(&self) -> RuleId {
        RuleId("CC_ERR_008")
    }

    fn name(&self) -> &'static str {
        "Panic Should Not Be Used for Validation"
    }

    fn description(&self) -> &'static str {
        "Using panic!() for input validation is inappropriate. Use Result or explicit validation."
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn severity(&self) -> Severity {
        Severity::Critical
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Match panic! macro invocations
        let query_str = r#"(macro_invocation
            macro: (identifier) @macro_name)"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                if name == "panic" {
                    let pos = cap.node.start_position();
                    issues.push(Issue::new(
                        "CC_ERR_008",
                        "Panic for Validation",
                        Severity::Critical,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        "panic!() should not be used for validation. Use Result or explicit error handling.",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["panic", "unwrap", "expect"])
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
        let rule = PanicForValidationRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_panic() {
        let code = r#"
fn validate(input: &str) {
    panic!("Invalid input: {}", input);
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect panic!");
    }

    #[test]
    fn test_no_false_positive_result() {
        let code = r#"
fn validate(input: &str) -> Result<(), ValidationError> {
    Ok(())
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag Result-based validation");
    }
}
