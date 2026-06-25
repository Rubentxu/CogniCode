//! RX42 — useEffect missing return type for cleanup
//!
//! Detects useEffect without proper cleanup function return.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX42"
    name: "useEffect missing cleanup return"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "When useEffect returns a function, it serves as a cleanup. Not returning cleanup when setting up subscriptions/timers causes memory leaks.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find useEffect that returns something other than a function
        let effect_pattern = regex::Regex::new(r"useEffect\s*\(\s*\(\s*\)\s*=>\s*\{([^}]+)\}").unwrap();

        for cap in effect_pattern.captures_iter(source) {
            if let Some(body) = cap.get(1) {
                let body_str = body.as_str();
                // Check if it returns a non-function (e.g., Promise, value)
                let has_return_non_function = body_str.contains("return")
                    && !body_str.contains("return () =>")
                    && !body_str.contains("return function");

                if has_return_non_function {
                    let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JS_RX42",
                        "useEffect should return a cleanup function or nothing".to_string(),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Return a function for cleanup: return () => { /* cleanup */ }"
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
    fn test_rx42_registered() {
        let rule = JS_RX42Rule::new();
        assert_eq!(rule.id(), "JS_RX42");
    }

    #[test]
    fn test_rx42_detects_invalid_return() {
        let rule = JS_RX42Rule::new();
        let smelly = r#"
useEffect(() => {
    const id = setInterval(() => tick(), 1000);
    return id; // Should return cleanup function
});
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect invalid return");
        assert_eq!(issues[0].rule_id, "JS_RX42");
    }

    #[test]
    fn test_rx42_allows_cleanup_return() {
        let rule = JS_RX42Rule::new();
        let clean = r#"
useEffect(() => {
    const id = setInterval(() => tick(), 1000);
    return () => clearInterval(id);
});
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow cleanup function return");
    }
}
