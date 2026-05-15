//! CC_TEST_001: Test Without Assertion
//!
//! Detects test functions that have no assertions.
//!
//! # Problem
//! Tests without assertions pass regardless of whether the code under test
//! actually works, producing false confidence.
//!
//! # Fix
//! Add at least one assertion to verify expected behavior:
//! - Python: assertEqual, assertTrue, pytest.raises, etc.
//! - JavaScript: expect().toBe(), expect().toEqual(), mock.assert_called*()

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_001 Rule: Test Without Assertion
pub struct TestWithoutAssertionRule;

impl Default for TestWithoutAssertionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TestWithoutAssertionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_001")
    }

    fn name(&self) -> &'static str {
        "Test Without Assertion"
    }

    fn description(&self) -> &'static str {
        "Detects test functions that have no assertions"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Python, SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Collect test functions, then filter in post-processing
        let test_query = match ctx.language {
            SrcLanguage::Python => {
                // Match all function definitions, we'll filter by name pattern later
                r#"(function_definition
                    name: (identifier) @test_name
                    body: (block) @test_body)"#
            }
            SrcLanguage::JavaScript => {
                // Match call expressions with test/it/specify names
                // The call must have an arrow function in arguments
                r#"(call_expression
                    function: (identifier) @test_func
                    arguments: (arguments
                        (arrow_function
                            body: (statement_block) @test_body)))"#
            }
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, test_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Collect all test function candidates
        let mut test_candidates: Vec<(String, tree_sitter::Node, tree_sitter::Node)> = Vec::new();

        while let Some(m) = matches.next() {
            let mut test_name = None;
            let mut test_body = None;

            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                match *field_name {
                    "test_name" | "test_func" => test_name = Some(cap.node),
                    "test_body" => test_body = Some(cap.node),
                    _ => {}
                }
            }

            if let (Some(name_node), Some(body_node)) = (test_name, test_body) {
                let name_text = name_node.utf8_text(source.as_bytes()).unwrap_or("");

                // Filter by test naming pattern in post-processing
                let is_test = match ctx.language {
                    SrcLanguage::Python => name_text.starts_with("test_") || name_text.starts_with("Test"),
                    SrcLanguage::JavaScript => name_text == "test" || name_text == "it" || name_text == "specify",
                    _ => false,
                };

                if is_test {
                    test_candidates.push((name_text.to_string(), name_node, body_node));
                }
            }
        }

        // Now check each test body for assertions
        for (test_name_text, _name_node, body_node) in test_candidates {
            let body_start = body_node.start_position();
            let body_text = body_node.utf8_text(source.as_bytes()).unwrap_or("");

            // Check for assertion patterns in the body text (post-processing approach)
            let assertion_patterns = match ctx.language {
                SrcLanguage::Python => &["assertEqual", "assertTrue", "assertFalse", "assertIs", "assertIsNone",
                    "assertIn", "assertNotIn", "assertRaises", "assertRaisesRegex",
                    "assertLogs", "assertGreater", "assertLess", "pytest.raises"],
                SrcLanguage::JavaScript => &["expect(", ".toBe(", ".toEqual(", ".toHaveBeenCalled",
                    ".toHaveBeenCalledWith", ".toHaveBeenNthCalledWith", ".toThrow",
                    ".toBeTruthy", ".toBeFalsy", ".toBeNull", ".toBeUndefined",
                    "mock.assert_called", "mock_assert_called"],
                _ => &[] as &[_],
            };

            let has_assertion = assertion_patterns.iter().any(|p| body_text.contains(p));

            if !has_assertion {
                issues.push(Issue::new(
                    "CC_TEST_001",
                    "Test Without Assertion",
                    Severity::Major,
                    Category::TestSmell,
                    ctx.file_path.to_string_lossy(),
                    body_start.row + 1,
                    0,
                    format!(
                        "Test '{}' has no assertions. This test passes regardless of \
                         whether the code under test actually works.",
                        test_name_text
                    ),
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["test", "describe", "it", "def test_", "async def test_", "expect", "assert"])
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
        let rule = TestWithoutAssertionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_test_without_assertion_python() {
        let code = r#"
def test_user_login():
    user = User.login('testuser', 'password')
    # No assertion - test passes incorrectly
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect test without assertion");
        assert_eq!(issues[0].rule_id, "CC_TEST_001");
    }

    #[test]
    fn test_detects_async_test_without_expect() {
        let code = r#"
test('fetches user data', async () => {
    const data = await fetchUser();
    // Missing expect - passes without verification
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect async test without expect");
    }

    #[test]
    fn test_no_false_positive_with_assertion() {
        let code = r#"
def test_addition():
    result = 2 + 2
    assertEqual(result, 4)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag test with assertions");
    }

    #[test]
    fn test_no_false_positive_with_expect() {
        let code = r#"
test('check value', () => {
    expect(add(2, 2)).toBe(4);
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag test with expect");
    }
}
