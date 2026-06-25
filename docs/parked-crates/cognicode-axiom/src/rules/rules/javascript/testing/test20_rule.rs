//! TEST20 — toEqual vs toStrictEqual
//!
//! Detects toEqual when toStrictEqual might be better.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST20"
    name: "toEqual used instead of toStrictEqual"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "toStrictEqual checks for undefined properties. toEqual may miss extra properties in objects.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find toEqual with objects
        let toequal_pattern = regex::Regex::new(r"\.toEqual\s*\(\s*\{").unwrap();
        let has_strict_equal = source.contains("toStrictEqual(");

        if !has_strict_equal {
            for cap in toequal_pattern.find_iter(source) {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST20",
                    "toEqual may miss undefined properties; consider toStrictEqual".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use toStrictEqual to catch extra/undefined properties"
                )));
                break;
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
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

    fn with_js_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::JavaScript.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::JavaScript,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_test20_registered() {
        let rule = JS_TEST20Rule::new();
        assert_eq!(rule.id(), "JS_TEST20");
    }

    #[test]
    fn test_test20_detects_toequal() {
        let rule = JS_TEST20Rule::new();
        let smelly = r#"
expect({ a: 1, b: undefined }).toEqual({ a: 1 });
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect toEqual");
        assert_eq!(issues[0].rule_id, "JS_TEST20");
    }

    #[test]
    fn test_test20_allows_strict_equal() {
        let rule = JS_TEST20Rule::new();
        let clean = r#"
expect({ a: 1, b: undefined }).toStrictEqual({ a: 1 });
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow toStrictEqual");
    }
}
