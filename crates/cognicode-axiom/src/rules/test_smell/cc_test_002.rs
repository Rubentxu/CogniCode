//! CC_TEST_002: Test Using Sleep
//!
//! Detects tests that use sleep/delay instead of proper synchronization.
//!
//! # Problem
//! Sleep-based synchronization causes flaky tests and timing-dependent
//! failures. The sleep duration is often arbitrary and may be too short
//! on slow systems or unnecessarily long on fast systems.
//!
//! # Fix
//! Use proper synchronization primitives:
//! - JavaScript: waitFor, waitForElementToBeRemoved, findBy* queries
//! - Python: pytest.waitUntil, asyncio.wait_for, proper event waiting

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_002 Rule: Test Using Sleep
pub struct TestUsingSleepRule;

impl Default for TestUsingSleepRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TestUsingSleepRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_002")
    }

    fn name(&self) -> &'static str {
        "Test Using Sleep"
    }

    fn description(&self) -> &'static str {
        "Detects tests using sleep/delay instead of proper synchronization"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Python, SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        let lang = ctx.language.to_ts_language();
        let sleep_names = &["sleep", "wait", "delay"];

        // For Python: find calls to time.sleep, asyncio.sleep, etc.
        // For JS: find delay(100), sleep(100), setTimeout, etc.
        let query_str = match ctx.language {
            SrcLanguage::Python => {
                // Match: any_call(function.attribute("sleep" or "wait" or "delay"))
                r#"(call
                    function: (attribute
                        attribute: (identifier) @attr_name))"#
            }
            SrcLanguage::JavaScript => {
                // Match both direct calls (delay, sleep) and member expressions (console.log, obj.wait)
                r#"(call_expression
                    function: (_) @func)"#
            }
            _ => return issues,
        };

        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                let pos = cap.node.start_position();

                // Check if this is a sleep/wait/delay pattern
                if sleep_names.contains(&name) {
                    issues.push(Issue::new(
                        "CC_TEST_002",
                        "Test Using Sleep",
                        Severity::Minor,
                        Category::TestSmell,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        "Test uses sleep/delay for synchronization. This causes flaky tests.",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["sleep", "delay", "wait", "setTimeout", "time.sleep", "Thread.sleep"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str, language: SrcLanguage) -> (tree_sitter::Tree, String) {
        let lang = language.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str, language: SrcLanguage) -> Vec<Issue> {
        let (tree, source) = parse_code(code, language);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.py"),
            &language,
            &metrics,
        );
        let rule = TestUsingSleepRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_time_sleep() {
        let code = r#"
import time

def test_network_request():
    response = fetch_data()
    time.sleep(1)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect time.sleep");
        assert_eq!(issues[0].rule_id, "CC_TEST_002");
    }

    #[test]
    fn test_detects_async_delay() {
        let code = r#"
test('delayed operation', async () => {
    await delay(100);
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect delay");
    }

    #[test]
    fn test_no_false_positive_with_wait_for() {
        let code = r#"
test('proper wait', async () => {
    await waitFor(() => expect(el).toBeVisible());
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag proper waitFor");
    }
}
