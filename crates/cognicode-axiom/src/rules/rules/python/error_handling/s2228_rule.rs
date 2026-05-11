//! S2228 — Raising string (Python 2 style)
//!
//! Detects raising string literals instead of exception instances.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2228"
    name: "Raising string"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "In Python 3, raising a string literal doesn't create an exception object. Use 'raise ExceptionType(message)' instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Match: raise "something" or raise 'something'
        let raise_string = regex::Regex::new(r#"raise\s+"[^"]*""#).unwrap();
        let raise_single_string = regex::Regex::new(r#"raise\s+'[^']*'"#).unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            if raise_string.is_match(line) || raise_single_string.is_match(line) {
                issues.push(Issue::new(
                    "PY_S2228",
                    format!("Raising string literal at line {} (Python 2 style)", line_num + 1),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Raise an exception instance: raise ValueError('message') instead of raise 'message'."
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
    fn test_s2228_registered() {
        let rule = PY_S2228Rule::new();
        assert_eq!(rule.id(), "PY_S2228");
    }

    #[test]
    fn test_s2228_detects_raise_string() {
        let rule = PY_S2228Rule::new();
        let smelly = r#"
raise "This is an error"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect raising string literal");
        assert_eq!(issues[0].rule_id, "PY_S2228");
    }

    #[test]
    fn test_s2228_allows_raise_exception() {
        let rule = PY_S2228Rule::new();
        let clean = r#"
raise ValueError("This is an error")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow raising exception instances");
    }
}
