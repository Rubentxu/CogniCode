//! RX43 — useCallback with empty deps
//!
//! Detects useCallback with empty dependency array.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX43"
    name: "useCallback with empty deps array"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "useCallback with [] dependencies captures values at render time. If the callback uses external values, it may become stale.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find useCallback with []
        let callback_pattern = regex::Regex::new(r"useCallback\s*\([^)]*\[\s*\]\s*\)").unwrap();

        for cap in callback_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "JS_RX43",
                "useCallback with empty deps may capture stale values".to_string(),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Consider if the callback should use a ref instead, or ensure all dependencies are listed"
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
    fn test_rx43_registered() {
        let rule = JS_RX43Rule::new();
        assert_eq!(rule.id(), "JS_RX43");
    }

    #[test]
    fn test_rx43_detects_empty_deps() {
        let rule = JS_RX43Rule::new();
        let smelly = r#"
const memoizedCallback = useCallback(() => {
    doSomething(a, b);
}, []);
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect empty deps");
        assert_eq!(issues[0].rule_id, "JS_RX43");
    }

    #[test]
    fn test_rx43_allows_proper_deps() {
        let rule = JS_RX43Rule::new();
        let clean = r#"
const memoizedCallback = useCallback(() => {
    doSomething(a, b);
}, [a, b]);
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow proper deps");
    }
}
