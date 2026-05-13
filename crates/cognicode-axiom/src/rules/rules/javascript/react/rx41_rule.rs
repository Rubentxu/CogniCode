//! RX41 — Context.Provider without value
//!
//! Detects React Context.Provider without a value prop.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX41"
    name: "Context.Provider without value prop"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "Context.Provider requires a value prop to pass data to consumers. Omitting it makes all consumers receive undefined.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find Context.Provider without value
        let provider_pattern = regex::Regex::new(r"<[A-Z]\w*\.Provider\s*(?:\/\s*>|>)").unwrap();

        for cap in provider_pattern.find_iter(source) {
            let match_end = cap.end();
            let after = &source[match_end..match_end + 100.min(source.len() - match_end)];

            // Check if value prop is provided
            let has_value = after.contains("value=") || after.contains("value =");

            if !has_value {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_RX41",
                    "Context.Provider without value prop".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Add value prop to Provider: <Context.Provider value={...}>"
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
    fn test_rx41_registered() {
        let rule = JS_RX41Rule::new();
        assert_eq!(rule.id(), "JS_RX41");
    }

    #[test]
    fn test_rx41_detects_missing_value() {
        let rule = JS_RX41Rule::new();
        let smelly = r#"
const MyContext = React.createContext();
return (
    <MyContext.Provider>
        <Child />
    </MyContext.Provider>
);
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect Provider without value");
        assert_eq!(issues[0].rule_id, "JS_RX41");
    }

    #[test]
    fn test_rx41_allows_with_value() {
        let rule = JS_RX41Rule::new();
        let clean = r#"
const MyContext = React.createContext();
return (
    <MyContext.Provider value={contextValue}>
        <Child />
    </MyContext.Provider>
);
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag Provider with value");
    }
}
