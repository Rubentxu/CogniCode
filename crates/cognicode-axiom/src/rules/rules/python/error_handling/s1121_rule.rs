//! S1121 — Raise generic Exception
//!
//! Detects raising generic Exception instead of specific exception types.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1121"
    name: "Raise generic Exception"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Raising a generic Exception is too broad. Raise a specific exception type that accurately describes the error."
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let raise_pattern = regex::Regex::new(r"raise\s+Exception\s*\(").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            if raise_pattern.is_match(line) {
                issues.push(Issue::new(
                    "PY_S1121",
                    format!("Generic Exception raised at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Raise a specific exception type that accurately describes the error."
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
    fn test_s1121_registered() {
        let rule = PY_S1121Rule::new();
        assert_eq!(rule.id(), "PY_S1121");
    }

    #[test]
    fn test_s1121_detects_generic_exception() {
        let rule = PY_S1121Rule::new();
        let smelly = r#"
def validate(data):
    raise Exception("Invalid data")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect raising generic Exception");
        assert_eq!(issues[0].rule_id, "PY_S1121");
    }

    #[test]
    fn test_s1121_allows_specific_exception() {
        let rule = PY_S1121Rule::new();
        let clean = r#"
def validate(data):
    raise ValueError("Invalid data")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag specific exception types");
    }
}
