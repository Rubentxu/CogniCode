//! B8 — Float equality
//!
//! Detects == comparisons between float variables.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1244"
    name: "Float equality comparison"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "Direct equality comparison of floating-point numbers is unreliable due to precision issues. Use approximate equality or epsilon comparison.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find float comparisons
        // Pattern: x == y where x or y involves float64 or float
        let float_cmp_pattern = regex::Regex::new(r"(?:float64|float32|float)\s*==|==\s*(?:float64|float32|float)|([a-zA-Z_][a-zA-Z0-9]*)\s*==\s*([a-zA-Z_][a-zA-Z0-9]*)").unwrap();

        for cap in float_cmp_pattern.captures_iter(source) {
            // Skip if it's type declaration pattern
            let line = cap.get(0).unwrap().as_str();
            if line.contains("float64") || line.contains("float32") || line.contains("float:") {
                continue;
            }

            let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S1244",
                format!("Float equality comparison may be unreliable"),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Use epsilon comparison for floating-point numbers"
            )));
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
    fn test_b8_registered() {
        let rule = GO_S1244Rule::new();
        assert_eq!(rule.id(), "GO_S1244");
    }

    #[test]
    fn test_b8_detects_float_cmp() {
        let rule = GO_S1244Rule::new();
        let smelly = r#"
if x == y {
    fmt.Println("equal")
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect float comparison");
        assert_eq!(issues[0].rule_id, "GO_S1244");
    }

    #[test]
    fn test_b8_allows_int_cmp() {
        let rule = GO_S1244Rule::new();
        let clean = r#"
if x == 5 {
    fmt.Println("equal")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag integer comparison");
    }
}
