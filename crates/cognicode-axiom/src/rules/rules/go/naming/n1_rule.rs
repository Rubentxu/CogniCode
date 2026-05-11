//! N1 — Function naming (camelCase)
//!
//! Detects function definitions that don't follow camelCase naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S100"
    name: "Function naming should use camelCase"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Function names in Go should use camelCase (start with lowercase, mixed case thereafter). Detected functions not following camelCase convention.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all function definitions
        // Go exported functions start with uppercase, unexported with lowercase
        // We flag: func lowercase() or func snake_case()
        let func_pattern = regex::Regex::new(r"func\s+([a-z_][a-zA-Z0-9_]*)\s*\(").unwrap();

        for cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = cap.get(1) {
                let func_name_str = func_name.as_str();
                // Check if name contains underscores (snake_case) or starts with uppercase
                // camelCase: no underscores, first letter lowercase
                if func_name_str.contains('_') || func_name_str.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    // For exported functions (starts with uppercase), camelCase is fine
                    // For unexported, if it has underscores, flag it
                    if func_name_str.starts_with('_') || (func_name_str.contains('_') && !func_name_str.starts_with(char::is_uppercase)) {
                        let line_num = source[..func_name.start()].lines().count() + 1;
                        issues.push(Issue::new(
                            "GO_S100",
                            format!("Function '{}' should use camelCase naming", func_name_str),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Rename function to use camelCase: start with lowercase, no underscores"
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
    fn test_n1_registered() {
        let rule = GO_S100Rule::new();
        assert_eq!(rule.id(), "GO_S100");
    }

    #[test]
    fn test_n1_detects_snake_case() {
        let rule = GO_S100Rule::new();
        let smelly = r#"
func my_function() {
    return
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect snake_case function name");
        assert_eq!(issues[0].rule_id, "GO_S100");
    }

    #[test]
    fn test_n1_allows_camel_case() {
        let rule = GO_S100Rule::new();
        let clean = r#"
func myFunction() {
    return
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag camelCase function names");
    }

    #[test]
    fn test_n1_allows_exported_function() {
        let rule = GO_S100Rule::new();
        let clean = r#"
func MyFunction() {
    return
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag exported functions");
    }
}
