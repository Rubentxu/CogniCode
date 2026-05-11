//! P14 — len() == 0 instead of not x
//!
//! Detects inefficient empty checks using len() == 0.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P14"
    name: "Use 'not x' instead of 'len(x) == 0' for empty check"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using 'not x' for empty checks is more Pythonic and efficient than 'len(x) == 0'.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let len_zero_pattern = regex::Regex::new(r"len\s*\(\s*\w+\s*\)\s*==\s*0").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if len_zero_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P14",
                    format!("Inefficient len() == 0 check detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use 'not x' instead of 'len(x) == 0' for more Pythonic and efficient code."
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
    fn test_p14_registered() {
        let rule = PY_P14Rule::new();
        assert_eq!(rule.id(), "PY_P14");
    }

    #[test]
    fn test_p14_detects_len_zero() {
        let rule = PY_P14Rule::new();
        let smelly = r#"
if len(items) == 0:
    print("empty")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect len() == 0 pattern");
        assert_eq!(issues[0].rule_id, "PY_P14");
    }

    #[test]
    fn test_p14_allows_not_x() {
        let rule = PY_P14Rule::new();
        let clean = r#"
if not items:
    print("empty")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag 'not x' pattern");
    }

    #[test]
    fn test_p14_allows_len_check_non_zero() {
        let rule = PY_P14Rule::new();
        let clean = r#"
if len(items) > 0:
    print("has items")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag len() > 0");
    }
}
