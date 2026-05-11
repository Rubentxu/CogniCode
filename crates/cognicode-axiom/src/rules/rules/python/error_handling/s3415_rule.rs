//! S3415 — assert arg order
//!
//! Detects assertEqual(actual, expected) with reversed argument order.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S3415"
    name: "assertEqual argument order"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "assertEqual(first, second) should have the actual value first and expected value second. Reversed arguments produce confusing error messages.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Match assertEqual calls with string literals or numbers swapped
        // Heuristic: assertEqual("literal", variable) or assertEqual(number, variable)
        let assert_equal_pattern = regex::Regex::new(
            r#"assertEqual\s*\(\s*['"][^'"]+['"]\s*,\s*\w+\s*\)"#
        ).unwrap();
        let assert_equal_num_pattern = regex::Regex::new(
            r"assertEqual\s*\(\s*\d+\s*,\s*\w+\s*\)"
        ).unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if (assert_equal_pattern.is_match(trimmed) || assert_equal_num_pattern.is_match(trimmed))
                && !trimmed.contains("#") {
                issues.push(Issue::new(
                    "PY_S3415",
                    format!("Possible reversed assertEqual arguments at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Verify assertEqual(actual, expected) has arguments in correct order."
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
    fn test_s3415_registered() {
        let rule = PY_S3415Rule::new();
        assert_eq!(rule.id(), "PY_S3415");
    }

    #[test]
    fn test_s3415_detects_suspicious_order() {
        let rule = PY_S3415Rule::new();
        let smelly = r#"
self.assertEqual("expected_string", result)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect suspicious assertEqual order");
        assert_eq!(issues[0].rule_id, "PY_S3415");
    }

    #[test]
    fn test_s3415_allows_correct_order() {
        let rule = PY_S3415Rule::new();
        let clean = r#"
self.assertEqual(result, "expected_string")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow correct assertEqual order");
    }
}
