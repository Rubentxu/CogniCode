//! SM10 — Low comment ratio
//!
//! Detects files with less than 10% comments.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S148"
    name: "File should have a higher comment ratio (>10%)"
    severity: Info
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Files with low comment ratios may be harder to understand. Consider adding documentation.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        let total_lines = source.lines().count();
        if total_lines < 10 {
            return issues;  // Skip very short files
        }

        let comment_lines = source.lines()
            .filter(|l| l.trim().starts_with("//"))
            .count();

        let comment_ratio = comment_lines as f64 / total_lines as f64;

        if comment_ratio < 0.10 && total_lines > 100 {
            issues.push(Issue::new(
                "GO_S148",
                format!("File has {:.1}% comments (min 10%)", comment_ratio * 100.0),
                Severity::Info,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick(
                "Add more documentation comments to explain the code"
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
    fn test_sm10_registered() {
        let rule = GO_S148Rule::new();
        assert_eq!(rule.id(), "GO_S148");
    }

    #[test]
    fn test_sm10_detects_low_comments() {
        let rule = GO_S148Rule::new();
        let mut smelly = String::from("package main\n\n");
        for i in 0..150 {
            smelly.push_str(&format!("func f{}() {{\n    fmt.Println({})\n}}\n", i, i));
        }

        let issues = with_go_context(&smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect low comment ratio");
        assert_eq!(issues[0].rule_id, "GO_S148");
    }

    #[test]
    fn test_sm10_allows_well_commented() {
        let rule = GO_S148Rule::new();
        let clean = r#"
// Package main provides examples
package main

// Main entry point
func main() {
    // Print hello
    fmt.Println("hello")
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag well-commented files");
    }
}
