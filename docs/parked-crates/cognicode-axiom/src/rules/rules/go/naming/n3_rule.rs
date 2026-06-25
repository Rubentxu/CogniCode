//! N3 — Constant naming (UPPER_CASE)
//!
//! Detects constant definitions that don't follow UPPER_CASE naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S115"
    name: "Constant naming should use UPPER_CASE"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Constant names in Go should use UPPER_CASE (all caps with underscores). Detected constants not following this convention.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all const declarations
        // Match: const X = or const ( X = multiline
        let const_pattern = regex::Regex::new(r"const\s+([A-Za-z_][A-Za-z0-9_]*)\s*=").unwrap();

        for cap in const_pattern.captures_iter(source) {
            if let Some(const_name) = cap.get(1) {
                let const_name_str = const_name.as_str();
                // Flag if it has lowercase and is not all caps (i.e., mixed case like myConst)
                let has_lower = const_name_str.chars().any(|c| c.is_lowercase());
                let is_upper = const_name_str.chars().all(|c| c.is_uppercase() || c == '_' || c.is_numeric());
                if has_lower && !is_upper {
                    let line_num = source[..const_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S115",
                        format!("Constant '{}' should use UPPER_CASE naming", const_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Rename constant to use UPPER_CASE: all caps with underscores"
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
    fn test_n3_registered() {
        let rule = GO_S115Rule::new();
        assert_eq!(rule.id(), "GO_S115");
    }

    #[test]
    fn test_n3_detects_lowercase_const() {
        let rule = GO_S115Rule::new();
        let smelly = r#"
const myConst = 42
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect lowercase constant name");
        assert_eq!(issues[0].rule_id, "GO_S115");
    }

    #[test]
    fn test_n3_allows_upper_case() {
        let rule = GO_S115Rule::new();
        let clean = r#"
const MY_CONST = 42
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag UPPER_CASE constant names");
    }
}
