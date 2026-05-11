//! P3 — map/filter with lambda instead of comprehension
//!
//! Detects inefficient use of map/filter with lambda when comprehension would be clearer.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P3"
    name: "Use comprehension instead of map/filter with lambda"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using map() or filter() with lambda is less efficient and readable than using list comprehensions.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let map_lambda_pattern = regex::Regex::new(r"map\s*\(\s*lambda\s+").unwrap();
        let filter_lambda_pattern = regex::Regex::new(r"filter\s*\(\s*lambda\s+").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if map_lambda_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P3",
                    format!("Inefficient map(lambda) pattern detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use list comprehension instead: [f(x) for x in items]"
                )));
            }
            if filter_lambda_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P3",
                    format!("Inefficient filter(lambda) pattern detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use list comprehension instead: [x for x in items if condition(x)]"
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
    fn test_p3_registered() {
        let rule = PY_P3Rule::new();
        assert_eq!(rule.id(), "PY_P3");
    }

    #[test]
    fn test_p3_detects_map_lambda() {
        let rule = PY_P3Rule::new();
        let smelly = r#"
doubled = map(lambda x: x * 2, items)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect map(lambda) pattern");
        assert_eq!(issues[0].rule_id, "PY_P3");
    }

    #[test]
    fn test_p3_detects_filter_lambda() {
        let rule = PY_P3Rule::new();
        let smelly = r#"
evens = filter(lambda x: x % 2 == 0, items)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect filter(lambda) pattern");
        assert_eq!(issues[0].rule_id, "PY_P3");
    }

    #[test]
    fn test_p3_allows_comprehension() {
        let rule = PY_P3Rule::new();
        let clean = r#"
doubled = [x * 2 for x in items]
evens = [x for x in items if x % 2 == 0]
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag list comprehensions");
    }
}
