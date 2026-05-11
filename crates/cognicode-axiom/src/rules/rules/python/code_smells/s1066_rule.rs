//! S1066 — Collapsible if
//!
//! Detects consecutive if statements that could be combined.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1066"
    name: "Collapsible if statements should be merged"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Consecutive if statements that check the same variable with equality comparisons can often be combined.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let lines: Vec<&str> = ctx.source.lines().collect();

        for i in 0..lines.len().saturating_sub(1) {
            let current = lines[i].trim();
            let next = lines[i + 1].trim();

            // Skip empty lines and comments
            if current.is_empty() || current.starts_with('#') || next.is_empty() || next.starts_with('#') {
                continue;
            }

            // Check for pattern like: if x == 1: ... if x == 2:
            if current.starts_with("if ") && current.contains("==") && next.starts_with("if ") && next.contains("==") {
                // Check if both reference same variable
                let current_var = current.split("==").next().unwrap_or("").trim();
                let next_var = next.split("==").next().unwrap_or("").trim();

                if current_var == next_var && !current_var.is_empty() {
                    issues.push(Issue::new(
                        "PY_S1066",
                        "Collapsible if statements detected - consider merging with 'in' operator",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        i + 1,
                    ).with_remediation(Remediation::quick(
                        "Merge consecutive if statements with the same variable: if x in (1, 2):"
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
    fn test_s1066_registered() {
        let rule = PY_S1066Rule::new();
        assert_eq!(rule.id(), "PY_S1066");
    }

    #[test]
    fn test_s1066_detects_collapsible_if() {
        let rule = PY_S1066Rule::new();
        // Consecutive if statements on adjacent lines
        let smelly = r#"if status == 1: pass
if status == 2: pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect collapsible if statements");
        assert_eq!(issues[0].rule_id, "PY_S1066");
    }

    #[test]
    fn test_s1066_allows_independent_if() {
        let rule = PY_S1066Rule::new();
        let clean = r#"
if status == 1:
    process()
if value == 2:
    other()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag independent if statements");
    }
}
