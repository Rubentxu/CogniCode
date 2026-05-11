//! B5 — Self-assignment
//!
//! Detects self-assignment patterns like x = x.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1656"
    name: "Self-assignment detected"
    severity: Critical
    category: Bug
    language: "Go"
    params: {}

    explanation: "Self-assignment like 'x = x' is a bug and does nothing. This is likely a typo or copy-paste error.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find self-assignment patterns: x = x or x := x (without backreferences)
        // We iterate over each line and look for the pattern
        for (line_num, line) in source.lines().enumerate() {
            // Match: identifier = identifier or identifier := identifier
            let assign_pattern = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:=|:=)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*$").unwrap();

            if let Some(cap) = assign_pattern.captures(line) {
                if let (Some(lhs), Some(rhs)) = (cap.get(1), cap.get(2)) {
                    let lhs_str = lhs.as_str();
                    let rhs_str = rhs.as_str();
                    if lhs_str == rhs_str {
                        issues.push(Issue::new(
                            "GO_S1656",
                            format!("Self-assignment detected: '{} = {}'", lhs_str, rhs_str),
                            Severity::Critical,
                            Category::Bug,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Remove the self-assignment or fix the typo"
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
    fn test_b5_registered() {
        let rule = GO_S1656Rule::new();
        assert_eq!(rule.id(), "GO_S1656");
    }

    #[test]
    fn test_b5_detects_self_assign() {
        let rule = GO_S1656Rule::new();
        let smelly = r#"
func main() {
    x := 1
    x = x
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect self-assignment");
        assert_eq!(issues[0].rule_id, "GO_S1656");
    }

    #[test]
    fn test_b5_allows_normal_assign() {
        let rule = GO_S1656Rule::new();
        let clean = r#"
func main() {
    x := 1
    y := 2
    x = y
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal assignment");
    }
}
