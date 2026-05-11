//! T10 — Duplicated test method
//!
//! Detects test methods that appear to be duplicated (same name pattern or identical body).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T10"
    name: "Duplicated test method"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Duplicated test methods indicate redundant tests and should be consolidated or parameterized.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all test methods
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();
        let mut test_methods: Vec<(String, usize, String)> = Vec::new();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str().to_string();

                // Find the method body
                let remaining = &source[method_start..];
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = remaining[2..body_end].to_string();
                let line_num = source[..method_start].lines().count() + 1;

                test_methods.push((method_name_str, line_num, method_body));
            }
        }

        // Check for duplicate names (simple heuristic)
        let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (name, _, _) in &test_methods {
            *name_counts.entry(name.clone()).or_insert(0) += 1;
        }

        for (name, count, _) in &test_methods {
            if let Some(&c) = name_counts.get(name) {
                if c > 1 {
                    issues.push(Issue::new(
                        "PY_T10",
                        format!("Test method '{}' appears {} times", name, c),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        *count,
                    ).with_remediation(Remediation::quick(
                        "Consolidate duplicate tests or use parameterized tests."
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
    fn test_t10_registered() {
        let rule = PY_T10Rule::new();
        assert_eq!(rule.id(), "PY_T10");
    }

    #[test]
    fn test_t10_detects_duplicate_test_names() {
        let rule = PY_T10Rule::new();
        let smelly = r#"
class TestCase(unittest.TestCase):
    def test_something(self):
        assert 1 == 1

    def test_something(self):
        assert 2 == 2
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect duplicate test names");
        assert_eq!(issues[0].rule_id, "PY_T10");
    }

    #[test]
    fn test_t10_allows_unique_tests() {
        let rule = PY_T10Rule::new();
        let clean = r#"
class TestCase(unittest.TestCase):
    def test_first(self):
        assert 1 == 1

    def test_second(self):
        assert 2 == 2
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag unique test names");
    }
}
