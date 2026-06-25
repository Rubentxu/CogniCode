//! S2589 — Always-true condition
//!
//! Detects conditions that are always true like `if True:`, `if 1:`, `if "string":`.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2589"
    name: "Condition should not always evaluate to true"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "An if-condition that always evaluates to True (like 'if True:' or 'if 1:') will always execute, making the else branch dead code.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect if True:, if False:, if 1:, if 0:, if "...":, if []: etc.
        let always_true = regex::Regex::new(r#"if\s+(True|1|"[^"]*"|'[^']*'|\[|\{)\s*:"#).unwrap();
        let always_false = regex::Regex::new(r"if\s+(False|0)\s*:\s*(?:else\s*:)?").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if always_true.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S2589",
                    "Condition always evaluates to true",
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Remove the conditional and inline the code, or use a meaningful condition."
                )));
            } else if always_false.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S2589",
                    "Condition always evaluates to false",
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Remove the conditional or invert the logic."
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
    fn test_s2589_registered() {
        let rule = PY_S2589Rule::new();
        assert_eq!(rule.id(), "PY_S2589");
    }

    #[test]
    fn test_s2589_detects_if_true() {
        let rule = PY_S2589Rule::new();
        let smelly = r#"
if True:
    print("always")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect if True:");
        assert_eq!(issues[0].rule_id, "PY_S2589");
    }

    #[test]
    fn test_s2589_detects_if_string() {
        let rule = PY_S2589Rule::new();
        let smelly = r#"
if "constant":
    do_something()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect if string:");
    }

    #[test]
    fn test_s2589_allows_variable_condition() {
        let rule = PY_S2589Rule::new();
        let clean = r#"
if condition:
    do_something()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag variable conditions");
    }
}
