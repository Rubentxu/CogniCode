//! B7 — Assignment in condition (= instead of ==)
//!
//! Detects assignment used instead of comparison in conditions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2757"
    name: "Assignment used instead of comparison in condition"
    severity: Critical
    category: Bug
    language: "Go"
    params: {}

    explanation: "Using '=' instead of '==' in a condition is likely a bug. Did you mean to compare instead of assign?",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find if with single = assignment in condition
        // Pattern: if x = val { (should be if x == val {
        let assign_cond_pattern = regex::Regex::new(r"if\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*([^=\s]+)").unwrap();

        for cap in assign_cond_pattern.captures_iter(source) {
            if let Some(var_name) = cap.get(1) {
                let line_num = source[..var_name.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "GO_S2757",
                    format!("Assignment used instead of comparison: did you mean '==' instead of '='?"),
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use '==' for comparison or '!=' for not equal"
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

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_b7_registered() {
        let rule = GO_S2757Rule::new();
        assert_eq!(rule.id(), "GO_S2757");
    }

    #[test]
    fn test_b7_detects_assign_in_cond() {
        let rule = GO_S2757Rule::new();
        let smelly = r#"
if x = 5 {
    fmt.Println(x)
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect assignment in condition");
        assert_eq!(issues[0].rule_id, "GO_S2757");
    }

    #[test]
    fn test_b7_allows_normal_if() {
        let rule = GO_S2757Rule::new();
        let clean = r#"
if x == 5 {
    fmt.Println(x)
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal comparison");
    }
}
