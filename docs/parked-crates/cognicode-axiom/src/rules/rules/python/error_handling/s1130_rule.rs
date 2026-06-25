//! S1130 — Raise in finally
//!
//! Detects raise statements inside finally blocks, which can mask exceptions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1130"
    name: "Raise in finally"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Raising an exception inside a finally block can cause the original exception to be lost. Use with caution or avoid entirely.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;
        let lines: Vec<&str> = source.lines().collect();

        let mut in_finally = false;
        let mut finally_col = 0;
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            let col = lines[i].len() - lines[i].trim_start().len();

            if line.starts_with("finally:") {
                in_finally = true;
                finally_col = col;
                i += 1;
                continue;
            }

            if in_finally {
                if col <= finally_col && !line.is_empty() {
                    in_finally = false;
                    continue;
                }
                if line.starts_with("raise ") {
                    issues.push(Issue::new(
                        "PY_S1130",
                        format!("Raise statement inside finally block at line {}", i + 1),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        i + 1,
                    ).with_remediation(Remediation::quick(
                        "Avoid raising exceptions in finally blocks as they can mask the original error."
                    )));
                }
            }
            i += 1;
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
    fn test_s1130_registered() {
        let rule = PY_S1130Rule::new();
        assert_eq!(rule.id(), "PY_S1130");
    }

    #[test]
    fn test_s1130_detects_raise_in_finally() {
        let rule = PY_S1130Rule::new();
        let smelly = r#"
try:
    do_something()
finally:
    raise RuntimeError("cleanup failed")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect raise in finally block");
        assert_eq!(issues[0].rule_id, "PY_S1130");
    }

    #[test]
    fn test_s1130_allows_normal_finally() {
        let rule = PY_S1130Rule::new();
        let clean = r#"
try:
    do_something()
finally:
    cleanup()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal finally block");
    }
}
