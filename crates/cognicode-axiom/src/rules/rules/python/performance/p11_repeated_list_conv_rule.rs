//! P11 — Repeated list(set(x)) conversion
//!
//! Detects repeated conversion of list to set and back, which is inefficient.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P11"
    name: "Avoid repeated list(set(x)) conversions"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Converting a list to a set and back to remove duplicates is expensive when done repeatedly. Cache the set result.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let list_set_pattern = regex::Regex::new(r"list\s*\(\s*set\s*\(").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if list_set_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P11",
                    format!("list(set(...)) conversion detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Cache the set result if used multiple times: unique_items = list(set(items))"
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
    fn test_p11_registered() {
        let rule = PY_P11Rule::new();
        assert_eq!(rule.id(), "PY_P11");
    }

    #[test]
    fn test_p11_detects_list_set() {
        let rule = PY_P11Rule::new();
        let smelly = r#"
unique = list(set(items))
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect list(set()) pattern");
        assert_eq!(issues[0].rule_id, "PY_P11");
    }

    #[test]
    fn test_p11_allows_set_direct() {
        let rule = PY_P11Rule::new();
        let clean = r#"
unique = set(items)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag set() alone");
    }

    #[test]
    fn test_p11_allows_frozenset() {
        let rule = PY_P11Rule::new();
        let clean = r#"
unique = frozenset(items)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag frozenset()");
    }
}
