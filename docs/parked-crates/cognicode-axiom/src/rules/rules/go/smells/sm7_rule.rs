//! SM7 — Duplicate branches
//!
//! Detects identical case branches in switch statements.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1871"
    name: "Duplicate branches in switch statement"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Identical case branches indicate duplicated logic that should be merged or refactored.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find switch statements and extract case branches
        let case_pattern = regex::Regex::new(r"case\s+([^:]+):\s*([^\n]+)").unwrap();

        let mut case_bodies: Vec<(String, String)> = Vec::new();
        for cap in case_pattern.captures_iter(source) {
            if let (Some(case_label), Some(body)) = (cap.get(1), cap.get(2)) {
                let body_trimmed = body.as_str().trim().to_string();
                if !body_trimmed.is_empty() && body_trimmed != "fallthrough" {
                    case_bodies.push((case_label.as_str().to_string(), body_trimmed));
                }
            }
        }

        // Check for duplicate bodies
        let mut seen: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (case_label, body) in &case_bodies {
            seen.entry(body.clone()).or_default().push(case_label.clone());
        }

        for (body, cases) in seen {
            if cases.len() > 1 {
                // Found duplicate case bodies
                let case_str = cases.join(", ");
                let line_num = source.find(&body).map(|p| source[..p].lines().count() + 1).unwrap_or(1);
                issues.push(Issue::new(
                    "GO_S1871",
                    format!("Duplicate branches in switch: cases {} have identical code", case_str),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Merge duplicate case branches or consolidate with multiple case labels"
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
    fn test_sm7_registered() {
        let rule = GO_S1871Rule::new();
        assert_eq!(rule.id(), "GO_S1871");
    }

    #[test]
    fn test_sm7_detects_duplicate_branches() {
        let rule = GO_S1871Rule::new();
        let smelly = r#"
switch x {
case 1:
    fmt.Println("one")
case 2:
    fmt.Println("one")
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect duplicate branches");
        assert_eq!(issues[0].rule_id, "GO_S1871");
    }

    #[test]
    fn test_sm7_allows_unique_branches() {
        let rule = GO_S1871Rule::new();
        let clean = r#"
switch x {
case 1:
    fmt.Println("one")
case 2:
    fmt.Println("two")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag unique branches");
    }
}
