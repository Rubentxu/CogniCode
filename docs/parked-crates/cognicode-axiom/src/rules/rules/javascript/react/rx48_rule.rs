//! RX48 — useImperativeHandle without display name
//!
//! Detects useImperativeHandle without setting displayName.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX48"
    name: "useImperativeHandle without displayName"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "Components using forwardRef with useImperativeHandle should set displayName for debugging.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find useImperativeHandle
        let imperative_pattern = regex::Regex::new(r"useImperativeHandle\s*\(").unwrap();

        for cap in imperative_pattern.find_iter(source) {
            // Find corresponding forwardRef or Component
            let before = &source[..cap.start()];
            let has_forward_ref = before.contains("forwardRef");
            let has_display_name = source[cap.start()..].contains("displayName");

            if has_forward_ref && !has_display_name {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_RX48",
                    "useImperativeHandle used without displayName".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Add displayName: Component.displayName = 'ComponentName'"
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
    fn test_rx48_registered() {
        let rule = JS_RX48Rule::new();
        assert_eq!(rule.id(), "JS_RX48");
    }

    #[test]
    fn test_rx48_detects_missing_display_name() {
        let rule = JS_RX48Rule::new();
        let smelly = r#"
const MyInput = forwardRef((props, ref) => {
    useImperativeHandle(ref, () => ({ focus: () => {} }));
    return <input ref={ref} {...props} />;
});
"#;
        let issues = with_js_context(smelly, "MyInput.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect missing displayName");
        assert_eq!(issues[0].rule_id, "JS_RX48");
    }

    #[test]
    fn test_rx48_allows_display_name() {
        let rule = JS_RX48Rule::new();
        let clean = r#"
const MyInput = forwardRef((props, ref) => {
    useImperativeHandle(ref, () => ({ focus: () => {} }));
    return <input ref={ref} {...props} />;
});
MyInput.displayName = 'MyInput';
"#;
        let issues = with_js_context(clean, "MyInput.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow with displayName");
    }
}
