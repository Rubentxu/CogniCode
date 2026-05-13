//! RX44 — useState initializer function call
//!
//! Detects useState(fn()) instead of useState(fn).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX44"
    name: "useState initializer should be function reference"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "useState(fn) passes fn as the initial state. useState(fn()) calls fn immediately and uses its return value. This causes extra renders.",
    clean_code: Clear,
    impacts: [Performance: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find useState with function call in initializer
        let state_pattern = regex::Regex::new(r"useState\s*\(\s*\w+\s*\(\s*\)\s*\)").unwrap();

        for cap in state_pattern.find_iter(source) {
            let match_str = cap.as_str();
            // Make sure it's actually calling a function, not just referencing
            if match_str.contains("()") && !match_str.contains("() =>") {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_RX44",
                    "useState initializer is called immediately".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Pass the function reference: useState(computeInitialState) instead of useState(computeInitialState())"
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
    fn test_rx44_registered() {
        let rule = JS_RX44Rule::new();
        assert_eq!(rule.id(), "JS_RX44");
    }

    #[test]
    fn test_rx44_detects_immediate_call() {
        let rule = JS_RX44Rule::new();
        let smelly = r#"
const [state, setState] = useState(computeInitialState());
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect immediate function call");
        assert_eq!(issues[0].rule_id, "JS_RX44");
    }

    #[test]
    fn test_rx44_allows_function_reference() {
        let rule = JS_RX44Rule::new();
        let clean = r#"
const [state, setState] = useState(computeInitialState);
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow function reference");
    }
}
