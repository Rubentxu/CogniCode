//! S2227 — Raising Exception without message
//!
//! Detects raising Exception() without any message argument.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2227"
    name: "Raising Exception without message"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Raising an exception without a message makes debugging difficult. Always provide a descriptive error message.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let raise_empty = regex::Regex::new(r"raise\s+Exception\s*\(\s*\)").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            if raise_empty.is_match(line) {
                issues.push(Issue::new(
                    "PY_S2227",
                    format!("Raising Exception without message at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Add a descriptive error message to the exception."
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
    fn test_s2227_registered() {
        let rule = PY_S2227Rule::new();
        assert_eq!(rule.id(), "PY_S2227");
    }

    #[test]
    fn test_s2227_detects_raise_without_message() {
        let rule = PY_S2227Rule::new();
        let smelly = r#"
raise Exception()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect Exception() without message");
        assert_eq!(issues[0].rule_id, "PY_S2227");
    }

    #[test]
    fn test_s2227_allows_raise_with_message() {
        let rule = PY_S2227Rule::new();
        let clean = r#"
raise Exception("Something went wrong")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow Exception with message");
    }
}
