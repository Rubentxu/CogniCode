//! B11 — defer file.Close() missing after open
//!
//! Detects os.Open without subsequent defer file.Close().
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2095"
    name: "Resource not closed after Open"
    severity: Major
    category: Bug
    language: "Go"
    params: {}

    explanation: "Files opened with os.Open should be closed with defer file.Close() to avoid resource leaks.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find os.Open calls
        let open_pattern = regex::Regex::new(r"os\.Open\s*\(").unwrap();

        for cap in open_pattern.find_iter(source) {
            let open_pos = cap.start();
            let open_line = source[..open_pos].lines().count() + 1;

            // Get remaining source after this line
            let remaining_start = source[..open_pos].find('\n').map(|p| p + 1).unwrap_or(open_pos);
            let remaining = &source[remaining_start..];

            // Look for defer close in the next few lines
            let close_pattern = regex::Regex::new(r"defer\s+\w+\.Close\s*\(\)").unwrap();
            let has_close = close_pattern.find(remaining).map(|m| {
                let close_line = source[..remaining_start + m.start()].lines().count() + 1;
                close_line - open_line <= 5  // Within 5 lines
            }).unwrap_or(false);

            if !has_close {
                issues.push(Issue::new(
                    "GO_S2095",
                    format!("Resource not closed after os.Open"),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    open_line,
                ).with_remediation(Remediation::quick(
                    "Use defer file.Close() immediately after os.Open"
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
    fn test_b11_registered() {
        let rule = GO_S2095Rule::new();
        assert_eq!(rule.id(), "GO_S2095");
    }

    #[test]
    fn test_b11_detects_unclosed_resource() {
        let rule = GO_S2095Rule::new();
        let smelly = r#"
func main() {
    file, _ := os.Open("test.txt")
    fmt.Println(file)
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unclosed resource");
        assert_eq!(issues[0].rule_id, "GO_S2095");
    }

    #[test]
    fn test_b11_allows_proper_close() {
        let rule = GO_S2095Rule::new();
        let clean = r#"
func main() {
    file, _ := os.Open("test.txt")
    defer file.Close()
    fmt.Println(file)
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag properly closed resource");
    }
}
