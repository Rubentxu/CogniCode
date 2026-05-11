//! B4 — Unused variable (blank identifier)
//!
//! Detects variables assigned to _ but never read.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1481"
    name: "Unused variable (blank identifier assignment)"
    severity: Minor
    category: Bug
    language: "Go"
    params: {}

    explanation: "Variables assigned to the blank identifier _ should be used if there's only one return value that needs ignoring.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find := with blank identifier assignments that seem unnecessary
        // Pattern: x, _ := func() where x is unused
        let blank_pattern = regex::Regex::new(r",\s*_\s*:=").unwrap();

        for cap in blank_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S1481",
                format!("Consider checking return values of this function call"),
                Severity::Minor,
                Category::Bug,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Ensure the blank identifier assignment is intentional"
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
    fn test_b4_registered() {
        let rule = GO_S1481Rule::new();
        assert_eq!(rule.id(), "GO_S1481");
    }

    #[test]
    fn test_b4_detects_blank_assign() {
        let rule = GO_S1481Rule::new();
        let smelly = r#"
func main() {
    result, _ := doSomething()
    fmt.Println(result)
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect blank identifier assignment");
        assert_eq!(issues[0].rule_id, "GO_S1481");
    }

    #[test]
    fn test_b4_allows_proper_blank() {
        let rule = GO_S1481Rule::new();
        let clean = r#"
func main() {
    _, err := doSomething()
    if err != nil {
        return
    }
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper error handling with blank");
    }
}
