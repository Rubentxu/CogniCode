//! S100 — Function naming (snake_case)
//!
//! Detects functions not following snake_case naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S100"
    name: "Function names should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Function names should follow the snake_case naming convention (lowercase with underscores).",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let snake_case_pattern = regex::Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                if let Some(start) = trimmed.find("def ") {
                    if let Some(paren) = trimmed.find('(') {
                        let func_name = &trimmed[start + 4..paren].trim();
                        // Skip dunder methods
                        if func_name.starts_with("__") && func_name.ends_with("__") {
                            continue;
                        }
                        if !snake_case_pattern.is_match(func_name) {
                            issues.push(Issue::new(
                                "PY_S100",
                                format!("Function '{}' should use snake_case naming", func_name),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                line_num + 1,
                            ).with_remediation(Remediation::quick(
                                "Rename function to use snake_case (e.g., 'my_function' instead of 'myFunction')."
                            )));
                        }
                    }
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
    fn test_s100_registered() {
        let rule = PY_S100Rule::new();
        assert_eq!(rule.id(), "PY_S100");
    }

    #[test]
    fn test_s100_detects_camel_case() {
        let rule = PY_S100Rule::new();
        let smelly = r#"
def myFunction():
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect camelCase function name");
        assert_eq!(issues[0].rule_id, "PY_S100");
    }

    #[test]
    fn test_s100_allows_snake_case() {
        let rule = PY_S100Rule::new();
        let clean = r#"
def my_function():
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag snake_case function name");
    }

    #[test]
    fn test_s100_allows_dunder() {
        let rule = PY_S100Rule::new();
        let clean = r#"
def __init__(self):
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag dunder methods");
    }
}
