//! P12 — Unnecessary deepcopy
//!
//! Detects unnecessary use of deepcopy which is expensive.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P12"
    name: "Avoid unnecessary deepcopy"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "deepcopy() is expensive. Consider if a shallow copy or other approach would suffice.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let deepcopy_pattern = regex::Regex::new(r"deepcopy\s*\(").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if deepcopy_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P12",
                    format!("deepcopy() detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Consider if a shallow copy (.copy()) or slice copy ([:]) would suffice."
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
    fn test_p12_registered() {
        let rule = PY_P12Rule::new();
        assert_eq!(rule.id(), "PY_P12");
    }

    #[test]
    fn test_p12_detects_deepcopy() {
        let rule = PY_P12Rule::new();
        let smelly = r#"
from copy import deepcopy
copy = deepcopy(obj)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect deepcopy()");
        assert_eq!(issues[0].rule_id, "PY_P12");
    }

    #[test]
    fn test_p12_allows_copy() {
        let rule = PY_P12Rule::new();
        let clean = r#"
copy = obj.copy()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag shallow copy");
    }

    #[test]
    fn test_p12_allows_slice() {
        let rule = PY_P12Rule::new();
        let clean = r#"
copy = items[:]
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag slice copy");
    }
}
