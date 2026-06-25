//! S115 — Constant naming (UPPER_CASE)
//!
//! Detects module-level variables not following UPPER_CASE naming for constants.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S115"
    name: "Constant names should use UPPER_CASE"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Module-level constants should follow the UPPER_CASE naming convention.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let upper_case_pattern = regex::Regex::new(r"^[A-Z][A-Z0-9_]*$").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Skip empty lines, comments, and imports
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("import ") || trimmed.starts_with("from ") {
                continue;
            }
            // Detect module-level assignments that look like constants
            if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
                break; // Stop at first function or class definition
            }
            if trimmed.contains('=') && !trimmed.contains("==") && !trimmed.contains("!=") {
                if let Some(name) = trimmed.split('=').next() {
                    let var_name = name.trim();
                    // Skip if already UPPER_CASE
                    if upper_case_pattern.is_match(var_name) {
                        continue;
                    }
                    // Flag non-UPPER_CASE module-level variables (they look like constants)
                    if var_name.len() >= 2 && !var_name.contains('.') && !var_name.contains('(') {
                        issues.push(Issue::new(
                            "PY_S115",
                            format!("Constant '{}' should use UPPER_CASE naming", var_name),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Rename constant to use UPPER_CASE (e.g., 'MAX_SIZE' instead of 'maxSize')."
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
    fn test_s115_registered() {
        let rule = PY_S115Rule::new();
        assert_eq!(rule.id(), "PY_S115");
    }

    #[test]
    fn test_s115_detects_lowercase_constant() {
        let rule = PY_S115Rule::new();
        let smelly = r#"
max_size = 100
api_url = "https://api.example.com"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect lowercase constant names");
        assert_eq!(issues[0].rule_id, "PY_S115");
    }

    #[test]
    fn test_s115_allows_upper_case() {
        let rule = PY_S115Rule::new();
        let clean = r#"
MAX_SIZE = 100
API_URL = "https://api.example.com"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag UPPER_CASE constant names");
    }
}
