//! B6 — Identical operands
//!
//! Detects expressions with identical operands like a == a or a + a.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1764"
    name: "Identical operands in expression"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "Expressions like 'a == a' or 'a + a' are usually bugs. The first is always true (or always false for NaN), the second is equivalent to '2*a'.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find identical operand patterns: extract var op var, compare var names
        for line in source.lines() {
            let re = regex::Regex::new(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\s*(==|!=|\+|-|\*|/)\s*\b([a-zA-Z_][a-zA-Z0-9_]*)\b").unwrap();
            if let Some(cap) = re.captures(line) {
                if let (Some(lhs), Some(_op), Some(rhs)) = (cap.get(1), cap.get(2), cap.get(3)) {
                    if lhs.as_str() == rhs.as_str() {
                        let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                        issues.push(Issue::new(
                            "GO_S1764",
                            format!("Identical operands: '{} {} {}'", lhs.as_str(), _op.as_str(), rhs.as_str()),
                            Severity::Major,
                            Category::Bug,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "This expression is always true, false, or can be simplified"
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
    fn test_b6_registered() {
        let rule = GO_S1764Rule::new();
        assert_eq!(rule.id(), "GO_S1764");
    }

    #[test]
    fn test_b6_detects_identical_eq() {
        let rule = GO_S1764Rule::new();
        let smelly = r#"
if x == x {
    fmt.Println("always")
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect identical operands");
        assert_eq!(issues[0].rule_id, "GO_S1764");
    }

    #[test]
    fn test_b6_allows_normal_cmp() {
        let rule = GO_S1764Rule::new();
        let clean = r#"
if x == y {
    fmt.Println("different")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag different operands");
    }
}
