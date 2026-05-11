//! T2 — Test with time.sleep()
//!
//! Detects test functions that use time.sleep() which makes tests slow and unreliable.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T2"
    name: "Test with time.sleep()"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using time.sleep() in tests makes them slow and indicates a race condition or timing dependency that should be handled properly.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all test methods (def test_*)
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body
                let remaining = &source[method_start..];
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = &remaining[2..body_end];

                // Check for time.sleep
                if method_body.contains("time.sleep") {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T2",
                        format!("Test method '{}' uses time.sleep()", method_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use mocks or event-based waiting instead of time.sleep()."
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
    fn test_t2_registered() {
        let rule = PY_T2Rule::new();
        assert_eq!(rule.id(), "PY_T2");
    }

    #[test]
    fn test_t2_detects_time_sleep_in_test() {
        let rule = PY_T2Rule::new();
        let smelly = r#"
def test_something():
    import time
    time.sleep(0.1)
    result = 1 + 1
    assert result == 2
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect time.sleep() in test");
        assert_eq!(issues[0].rule_id, "PY_T2");
    }

    #[test]
    fn test_t2_allows_time_sleep_outside_test() {
        let rule = PY_T2Rule::new();
        let clean = r#"
import time

def helper():
    time.sleep(1)

def test_something():
    result = 1 + 1
    assert result == 2
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag time.sleep outside test methods");
    }
}
