//! TEST14 — mockImplementation vs mockReturnValue
//!
//! Detects potentially incorrect mock implementation usage.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST14"
    name: "mockImplementation used when mockReturnValue may be simpler"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "mockReturnValue is simpler for constant returns. mockImplementation should return different values per call.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find mockImplementation with simple return
        let impl_pattern = regex::Regex::new(r"mockImplementation\s*\(\s*\(\s*\)\s*=>\s*(?:[^)]+)").unwrap();

        for cap in impl_pattern.find_iter(source) {
            let impl_body = cap.get(0).unwrap().as_str();
            // Check if it's just returning a constant
            let is_simple_return = impl_body.contains("return ") && !impl_body.contains("if ") && !impl_body.contains("switch ");

            if is_simple_return {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST14",
                    "mockImplementation with simple return could use mockReturnValue".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use mockReturnValue for simple constant returns"
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
    fn test_test14_registered() {
        let rule = JS_TEST14Rule::new();
        assert_eq!(rule.id(), "JS_TEST14");
    }

    #[test]
    fn test_test14_detects_simple_impl() {
        let rule = JS_TEST14Rule::new();
        let smelly = r#"
fn.mockImplementation(() => 42);
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect simple mockImplementation");
        assert_eq!(issues[0].rule_id, "JS_TEST14");
    }

    #[test]
    fn test_test14_allows_complex_impl() {
        let rule = JS_TEST14Rule::new();
        let clean = r#"
fn.mockImplementation((x) => {
    if (x > 0) return 'positive';
    return 'non-positive';
});
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow complex mockImplementation");
    }
}
