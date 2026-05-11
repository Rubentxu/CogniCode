//! N6 — Missing doc comment on exported function
//!
//! Detects exported functions that lack documentation comments.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S173"
    name: "Exported function should have a doc comment"
    severity: Info
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Exported functions in Go should have documentation comments explaining their purpose and usage.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find exported function definitions (start with uppercase)
        let func_pattern = regex::Regex::new(r"func\s+([A-Z][a-zA-Z0-9]*)\s*\(").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let func_name_str = func_name.as_str();
                let func_start = cap.get(0).unwrap().start();

                // Find the start of the line
                let line_start = source[..func_start].rfind('\n').map(|p| p + 1).unwrap_or(0);
                let line_end = source[func_start..].find('\n').map(|p| func_start + p).unwrap_or(func_start + cap.get(0).unwrap().len());
                let current_line = &source[line_start..line_end];

                // Check if there's a comment on the line before the function
                // Get the line before this one
                let before_line_start = source[..line_start].trim_end().rfind('\n').map(|p| p + 1).unwrap_or(0);
                let before_line = source[before_line_start..line_start].trim();

                // Check if before_line is a doc comment (starts with //)
                if !before_line.starts_with("//") && !before_line.is_empty() {
                    // Also check if there's a blank line before - if so, check the line before that
                    let is_doc = if before_line.is_empty() {
                        let prev_line_start = source[..before_line_start].trim_end().rfind('\n').map(|p| p + 1).unwrap_or(0);
                        let prev_line = source[prev_line_start..before_line_start].trim();
                        prev_line.starts_with("//")
                    } else {
                        false
                    };

                    if !is_doc && !before_line.is_empty() {
                        let line_num = source[..func_start].lines().count() + 1;
                        issues.push(Issue::new(
                            "GO_S173",
                            format!("Exported function '{}' should have a doc comment", func_name_str),
                            Severity::Info,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Add a doc comment above the function explaining its purpose"
                        )));
                    }
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
    fn test_n6_registered() {
        let rule = GO_S173Rule::new();
        assert_eq!(rule.id(), "GO_S173");
    }

    #[test]
    fn test_n6_detects_missing_doc() {
        let rule = GO_S173Rule::new();
        let smelly = r#"
package main

func ExportedFunction() {
    return
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect missing doc comment");
        assert_eq!(issues[0].rule_id, "GO_S173");
    }

    #[test]
    fn test_n6_allows_function_with_doc() {
        let rule = GO_S173Rule::new();
        let clean = r#"
package main

// ExportedFunction does something great
func ExportedFunction() {
    return
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with doc comment");
    }
}
