//! RX46 — useEffect with setState without deps
//!
//! Detects useEffect with setState that may cause infinite loop.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX46"
    name: "useEffect with setState may cause infinite loop"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "setState in useEffect without proper dependencies causes infinite re-renders.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find useEffect with setState but no deps array
        let effect_with_setstate = regex::Regex::new(r"useEffect\s*\(\s*\(\s*\)\s*=>\s*\{[^}]*set\w+\s*\([^)]*\)").unwrap();
        let deps_pattern = regex::Regex::new(r"\[(.*?)\]").unwrap();

        for cap in effect_with_setstate.find_iter(source) {
            let effect_str = cap.as_str();
            // Check if it has empty or no deps
            if !deps_pattern.is_match(effect_str) || effect_str.contains("[]") {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_RX46",
                    "setState in useEffect without proper dependencies".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Add proper dependencies array or use useMemo/useCallback"
                )));
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
    fn test_rx46_registered() {
        let rule = JS_RX46Rule::new();
        assert_eq!(rule.id(), "JS_RX46");
    }

    #[test]
    fn test_rx46_detects_setstate_in_effect() {
        let rule = JS_RX46Rule::new();
        let smelly = r#"
useEffect(() => {
    setCount(count + 1);
});
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect setState in useEffect");
        assert_eq!(issues[0].rule_id, "JS_RX46");
    }

    #[test]
    fn test_rx46_allows_proper_deps() {
        let rule = JS_RX46Rule::new();
        let clean = r#"
useEffect(() => {
    setCount(count + 1);
}, [count]);
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow proper deps");
    }
}
