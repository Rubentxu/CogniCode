//! P1 — String concat in loop
//!
//! Detects string concatenation in loops.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1700"
    name: "String concatenation in loop should use strings.Builder"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "String concatenation in loops creates many temporary strings. Use strings.Builder for better performance.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        let lines: Vec<&str> = source.lines().collect();
        let mut in_loop = false;
        let mut loop_indent = 0;

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let line_indent = line.len() - line.trim_start().len();

            if trimmed.starts_with("for ") || trimmed.starts_with("for\t") {
                in_loop = true;
                loop_indent = line_indent;
                continue;
            }

            if in_loop && !trimmed.is_empty() && line_indent <= loop_indent {
                in_loop = false;
            }

            if in_loop && trimmed.contains("+=") {
                issues.push(Issue::new(
                    "GO_S1700",
                    "String concatenation in loop should use strings.Builder",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_idx + 1,
                ).with_remediation(Remediation::quick(
                    "Use strings.Builder for string concatenation in loops"
                )));
                in_loop = false;
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
    fn test_p1_registered() {
        let rule = GO_S1700Rule::new();
        assert_eq!(rule.id(), "GO_S1700");
    }

    #[test]
    fn test_p1_detects_string_concat_in_loop() {
        let rule = GO_S1700Rule::new();
        let smelly = r#"
func main() {
    s := ""
    for i := 0; i < 10; i++ {
        s += "a"
    }
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect string concat in loop");
        assert_eq!(issues[0].rule_id, "GO_S1700");
    }

    #[test]
    fn test_p1_allows_no_concat_in_loop() {
        let rule = GO_S1700Rule::new();
        let clean = r#"
func main() {
    var sb strings.Builder
    for i := 0; i < 10; i++ {
        sb.WriteString("a")
    }
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag strings.Builder usage");
    }
}
