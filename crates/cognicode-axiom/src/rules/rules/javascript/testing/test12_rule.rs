//! TEST12 — expect.assertions count mismatch
//!
//! Detects mismatched expect.assertions counts.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST12"
    name: "expect.assertions count mismatch"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "expect.assertions specifies how many assertions are expected. Mismatch may indicate incomplete tests.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find expect.assertions
        let assertions_pattern = regex::Regex::new(r"expect\.assertions\s*\(\s*(\d+)\s*\)").unwrap();

        for cap in assertions_pattern.captures_iter(source) {
            if let Some(count) = cap.get(1) {
                let expected: i32 = count.as_str().parse().unwrap_or(0);
                // Count actual assertions in the test
                let test_start = cap.get(0).unwrap().end();
                let remaining = &source[test_start..test_start + 500.min(source.len() - test_start)];
                let actual_count = remaining.matches("expect(").count() as i32;

                if actual_count != expected {
                    let line_num = source[..cap.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JS_TEST12",
                        format!("expect.assertions({}) but {} assertions found", expected, actual_count),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Update expect.assertions count to match actual assertions"
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
    fn test_test12_registered() {
        let rule = JS_TEST12Rule::new();
        assert_eq!(rule.id(), "JS_TEST12");
    }

    #[test]
    fn test_test12_detects_mismatch() {
        let rule = JS_TEST12Rule::new();
        let smelly = r#"
expect.assertions(3);
expect(a).toBe(true);
expect(b).toBe(true);
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect mismatch");
        assert_eq!(issues[0].rule_id, "JS_TEST12");
    }

    #[test]
    fn test_test12_allows_matching() {
        let rule = JS_TEST12Rule::new();
        let clean = r#"
expect.assertions(2);
expect(a).toBe(true);
expect(b).toBe(true);
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow matching count");
    }
}
