//! CC_TEST_006: Multiple Assertions In Single Test
//!
//! Detects tests with more than 3 assertions.
//!
//! # Problem
//! Multiple assertions in a single test cause the test to fail on the
//! first assertion, hiding whether subsequent assertions would have
//! passed or failed.
//!
//! # Fix
//! Split into multiple focused tests, each with a single assertion.
//! Use parameterized tests for variations of the same check.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_006 Rule: Multiple Assertions In Single Test
pub struct MultipleAssertionsRule;

impl Default for MultipleAssertionsRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for MultipleAssertionsRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_006")
    }

    fn name(&self) -> &'static str {
        "Multiple Assertions In Single Test"
    }

    fn description(&self) -> &'static str {
        "Detects tests with more than 3 assertions"
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

        let (test_query, assertion_query) = match ctx.language {
            SrcLanguage::Python => (
                r#"(function_definition
                    name: (identifier) @test_name
                    body: (block) @test_body
                    (#match? @test_name "^test_"))"#,
                r#"(call
                    function: (identifier) @assert_func
                    (#any? @assert_func
                        "assertEqual" "assertTrue" "assertFalse" "assertIs"
                        "assertIn" "assertNotIn" "assertRaises" "assertAlmostEqual"))"#,
            ),
            SrcLanguage::JavaScript => (
                r#"(call_expression
                    function: (identifier) @test_func
                    arguments: (arguments (arrow_function
                        body: (block) @test_body))
                    (#match? @test_func "^test$|^it$|^specify$"))"#,
                r#"(call_expression
                    function: (call_expression
                        function: (identifier) @expect
                        (#eq? @expect "expect")))"#,
            ),
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(test_query) = tree_sitter::Query::new(&lang, test_query) else {
            return issues;
        };
        let Ok(assertion_query) = tree_sitter::Query::new(&lang, assertion_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&test_query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            let mut test_name = None;
            let mut test_body = None;

            for cap in m.captures {
                let field_name = &test_query.capture_names()[cap.index as usize];
                match *field_name {
                    "test_name" | "test_func" => test_name = Some(cap.node),
                    "test_body" => test_body = Some(cap.node),
                    _ => {}
                }
            }

            if let (Some(name_node), Some(body_node)) = (test_name, test_body) {
                let test_name_text = name_node.utf8_text(source.as_bytes()).unwrap_or("");

                // Count assertions in the test body
                let mut assert_cursor = tree_sitter::QueryCursor::new();
                let mut assert_matches = assert_cursor.matches(
                    &assertion_query,
                    body_node,
                    source.as_bytes(),
                );

                let mut assert_count = 0;
                while let Some(_am) = assert_matches.next() {
                    assert_count += 1;
                }

                let pos = body_node.start_position();

                if assert_count > 3 {
                    issues.push(Issue::new(
                        "CC_TEST_006",
                        "Multiple Assertions In Single Test",
                        Severity::Minor,
                        Category::TestSmell,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        0,
                        format!(
                            "Test '{}' has {} assertions (threshold: 3). \
                             When the first assertion fails, others don't run, \
                             hiding potential issues. Split into separate focused tests.",
                            test_name_text, assert_count
                        ),
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["assert", "expect", "should", "then", "toBe", "toEqual"])
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
        let rule = MultipleAssertionsRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_multiple_assertions() {
        let code = r#"
def test_user():
    assertEqual(user.name, 'Alice')
    assertEqual(user.age, 30)
    assertEqual(user.email, 'alice@test.com')
    assertEqual(user.active, True)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect multiple assertions");
        assert_eq!(issues[0].rule_id, "CC_TEST_006");
    }

    #[test]
    fn test_no_false_positive_single_assertion() {
        let code = r#"
def test_user_name():
    assertEqual(user.name, 'Alice')
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag single assertion");
    }

    #[test]
    fn test_no_false_positive_two_assertions() {
        let code = r#"
def test_user():
    assertEqual(user.name, 'Alice')
    assertEqual(user.age, 30)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag two assertions");
    }
}
