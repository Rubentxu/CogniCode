//! SM8 — File too long (>500 lines)
//!
//! Detects files that are too long.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S122"
    name: "File should not be too long (>500 lines)"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Files with more than 500 lines are hard to navigate and maintain. Consider splitting into smaller files.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        let line_count = source.lines().count();

        if line_count > 500 {
            issues.push(Issue::new(
                "GO_S122",
                format!("File has {} lines (max 500)", line_count),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick(
                "Split this file into smaller, focused files by feature or type"
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
    fn test_sm8_registered() {
        let rule = GO_S122Rule::new();
        assert_eq!(rule.id(), "GO_S122");
    }

    #[test]
    fn test_sm8_detects_long_file() {
        let rule = GO_S122Rule::new();
        let mut smelly = String::from("package main\n\nfunc init() {\n");
        for i in 0..510 {
            smelly.push_str(&format!("    _ = {}\n", i));
        }
        smelly.push_str("}\n");

        let issues = with_go_context(&smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect long file");
        assert_eq!(issues[0].rule_id, "GO_S122");
    }

    #[test]
    fn test_sm8_allows_short_file() {
        let rule = GO_S122Rule::new();
        let clean = r#"
package main

func main() {
    fmt.Println("hello")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag short files");
    }
}
