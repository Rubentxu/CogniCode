//! CC_TEST_008: Test Using Random Values
//!
//! Detects tests that use random values without a fixed seed.
//!
//! # Problem
//! Tests using random values without seeding produce non-deterministic
//! results, causing flaky tests that pass sometimes and fail other times.
//!
//! # Fix
//! Use a fixed seed for reproducibility:
//! - Python: random.seed(42) or random.seed(fixed_value)
//! - Or use predefined test data factories

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_008 Rule: Test Using Random Values
pub struct TestUsingRandomRule;

impl Default for TestUsingRandomRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TestUsingRandomRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_008")
    }

    fn name(&self) -> &'static str {
        "Test Using Random Values"
    }

    fn description(&self) -> &'static str {
        "Detects tests using random values without a fixed seed"
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

        // Collect call candidates and filter in post-processing
        let random_query = match ctx.language {
            SrcLanguage::Python => {
                // Match any call - we'll filter by checking for random attribute access
                r#"(call
                    function: (_) @func)"#
            }
            SrcLanguage::JavaScript => {
                // Match any call expression - we'll check for Math.random pattern
                r#"(call_expression
                    function: (_) @func)"#
            }
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, random_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Random object patterns (Python)
        let random_objs = ["random", "np.random", "numpy.random", "Random"];
        let random_methods = ["random", "randint", "randrange", "uniform", "choice", "sample", "shuffle", "triangular"];

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                if *field_name == "func" {
                    let func_node = cap.node;
                    let pos = func_node.start_position();

                    match ctx.language {
                        SrcLanguage::Python => {
                            // For Python, check if it's a random.method() call
                            if func_node.kind() == "attribute" {
                                // Use child_by_field_name for attribute fields
                                let obj_node = func_node.child_by_field_name("object");
                                let attr_node = func_node.child_by_field_name("attribute");

                                if let (Some(obj), Some(attr)) = (obj_node, attr_node) {
                                    let obj_text = obj.utf8_text(source.as_bytes()).unwrap_or("");
                                    let attr_text = attr.utf8_text(source.as_bytes()).unwrap_or("");

                                    if random_objs.contains(&obj_text) && random_methods.contains(&attr_text) {
                                        // Check for seed before this
                                        let lines: Vec<&str> = source.lines().collect();
                                        let before_text = lines[..pos.row.min(lines.len())].join("\n");
                                        let has_seed = before_text.contains("random.seed")
                                            || before_text.contains("np.random.seed")
                                            || before_text.contains("numpy.random.seed");

                                        if !has_seed {
                                            issues.push(Issue::new(
                                                "CC_TEST_008",
                                                "Test Using Random Values",
                                                Severity::Major,
                                                Category::TestSmell,
                                                ctx.file_path.to_string_lossy(),
                                                pos.row + 1,
                                                pos.column,
                                                "Test uses random values without a fixed seed. \
                                                 This causes non-deterministic test results. \
                                                 Add random.seed(fixed_value) for reproducibility.",
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        SrcLanguage::JavaScript => {
                            // For JS, check if it's Math.random() pattern
                            if func_node.kind() == "member_expression" {
                                // Use child_by_field_name for member_expression
                                let obj_node = func_node.child_by_field_name("object");
                                let prop_node = func_node.child_by_field_name("property");

                                if let (Some(obj), Some(prop)) = (obj_node, prop_node) {
                                    let obj_text = obj.utf8_text(source.as_bytes()).unwrap_or("");
                                    let prop_text = prop.utf8_text(source.as_bytes()).unwrap_or("");

                                    if obj_text == "Math" && prop_text == "random" {
                                        issues.push(Issue::new(
                                            "CC_TEST_008",
                                            "Test Using Random Values",
                                            Severity::Major,
                                            Category::TestSmell,
                                            ctx.file_path.to_string_lossy(),
                                            pos.row + 1,
                                            pos.column,
                                            "Test uses Math.random() without a fixed seed. \
                                             This causes non-deterministic test results. \
                                             Use a seeded random number generator instead.",
                                        ));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["random", "Random", "Math.random", "randint", "randrange", "uniform"])
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
        let rule = TestUsingRandomRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_random_without_seed() {
        let code = r#"
import random

def test_lottery():
    number = random.randint(1, 100)
    assertTrue(number > 0)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect random without seed");
        assert_eq!(issues[0].rule_id, "CC_TEST_008");
    }

    #[test]
    fn test_detects_math_random() {
        let code = r#"
test('random id generation', () => {
    const id = Math.random();
    expect(generateId()).toBe(id);
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect Math.random");
    }

    #[test]
    fn test_no_false_positive_with_seed() {
        let code = r#"
def test_deterministic():
    random.seed(42)
    result = random.randint(1, 100)
    assertEqual(result, 81)
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag random with seed");
    }
}
