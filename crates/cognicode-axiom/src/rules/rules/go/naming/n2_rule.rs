//! N2 — Type naming (PascalCase)
//!
//! Detects type definitions that don't follow PascalCase naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S101"
    name: "Type naming should use PascalCase"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Type names in Go should use PascalCase (start with uppercase). Detected types not following PascalCase convention.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all type definitions: type X struct, type X interface, type X int, etc.
        let type_pattern = regex::Regex::new(r"type\s+([a-z_][a-zA-Z0-9_]*)\s+").unwrap();

        for cap in type_pattern.captures_iter(source) {
            if let Some(type_name) = cap.get(1) {
                let type_name_str = type_name.as_str();
                // Check if name starts with lowercase or contains underscores
                if type_name_str.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                    let line_num = source[..type_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S101",
                        format!("Type '{}' should use PascalCase naming", type_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Rename type to use PascalCase: start with uppercase letter"
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
    fn test_n2_registered() {
        let rule = GO_S101Rule::new();
        assert_eq!(rule.id(), "GO_S101");
    }

    #[test]
    fn test_n2_detects_lowercase_type() {
        let rule = GO_S101Rule::new();
        let smelly = r#"
type myType struct {
    Name string
}
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect lowercase type name");
        assert_eq!(issues[0].rule_id, "GO_S101");
    }

    #[test]
    fn test_n2_allows_pascal_case() {
        let rule = GO_S101Rule::new();
        let clean = r#"
type MyType struct {
    Name string
}
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag PascalCase type names");
    }
}
