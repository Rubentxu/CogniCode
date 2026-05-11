//! B10 — log.Fatal in library code
//!
//! Detects log.Fatal calls outside of main() function.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2221"
    name: "log.Fatal should not be used in library code"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "log.Fatal terminates the program and should not be used in library code. Return errors instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find log.Fatal calls
        let fatal_pattern = regex::Regex::new(r"log\.Fatal").unwrap();

        // Find main() function
        let main_pattern = regex::Regex::new(r"func\s+main\s*\(\)").unwrap();
        let main_match = main_pattern.find(source);

        for cap in fatal_pattern.find_iter(source) {
            let fatal_line = source[..cap.start()].lines().count() + 1;

            // If main() exists and log.Fatal is within a reasonable distance, skip
            if let Some(m) = main_match {
                let main_line = source[..m.start()].lines().count() + 1;
                if fatal_line >= main_line && fatal_line <= main_line + 50 {
                    continue;
                }
            }

            issues.push(Issue::new(
                "GO_S2221",
                format!("log.Fatal should not be used in library code"),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                fatal_line,
            ).with_remediation(Remediation::quick(
                "Return an error instead of terminating the program"
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
    fn test_b10_registered() {
        let rule = GO_S2221Rule::new();
        assert_eq!(rule.id(), "GO_S2221");
    }

    #[test]
    fn test_b10_detects_log_fatal() {
        let rule = GO_S2221Rule::new();
        let smelly = r#"
package main

func Open() {
    log.Fatal("not implemented")
}
"#;
        let issues = with_go_context(smelly, "lib.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect log.Fatal in library");
        assert_eq!(issues[0].rule_id, "GO_S2221");
    }

    #[test]
    fn test_b10_allows_no_fatal() {
        let rule = GO_S2221Rule::new();
        let clean = r#"
package main

func Open() error {
    return errors.New("not implemented")
}
"#;
        let issues = with_go_context(clean, "lib.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper error handling");
    }
}
