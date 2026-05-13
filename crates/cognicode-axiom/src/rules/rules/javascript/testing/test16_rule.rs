//! TEST16 — act() wrapper missing
//!
//! Detects async operations without act() wrapper.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST16"
    name: "async test without act() wrapper"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "State updates should be wrapped in act() to ensure proper batching and synchronous updates.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find async tests with setState/fireEvent without act
        let async_test = regex::Regex::new(r"(?:it|test)\s*\([^)]*,\s*(?:async\s*)?\([^)]*\)\s*=>").unwrap();
        let has_act = source.contains("act(");

        if !has_act {
            for cap in async_test.find_iter(source) {
                let after = &source[cap.end()..cap.end() + 300.min(source.len() - cap.end())];
                if after.contains("setState") || after.contains("fireEvent") || after.contains("dispatchEvent") {
                    let line_num = source[..cap.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JS_TEST16",
                        "async state update without act() wrapper".to_string(),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Wrap state updates in act(): await act(async () => { ... })"
                    )));
                    break;
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
    fn test_test16_registered() {
        let rule = JS_TEST16Rule::new();
        assert_eq!(rule.id(), "JS_TEST16");
    }

    #[test]
    fn test_test16_detects_missing_act() {
        let rule = JS_TEST16Rule::new();
        let smelly = r#"
it('updates state', async () => {
    fireEvent.click(button);
    expect(screen.getByText('updated')).toBeTruthy();
});
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect missing act");
        assert_eq!(issues[0].rule_id, "JS_TEST16");
    }

    #[test]
    fn test_test16_allows_act_wrapper() {
        let rule = JS_TEST16Rule::new();
        let clean = r#"
it('updates state', async () => {
    await act(async () => {
        fireEvent.click(button);
    });
    expect(screen.getByText('updated')).toBeTruthy();
});
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow with act");
    }
}
