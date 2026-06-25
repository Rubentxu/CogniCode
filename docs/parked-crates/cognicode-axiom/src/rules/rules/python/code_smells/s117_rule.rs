//! S117 — Variable naming (snake_case)
//!
//! Detects variables not following snake_case naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S117"
    name: "Variable names should use snake_case"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Variable names should follow the snake_case naming convention (lowercase with underscores).",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let snake_case_pattern = regex::Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();
        let camel_case_pattern = regex::Regex::new(r"^[a-z]+[A-Z]").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Skip empty lines, comments, imports, and function definitions
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("import ") || trimmed.starts_with("from ") || trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                continue;
            }
            // Detect variable assignments
            if trimmed.contains('=') && !trimmed.contains("==") && !trimmed.contains("!=") {
                if let Some(name) = trimmed.split('=').next() {
                    let var_name = name.trim();
                    // Skip self.* and cls.* references
                    if var_name.starts_with("self.") || var_name.starts_with("cls.") {
                        continue;
                    }
                    // Skip function calls and method chains
                    if var_name.contains('(') || var_name.contains('.') {
                        continue;
                    }
                    if var_name.len() >= 2 && camel_case_pattern.is_match(var_name) {
                        issues.push(Issue::new(
                            "PY_S117",
                            format!("Variable '{}' should use snake_case naming", var_name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Rename variable to use snake_case (e.g., 'my_variable' instead of 'myVariable')."
                        )));
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
    fn test_s117_registered() {
        let rule = PY_S117Rule::new();
        assert_eq!(rule.id(), "PY_S117");
    }

    #[test]
    fn test_s117_detects_camel_case() {
        let rule = PY_S117Rule::new();
        let smelly = r#"
def process():
    myVariable = 42
    return myVariable
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect camelCase variable name");
        assert_eq!(issues[0].rule_id, "PY_S117");
    }

    #[test]
    fn test_s117_allows_snake_case() {
        let rule = PY_S117Rule::new();
        let clean = r#"
def process():
    my_variable = 42
    return my_variable
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag snake_case variable name");
    }
}
