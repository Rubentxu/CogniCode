//! T1 — Test without assertion
//!
//! Detects test functions that lack any assertion statements.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T1"
    name: "Test without assertion"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Test functions should contain at least one assertion to verify the expected behavior.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all test methods (def test_*)
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body - look for the next def or end of class/file
                let remaining = &source[method_start..];
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = &remaining[2..body_end];

                // Check for assertions in the method body
                let has_assert = method_body.contains("assert")
                    || method_body.contains("self.assert")
                    || method_body.contains("self.assertEqual")
                    || method_body.contains("self.assertTrue")
                    || method_body.contains("self.assertFalse")
                    || method_body.contains("self.assertIs")
                    || method_body.contains("self.assertIsNone")
                    || method_body.contains("self.assertIn")
                    || method_body.contains("self.assertRaises")
                    || method_body.contains("pytest.raises")
                    || method_body.contains("unittest.TestCase.assert");

                if !has_assert {
                    // Calculate line number
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T1",
                        format!("Test method '{}' has no assertions", method_name_str),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Add at least one assertion to verify the expected behavior."
                    )));
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_t1_registered() {
        let rule = PY_T1Rule::new();
        assert_eq!(rule.id(), "PY_T1");
    }

    #[test]
    fn test_t1_detects_test_without_assertion() {
        let rule = PY_T1Rule::new();
        let smelly = r#"
def test_something():
    x = 1
    y = 2
    result = x + y
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect test without assertion");
        assert_eq!(issues[0].rule_id, "PY_T1");
    }

    #[test]
    fn test_t1_allows_test_with_assertion() {
        let rule = PY_T1Rule::new();
        let clean = r#"
def test_something():
    x = 1
    y = 2
    result = x + y
    assert result == 3
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag test with assertion");
    }

    #[test]
    fn test_t1_allows_test_with_self_assert() {
        let rule = PY_T1Rule::new();
        let clean = r#"
def test_something():
    self.assertEqual(1, 1)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag test with self.assertEqual");
    }
}
