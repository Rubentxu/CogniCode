//! T3 — assertEqual vs assertTrue
//!
//! Detects the use of assertTrue(x == y) instead of assertEqual(x, y).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T3"
    name: "assertEqual should be used instead of assertTrue"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using assertEqual(x, y) provides better error messages than assertTrue(x == y) because it shows the actual and expected values.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find assertTrue with comparison patterns
        let assert_true_pattern = regex::Regex::new(r"assertTrue\s*\(\s*[^)]*==[^)]*\)").unwrap();

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if assert_true_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_T3",
                    format!("Use assertEqual() for better error messages at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Replace assertTrue(a == b) with assertEqual(a, b)"
                )));
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
    fn test_t3_registered() {
        let rule = PY_T3Rule::new();
        assert_eq!(rule.id(), "PY_T3");
    }

    #[test]
    fn test_t3_detects_assert_true_with_equals() {
        let rule = PY_T3Rule::new();
        let smelly = r#"
def test_something():
    self.assertTrue(1 == 2)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect assertTrue with ==");
        assert_eq!(issues[0].rule_id, "PY_T3");
    }

    #[test]
    fn test_t3_allows_assert_equal() {
        let rule = PY_T3Rule::new();
        let clean = r#"
def test_something():
    self.assertEqual(1, 2)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag assertEqual");
    }

    #[test]
    fn test_t3_allows_assert_true_without_comparison() {
        let rule = PY_T3Rule::new();
        let clean = r#"
def test_something():
    self.assertTrue(value)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag assertTrue without comparison");
    }
}
