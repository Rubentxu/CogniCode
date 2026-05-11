//! N1 — Function naming (snake_case)
//!
//! Detects function definitions that don't follow snake_case naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N1"
    name: "Function naming should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Function names should be snake_case (all lowercase with underscores). Detected CamelCase or mixedCase function names.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all function definitions that are NOT snake_case
        // snake_case: all lowercase, may have underscores, may end with underscore
        // We flag: def CamelCase, def mixedCase, def some_CamelCase, etc.
        let func_pattern = regex::Regex::new(r"def\s+([a-z_][a-zA-Z_]*)\s*\(").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let func_name_str = func_name.as_str();
                // Check if name contains uppercase letters (not snake_case)
                if func_name_str.chars().any(|c| c.is_uppercase()) {
                    let line_num = source[..func_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N1",
                        format!("Function '{}' should use snake_case naming", func_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Rename function to use snake_case: lowercase with underscores"
                    )));
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
    fn test_n1_registered() {
        let rule = PY_N1Rule::new();
        assert_eq!(rule.id(), "PY_N1");
    }

    #[test]
    fn test_n1_detects_camel_case() {
        let rule = PY_N1Rule::new();
        let smelly = r#"
def myFunction():
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect CamelCase function name");
        assert_eq!(issues[0].rule_id, "PY_N1");
    }

    #[test]
    fn test_n1_detects_mixed_case() {
        let rule = PY_N1Rule::new();
        let smelly = r#"
def someFunctionName():
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mixedCase function name");
    }

    #[test]
    fn test_n1_allows_snake_case() {
        let rule = PY_N1Rule::new();
        let clean = r#"
def my_function():
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag snake_case function names");
    }

    #[test]
    fn test_n1_allows_private_function() {
        let rule = PY_N1Rule::new();
        let clean = r#"
def _private_function():
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag private functions with underscore prefix");
    }
}