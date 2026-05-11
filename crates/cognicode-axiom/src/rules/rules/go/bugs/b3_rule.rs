//! B3 — Dead store / unused variable
//!
//! Detects variables that are assigned but never used.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S185"
    name: "Variable assigned but never used"
    severity: Minor
    category: Bug
    language: "Go"
    params: {}

    explanation: "Variables that are assigned but never read are usually dead code or indicate a bug.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        let assign_re = regex::Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*:=").unwrap();
        let all_lines: Vec<&str> = source.lines().collect();

        for cap in assign_re.captures_iter(source) {
            if let Some(var_match) = cap.get(1) {
                let var_name = var_match.as_str();
                if var_name == "_" {
                    continue;
                }
                // Find the assignment line and search after it
                let assign_pos = var_match.start();
                let assign_line_start = source[..assign_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
                // Skip to the line after the assignment
                let after_assign = &source[assign_line_start..];
                let after_newline = after_assign[1..].find('\n').map(|p| p + 1).unwrap_or(after_assign.len() - 1);
                let remaining_str = &after_assign[after_newline..];
                let usages = if remaining_str.contains(var_name) { vec![1] } else { vec![] };

                if usages.is_empty() {
                    issues.push(Issue::new(
                        "GO_S185",
                        format!("Variable '{}' assigned but never used", var_name),
                        Severity::Minor,
                        Category::Bug,
                        ctx.file_path,
                        source[..assign_pos].lines().count() + 1,
                    ).with_remediation(Remediation::quick(
                        "Remove the unused variable or use it"
                    )));
                }
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
    fn test_b3_registered() {
        let rule = GO_S185Rule::new();
        assert_eq!(rule.id(), "GO_S185");
    }

    #[test]
    fn test_b3_detects_dead_store() {
        let rule = GO_S185Rule::new();
        let smelly = r#"
func main() {
    x := 1
    y := 2
    fmt.Println(y)
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect dead store");
        assert_eq!(issues[0].rule_id, "GO_S185");
    }

    #[test]
    fn test_b3_allows_used_var() {
        let rule = GO_S185Rule::new();
        let clean = r#"
func main() {
    x := 1
    fmt.Println(x)
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag used variables");
    }
}
