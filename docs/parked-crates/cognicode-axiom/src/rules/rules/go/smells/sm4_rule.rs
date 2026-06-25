//! SM4 — TODO/FIXME comments
//!
//! Detects TODO and FIXME comments in code.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1135"
    name: "TODO/FIXME comment should be addressed"
    severity: Info
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "TODO and FIXME comments indicate incomplete work that should be addressed before production.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find TODO and FIXME comments
        let todo_pattern = regex::Regex::new(r"//\s*(?:TODO|FIXME)\s*:?").unwrap();

        for cap in todo_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S1135",
                format!("TODO/FIXME comment should be addressed"),
                Severity::Info,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Address the TODO/FIXME comment or create a tracking issue"
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
    fn test_sm4_registered() {
        let rule = GO_S1135Rule::new();
        assert_eq!(rule.id(), "GO_S1135");
    }

    #[test]
    fn test_sm4_detects_todo() {
        let rule = GO_S1135Rule::new();
        let smelly = r#"
// TODO: implement this
func foo() { }
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect TODO comment");
        assert_eq!(issues[0].rule_id, "GO_S1135");
    }

    #[test]
    fn test_sm4_detects_fixme() {
        let rule = GO_S1135Rule::new();
        let smelly = r#"
// FIXME: this is broken
func foo() { }
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect FIXME comment");
    }

    #[test]
    fn test_sm4_allows_clean_code() {
        let rule = GO_S1135Rule::new();
        let clean = r#"
// This is a proper comment
func foo() { }
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal comments");
    }
}
