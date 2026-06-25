//! B2 — Empty error handling
//!
//! Detects empty error handling blocks.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S108"
    name: "Empty error handling block"
    severity: Critical
    category: Bug
    language: "Go"
    params: {}

    explanation: "Error handling blocks should not be empty. Ignoring errors can lead to silent failures.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find if err != nil { } patterns
        let empty_err_pattern = regex::Regex::new(r"if\s+err\s*!=\s*nil\s*\{\s*\}(?:\s|$)").unwrap();
        let empty_err_block_pattern = regex::Regex::new(r"if\s+err\s*!=\s*nil\s*\{\s*\n?\s*\}").unwrap();

        for cap in empty_err_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S108",
                format!("Empty error handling block"),
                Severity::Critical,
                Category::Bug,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Handle the error properly or log it"
            )));
        }

        for cap in empty_err_block_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S108",
                format!("Empty error handling block"),
                Severity::Critical,
                Category::Bug,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Handle the error properly or log it"
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
    fn test_b2_registered() {
        let rule = GO_S108Rule::new();
        assert_eq!(rule.id(), "GO_S108");
    }

    #[test]
    fn test_b2_detects_empty_err() {
        let rule = GO_S108Rule::new();
        let smelly = r#"
if err != nil { }
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect empty error handling");
        assert_eq!(issues[0].rule_id, "GO_S108");
    }

    #[test]
    fn test_b2_allows_proper_err() {
        let rule = GO_S108Rule::new();
        let clean = r#"
if err != nil {
    return err
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper error handling");
    }
}
