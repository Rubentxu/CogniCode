//! ADV1 — enum with numeric values
//!
//! Detects numeric enums instead of const enums or unions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "TS_ADV1"
    name: "numeric enum should be const enum or union type"
    severity: Minor
    category: CodeSmell
    language: "TypeScript"
    params: {}

    explanation: "Numeric enums generate runtime code. Use const enum or union types for better tree-shaking.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find numeric enums (not const)
        let enum_pattern = regex::Regex::new(r"enum\s+(\w+)\s*\{[^}]*=\s*\d+").unwrap();

        for cap in enum_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "TS_ADV1",
                "numeric enum generates runtime code".to_string(),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Use 'const enum' or union type for better tree-shaking"
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
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    fn with_ts_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::TypeScript.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::TypeScript,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_adv1_registered() {
        let rule = TS_ADV1Rule::new();
        assert_eq!(rule.id(), "TS_ADV1");
    }

    #[test]
    fn test_adv1_detects_numeric_enum() {
        let rule = TS_ADV1Rule::new();
        let smelly = r#"
enum Status {
    Active = 1,
    Inactive = 2
}
"#;
        let issues = with_ts_context(smelly, "types.ts", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect numeric enum");
        assert_eq!(issues[0].rule_id, "TS_ADV1");
    }

    #[test]
    fn test_adv1_allows_const_enum() {
        let rule = TS_ADV1Rule::new();
        let clean = r#"
const enum Status {
    Active = 1,
    Inactive = 2
}
"#;
        let issues = with_ts_context(clean, "types.ts", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow const enum");
    }
}
