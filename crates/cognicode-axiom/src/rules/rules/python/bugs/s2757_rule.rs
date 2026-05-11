//! S2757 — Assignment vs comparison in condition
//!
//! Detects `=` used instead of `==` in conditions. Note: Python raises SyntaxError
//! for `if x = y:` but this can appear in list comprehensions or walrus operator context.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2757"
    name: "Assignment should not be used in conditions"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Using '=' instead of '==' in a condition is usually a bug. In Python, this raises SyntaxError in if-statements but may silently work in comprehensions.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect assignment operators inside conditions
        // Pattern: if x = y: (but not walrus := which is ok in comprehensions)
        let assign_in_cond = regex::Regex::new(r"if\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*[a-zA-Z_][a-zA-Z0-9_]*\s*:").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if assign_in_cond.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S2757",
                    "Assignment used instead of comparison in condition",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use '==' for comparison, not '=' for assignment."
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
    fn test_s2757_registered() {
        let rule = PY_S2757Rule::new();
        assert_eq!(rule.id(), "PY_S2757");
    }

    #[test]
    fn test_s2757_detects_assignment_in_condition() {
        let rule = PY_S2757Rule::new();
        let smelly = r#"
if x = y:
    print("oops")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect assignment in condition");
        assert_eq!(issues[0].rule_id, "PY_S2757");
    }

    #[test]
    fn test_s2757_allows_comparison() {
        let rule = PY_S2757Rule::new();
        let clean = r#"
if x == y:
    print("equal")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag comparison");
    }
}
