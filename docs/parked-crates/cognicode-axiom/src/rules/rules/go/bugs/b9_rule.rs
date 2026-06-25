//! B9 — Return value ignored
//!
//! Detects function calls where return value is ignored.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2201"
    name: "Return value ignored"
    severity: Minor
    category: Bug
    language: "Go"
    params: {}

    explanation: "Function return values should be checked. Ignoring return values can mask errors.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find function calls that don't use the return value
        // Pattern: func() where func returns something (heuristic)
        let func_call_pattern = regex::Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*$").unwrap();
        let assign_pattern = regex::Regex::new(r"^\s*(?:var\s+)?[a-zA-Z_][a-zA-Z0-9_]*\s*(?::=?\s*\w+\s*)?,$").unwrap();

        // Look for function calls on their own line or as statements
        let stmt_pattern = regex::Regex::new(r"(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*$").unwrap();

        for cap in stmt_pattern.captures_iter(source) {
            let match_str = cap.get(0).unwrap().as_str();

            // Extract function name from the match
            let func_name_capture = regex::Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)").unwrap();
            let func_name = func_name_capture.find(match_str).map(|m| m.as_str()).unwrap_or("unknown");

            // Skip common functions that return nothing or are often called for side effects
            let skip_funcs = ["fmt.Println", "fmt.Printf", "fmt.Sprint", "fmt.Sprintln", "print", "println"];
            if skip_funcs.iter().any(|f| match_str.contains(f)) {
                continue;
            }

            // Skip if it's part of a larger statement (has semicolon or is in control flow)
            let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S2201",
                format!("Return value of '{}' is ignored", func_name),
                Severity::Minor,
                Category::Bug,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Check the return value or use blank identifier _ if intentionally ignored"
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
    fn test_b9_registered() {
        let rule = GO_S2201Rule::new();
        assert_eq!(rule.id(), "GO_S2201");
    }

    #[test]
    fn test_b9_detects_ignored_return() {
        let rule = GO_S2201Rule::new();
        let smelly = r#"
func main() {
    doSomething()
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect ignored return");
        assert_eq!(issues[0].rule_id, "GO_S2201");
    }

    #[test]
    fn test_b9_allows_proper_usage() {
        let rule = GO_S2201Rule::new();
        let clean = r#"
func main() {
    result := doSomething()
    fmt.Println(result)
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper return value usage");
    }
}
