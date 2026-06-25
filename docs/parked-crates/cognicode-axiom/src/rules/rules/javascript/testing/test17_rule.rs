//! TEST17 — waitFor timeout too short
//!
//! Detects waitFor with very short timeout.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST17"
    name: "waitFor with timeout < 1000ms"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "waitFor with very short timeout may cause flaky tests on slow CI environments.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find waitFor with timeout < 1000
        let waitfor_pattern = regex::Regex::new(r"waitFor\s*\([^)]*,\s*\{\s*timeout:\s*(\d+)").unwrap();

        for cap in waitfor_pattern.captures_iter(source) {
            if let Some(timeout) = cap.get(1) {
                let timeout_val: i32 = timeout.as_str().parse().unwrap_or(0);
                if timeout_val < 1000 {
                    let line_num = source[..cap.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JS_TEST17",
                        format!("waitFor timeout {}ms may be too short", timeout_val),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use timeout >= 1000ms for more reliable tests"
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
    fn test_test17_registered() {
        let rule = JS_TEST17Rule::new();
        assert_eq!(rule.id(), "JS_TEST17");
    }

    #[test]
    fn test_test17_detects_short_timeout() {
        let rule = JS_TEST17Rule::new();
        let smelly = r#"
waitFor(() => getByText('loaded'), { timeout: 100 });
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect short timeout");
        assert_eq!(issues[0].rule_id, "JS_TEST17");
    }

    #[test]
    fn test_test17_allows_sufficient_timeout() {
        let rule = JS_TEST17Rule::new();
        let clean = r#"
waitFor(() => getByText('loaded'), { timeout: 5000 });
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow sufficient timeout");
    }
}
