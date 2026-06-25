//! SM6 — Empty function
//!
//! Detects empty function bodies.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1186"
    name: "Empty function should be removed or filled"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Empty functions add no value and may indicate incomplete implementation.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find function definitions
        let func_pattern = regex::Regex::new(r"func\s+(\w+)\s*\([^)]*\)\s*\{\s*\}").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let line_num = source[..func_name.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "GO_S1186",
                    format!("Function '{}' has an empty body", func_name.as_str()),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Fill in the function body or remove if not needed"
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
    fn test_sm6_registered() {
        let rule = GO_S1186Rule::new();
        assert_eq!(rule.id(), "GO_S1186");
    }

    #[test]
    fn test_sm6_detects_empty_func() {
        let rule = GO_S1186Rule::new();
        let smelly = r#"
func EmptyFunction() { }
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect empty function");
        assert_eq!(issues[0].rule_id, "GO_S1186");
    }

    #[test]
    fn test_sm6_allows_nonempty_func() {
        let rule = GO_S1186Rule::new();
        let clean = r#"
func NonEmptyFunction() {
    fmt.Println("hello")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag non-empty functions");
    }
}
