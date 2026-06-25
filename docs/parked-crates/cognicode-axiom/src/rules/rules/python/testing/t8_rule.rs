//! T8 — Multiple asserts in one test
//!
//! Detects tests with multiple independent assertions that should be split into separate tests.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T8"
    name: "Multiple asserts in one test"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Multiple assertions in a single test make it harder to identify which specific check failed and can indicate the test is checking too many things.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find test methods
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body
                let remaining = &source[method_start..];
                // Find next method or class to determine body end
                // Search in remaining[2..] to skip "de" from "def"
                let search_in = &remaining[2..];
                let body_end_rel = search_in
                    .find("\ndef ")
                    .or_else(|| search_in.find("\nclass "));

                let body_end = match body_end_rel {
                    Some(pos) => 2 + pos,  // Absolute position
                    None => remaining.len(), // No next method found, use full string
                };
                let method_body = &remaining[..body_end];

                // Count assert statements (excluding comments)
                let assert_pattern = regex::Regex::new(r"(?m)^\s*(?:self\.)?assert").unwrap();
                let assert_count = assert_pattern.find_iter(method_body).count();

                if assert_count > 3 {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T8",
                        format!("Test '{}' has {} assertions - consider splitting", method_name_str, assert_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Split test into multiple focused tests, one per assertion."
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
    fn test_t8_registered() {
        let rule = PY_T8Rule::new();
        assert_eq!(rule.id(), "PY_T8");
    }

    #[test]
    fn test_t8_detects_multiple_asserts() {
        let rule = PY_T8Rule::new();
        let smelly = r#"
def test_something():
    assert 1 == 1
    assert 2 == 2
    assert 3 == 3
    assert 4 == 4
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect multiple asserts");
        assert_eq!(issues[0].rule_id, "PY_T8");
    }

    #[test]
    fn test_t8_allows_single_assert() {
        let rule = PY_T8Rule::new();
        let clean = r#"
def test_something():
    assert 1 == 1
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag single assert");
    }
}
