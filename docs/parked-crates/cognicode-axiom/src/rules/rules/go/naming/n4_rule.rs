//! N4 — Variable naming (camelCase)
//!
//! Detects variable definitions that use underscores (should use camelCase).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S117"
    name: "Variable naming should use camelCase"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Variable names in Go should use camelCase (no underscores). Detected variables with underscores in their names.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find variable declarations with := or var
        // Match: x := value or var x = value
        // We look for patterns like: my_var := or var my_var =
        let var_pattern = regex::Regex::new(r"(?:var\s+)?([a-z][a-zA-Z0-9]*)_\w*\s*(?::=|=)").unwrap();

        for cap in var_pattern.captures_iter(source) {
            if let Some(var_name) = cap.get(1) {
                let var_name_str = var_name.as_str();
                // Check if the full match contains underscores
                let full_match = cap.get(0).unwrap().as_str();
                if full_match.contains('_') {
                    let line_num = source[..var_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S117",
                        format!("Variable '{}' should use camelCase naming", var_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Rename variable to use camelCase: no underscores"
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
    fn test_n4_registered() {
        let rule = GO_S117Rule::new();
        assert_eq!(rule.id(), "GO_S117");
    }

    #[test]
    fn test_n4_detects_underscore_var() {
        let rule = GO_S117Rule::new();
        let smelly = r#"
my_var := 42
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect underscore in variable name");
        assert_eq!(issues[0].rule_id, "GO_S117");
    }

    #[test]
    fn test_n4_allows_camel_case() {
        let rule = GO_S117Rule::new();
        let clean = r#"
myVar := 42
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag camelCase variable names");
    }
}
