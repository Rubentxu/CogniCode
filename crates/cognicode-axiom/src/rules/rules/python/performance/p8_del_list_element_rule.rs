//! P8 — del on list element (O(n))
//!
//! Detects deletion of list elements which is O(n) operation.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P8"
    name: "Avoid deleting list elements by index"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Deleting elements from a list by index is O(n) because it requires shifting all subsequent elements. Consider using a different data structure.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let del_list_pattern = regex::Regex::new(r"del\s+\w+\s*\[\s*\w+\s*\]").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if del_list_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P8",
                    format!("del on list element detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Consider using a deque or list comprehension to filter instead of deleting elements."
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
    fn test_p8_registered() {
        let rule = PY_P8Rule::new();
        assert_eq!(rule.id(), "PY_P8");
    }

    #[test]
    fn test_p8_detects_del_list_element() {
        let rule = PY_P8Rule::new();
        let smelly = r#"
del items[0]
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect del on list element");
        assert_eq!(issues[0].rule_id, "PY_P8");
    }

    #[test]
    fn test_p8_detects_del_in_loop() {
        let rule = PY_P8Rule::new();
        let smelly = r#"
for i in range(len(items)):
    if items[i] % 2 == 0:
        del items[i]
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect del in loop context");
    }

    #[test]
    fn test_p8_allows_dict_del() {
        let rule = PY_P8Rule::new();
        let clean = r#"
del my_dict["key"]
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        // Dict deletion is O(1), so it's fine
        assert!(issues.is_empty(), "Should not flag del on dict");
    }
}
