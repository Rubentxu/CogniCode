//! N4 — Variable naming (snake_case)
//!
//! Detects local variable assignments that don't follow snake_case naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N4"
    name: "Variable naming should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Local variable names should be snake_case (all lowercase with underscores). Detected camelCase or mixedCase variable assignments.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all variable assignments with camelCase or mixedCase
        // Pattern: variableName = value (starts with lowercase, contains uppercase)
        let var_pattern = regex::Regex::new(r"(?m)^\s*([a-z][a-zA-Z_]*)\s*=").unwrap();

        for cap in var_pattern.captures_iter(source) {
            if let Some(var_name) = cap.get(1) {
                let var_name_str = var_name.as_str();
                // Check if name contains uppercase (camelCase or mixedCase)
                if var_name_str.chars().any(|c| c.is_uppercase()) {
                    // Skip if it looks like a constant (all uppercase)
                    if var_name_str == var_name_str.to_uppercase() && var_name_str.contains('_') {
                        continue;
                    }
                    let line_num = source[..var_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N4",
                        format!("Variable '{}' should use snake_case naming", var_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Rename variable to use snake_case: lowercase with underscores"
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
    fn test_n4_registered() {
        let rule = PY_N4Rule::new();
        assert_eq!(rule.id(), "PY_N4");
    }

    #[test]
    fn test_n4_detects_camel_case() {
        let rule = PY_N4Rule::new();
        let smelly = r#"
def func():
    myVar = 5
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect camelCase variable name");
        assert_eq!(issues[0].rule_id, "PY_N4");
    }

    #[test]
    fn test_n4_detects_mixed_case() {
        let rule = PY_N4Rule::new();
        let smelly = r#"
def func():
    someVariable = 10
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mixedCase variable name");
    }

    #[test]
    fn test_n4_allows_snake_case() {
        let rule = PY_N4Rule::new();
        let clean = r#"
def func():
    my_var = 5
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag snake_case variable names");
    }

    #[test]
    fn test_n4_allows_constant() {
        let rule = PY_N4Rule::new();
        let clean = r#"
MY_CONSTANT = 100
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag UPPER_CASE constant names");
    }
}