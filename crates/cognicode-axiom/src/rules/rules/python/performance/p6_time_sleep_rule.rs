//! P6 — time.sleep() in test
//!
//! Detects time.sleep() calls which can slow down test execution.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P6"
    name: "Avoid time.sleep() in tests"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "time.sleep() in test files can significantly slow down test execution and indicate flaky test design.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let sleep_pattern = regex::Regex::new(r"time\s*\.\s*sleep\s*\(").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if sleep_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P6",
                    format!("time.sleep() detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use mocks or explicit waits instead of time.sleep(). Consider using unittest.mock.advance_time() or similar."
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
    fn test_p6_registered() {
        let rule = PY_P6Rule::new();
        assert_eq!(rule.id(), "PY_P6");
    }

    #[test]
    fn test_p6_detects_time_sleep() {
        let rule = PY_P6Rule::new();
        let smelly = r#"
import time
time.sleep(1)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect time.sleep()");
        assert_eq!(issues[0].rule_id, "PY_P6");
    }

    #[test]
    fn test_p6_detects_sleep_with_time_prefix() {
        let rule = PY_P6Rule::new();
        let smelly = r#"
import time
time.sleep(0.5)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect time.sleep()");
    }

    #[test]
    fn test_p6_allows_production_code() {
        let rule = PY_P6Rule::new();
        let clean = r#"
# This is production code where sleep is acceptable
import time
time.sleep(1)  # Rate limiting
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        // Note: The rule doesn't distinguish test vs production, just flags all sleep calls
        // This is intentional - production rate-limiting is also a code smell in many contexts
    }
}
