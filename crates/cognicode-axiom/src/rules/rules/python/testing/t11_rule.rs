//! T11 — broad except in tests
//!
//! Detects overly broad exception handling (except:) in test code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T11"
    name: "Broad except clause in tests"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using bare 'except:' catches all exceptions including KeyboardInterrupt and SystemExit, which can mask bugs.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
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

                // Check for bare except
                let bare_except_pattern = regex::Regex::new(r"except\s*:").unwrap();

                if bare_except_pattern.is_match(method_body) {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T11",
                        format!("Test '{}' uses bare except clause", method_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use 'except Exception:' or a specific exception type instead."
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
    fn test_t11_registered() {
        let rule = PY_T11Rule::new();
        assert_eq!(rule.id(), "PY_T11");
    }

    #[test]
    fn test_t11_detects_bare_except() {
        let rule = PY_T11Rule::new();
        let smelly = r#"
def test_something():
    try:
        x = 1
    except:
        pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect bare except");
        assert_eq!(issues[0].rule_id, "PY_T11");
    }

    #[test]
    fn test_t11_allows_specific_exception() {
        let rule = PY_T11Rule::new();
        let clean = r#"
def test_something():
    try:
        x = 1
    except ValueError:
        pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag specific exception");
    }
}
