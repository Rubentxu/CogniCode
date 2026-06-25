//! B1 — panic() in library code
//!
//! Detects panic() calls outside of main() function.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1148"
    name: "panic() should not be used in library code"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "panic() crashes the entire program and should not be used in library code. Return errors instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find panic() calls
        let panic_pattern = regex::Regex::new(r"\bpanic\s*\(").unwrap();

        // Find main() function boundaries
        let main_pattern = regex::Regex::new(r"func\s+main\s*\(\)").unwrap();
        let main_match = main_pattern.find(source);

        for cap in panic_pattern.find_iter(source) {
            let panic_line = source[..cap.start()].lines().count() + 1;

            // If we found main() and panic is after it, it's likely in main
            // This is a simplified heuristic
            if let Some(m) = main_match {
                let main_line = source[..m.start()].lines().count() + 1;
                if panic_line >= main_line && panic_line <= main_line + 50 {
                    // Likely in main, skip
                    continue;
                }
            }

            issues.push(Issue::new(
                "GO_S1148",
                format!("panic() should not be used in library code"),
                Severity::Major,
                Category::Bug,
                ctx.file_path,
                panic_line,
            ).with_remediation(Remediation::quick(
                "Return an error instead of panicking"
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
    fn test_b1_registered() {
        let rule = GO_S1148Rule::new();
        assert_eq!(rule.id(), "GO_S1148");
    }

    #[test]
    fn test_b1_detects_panic() {
        let rule = GO_S1148Rule::new();
        let smelly = r#"
package main

func Open() {
    panic("not implemented")
}
"#;
        let issues = with_go_context(smelly, "lib.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect panic in library code");
        assert_eq!(issues[0].rule_id, "GO_S1148");
    }

    #[test]
    fn test_b1_allows_no_panic() {
        let rule = GO_S1148Rule::new();
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
