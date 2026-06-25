//! N3 — Constant naming (UPPER_CASE)
//!
//! Detects module-level assignments that should be constants but use lowercase naming.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N3"
    name: "Module-level constants should use UPPER_CASE naming"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Module-level constants should be named with UPPER_CASE (all uppercase with underscores). Detected lowercase constant names at module level.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find module-level assignments (not indented, not inside class/function)
        // Look for lines like: myConst = 42, my_const = "value"
        // Skip if line starts with class, def, import, from, if, for, while, etc.

        let lines: Vec<&str> = source.lines().collect();
        let mut in_function_or_class = false;
        let mut indent_level = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Track if we're entering/exiting a function or class
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                in_function_or_class = true;
                indent_level = line.len() - line.trim_start().len();
                continue;
            }

            // Reset when indent decreases
            let current_indent = line.len() - line.trim_start().len();
            if current_indent < indent_level && in_function_or_class {
                in_function_or_class = false;
            }

            if in_function_or_class {
                continue;
            }

            // Look for assignment statements at module level
            // Pattern: name = value (where name starts with lowercase)
            let assign_pattern = regex::Regex::new(r"^([a-z][a-zA-Z_]*)\s*=").unwrap();
            if let Some(cap) = assign_pattern.captures(trimmed) {
                if let Some(var_name) = cap.get(1) {
                    let var_name_str = var_name.as_str();
                    // Skip common module-level non-constants
                    if var_name_str == "__name__" || var_name_str == "__all__" {
                        continue;
                    }
                    // Flag if it looks like a constant (not a function call result)
                    if !trimmed.contains('(') {
                        issues.push(Issue::new(
                            "PY_N3",
                            format!("Constant '{}' should use UPPER_CASE naming", var_name_str),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_idx + 1,
                        ).with_remediation(Remediation::quick(
                            "Rename constant to UPPER_CASE: use underscores and all uppercase"
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
    fn test_n3_registered() {
        let rule = PY_N3Rule::new();
        assert_eq!(rule.id(), "PY_N3");
    }

    #[test]
    fn test_n3_detects_lowercase_constant() {
        let rule = PY_N3Rule::new();
        let smelly = r#"
myConst = 42
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect lowercase constant name");
        assert_eq!(issues[0].rule_id, "PY_N3");
    }

    #[test]
    fn test_n3_detects_snake_case_constant() {
        let rule = PY_N3Rule::new();
        let smelly = r#"
my_constant = "value"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect snake_case constant name");
    }

    #[test]
    fn test_n3_allows_upper_case() {
        let rule = PY_N3Rule::new();
        let clean = r#"
MY_CONSTANT = 42
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag UPPER_CASE constant names");
    }

    #[test]
    fn test_n3_allows_dunder_names() {
        let rule = PY_N3Rule::new();
        let clean = r#"
__name__ = "__main__"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag dunder names");
    }
}