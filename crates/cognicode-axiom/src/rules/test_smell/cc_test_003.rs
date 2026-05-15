//! CC_TEST_003: Test Skipped Without Reason
//!
//! Detects skip decorators/calls that lack a reason explanation.
//!
//! # Problem
//! Skipped tests without reasons create technical debt. Over time,
//! no one remembers why tests were skipped, and they never get fixed.
//!
//! # Fix
//! Add a descriptive reason to skip decorators/calls:
//! - Python: @skip("reason"), @pytest.mark.skip(reason="...")
//! - JavaScript: it.skip("reason"), test.skip("reason")

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_003 Rule: Test Skipped Without Reason
pub struct TestSkippedWithoutReasonRule;

impl Default for TestSkippedWithoutReasonRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TestSkippedWithoutReasonRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_003")
    }

    fn name(&self) -> &'static str {
        "Test Skipped Without Reason"
    }

    fn description(&self) -> &'static str {
        "Detects skip decorators/calls that lack a reason explanation"
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

        // For Python, look for decorator calls on functions
        let skip_query = match ctx.language {
            SrcLanguage::Python => {
                // Match decorator on function definition
                r#"(decorator
                    (identifier) @decorator)"#
            }
            SrcLanguage::JavaScript => {
                // Match .skip calls on it, test, describe etc
                r#"(call_expression
                    function: (member_expression
                        property: (property_identifier) @prop))"#
            }
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, skip_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Skip decorator/function names
        let skip_patterns: &[&str] = match ctx.language {
            SrcLanguage::Python => &["skip", "skipIf", "skipUnless", "xfail", "unittest.skip"],
            SrcLanguage::JavaScript => &["skip"],
            _ => return issues,
        };

        // Valid test framework objects for JS
        let valid_skip_objs = ["describe", "it", "test", "xdescribe", "xit", "xtest", "context", "specify", "ftest", "fdescribe"];

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                let text = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                let pos = cap.node.start_position();

                match ctx.language {
                    SrcLanguage::Python => {
                        // Check if decorator name matches skip pattern
                        let is_skip_decorator = skip_patterns.iter().any(|p| text == *p);
                        if is_skip_decorator {
                            issues.push(Issue::new(
                                "CC_TEST_003",
                                "Test Skipped Without Reason",
                                Severity::Minor,
                                Category::TestSmell,
                                ctx.file_path.to_string_lossy(),
                                pos.row + 1,
                                pos.column,
                                "Test is skipped without a reason. Add a descriptive reason \
                                 explaining why this test is skipped, including any relevant \
                                 ticket numbers or issue references.",
                            ));
                        }
                    }
                    SrcLanguage::JavaScript => {
                        // For JS, we need to check both the object and property
                        // property is captured as @prop, we need to find the object too
                        if text == "skip" {
                            // cap.node is the property_identifier ("skip")
                            // its parent is the member_expression (it.skip)
                            // we need to get the object from the member_expression
                            let node = cap.node;
                            if let Some(member_node) = node.parent() {
                                if member_node.kind() == "member_expression" {
                                    if let Some(obj_node) = member_node.child_by_field_name("object") {
                                        let obj_text = obj_node.utf8_text(source.as_bytes()).unwrap_or("");
                                        if valid_skip_objs.contains(&obj_text) {
                                            issues.push(Issue::new(
                                                "CC_TEST_003",
                                                "Test Skipped Without Reason",
                                                Severity::Minor,
                                                Category::TestSmell,
                                                ctx.file_path.to_string_lossy(),
                                                pos.row + 1,
                                                pos.column,
                                                "Test is skipped without a reason. Add a descriptive reason \
                                                 explaining why this test is skipped, including any relevant \
                                                 ticket numbers or issue references.",
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["skip", "xit", "skipIf", "skipUnless", "@unittest.skip", "@pytest.mark.skip"])
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
        let rule = TestSkippedWithoutReasonRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_skip_without_reason() {
        let code = r#"
@skip
def test_old_feature():
    pass
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect @skip without reason");
        assert_eq!(issues[0].rule_id, "CC_TEST_003");
    }

    #[test]
    fn test_detects_it_skip_without_reason() {
        let code = r#"
it.skip('test pending implementation');
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect it.skip without reason");
    }

    #[test]
    fn test_no_false_positive_with_reason() {
        let code = r#"
@skip('JIRA-123: flaky network test')
def test_network():
    pass
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag skip with reason");
    }
}
