//! T12 — print() in tests
//!
//! Detects print() statements in test code, which should use proper assertions instead.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T12"
    name: "print() statement in tests"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using print() in tests indicates debugging code or manual verification. Tests should use assertions to verify behavior.",
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
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = &remaining[2..body_end];

                // Check for print statements
                let print_pattern = regex::Regex::new(r"(?m)^\s*print\s*\(").unwrap();

                if print_pattern.is_match(method_body) {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T12",
                        format!("Test '{}' contains print() statement", method_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Replace print() with assertions or use proper test reporting."
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
    fn test_t12_registered() {
        let rule = PY_T12Rule::new();
        assert_eq!(rule.id(), "PY_T12");
    }

    #[test]
    fn test_t12_detects_print_in_test() {
        let rule = PY_T12Rule::new();
        let smelly = r#"
def test_something():
    print("Debug: starting test")
    x = 1
    assert x == 1
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect print in test");
        assert_eq!(issues[0].rule_id, "PY_T12");
    }

    #[test]
    fn test_t12_allows_print_outside_test() {
        let rule = PY_T12Rule::new();
        let clean = r#"
def helper():
    print("Debug info")
    return 1

def test_something():
    assert helper() == 1
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag print outside test methods");
    }
}
