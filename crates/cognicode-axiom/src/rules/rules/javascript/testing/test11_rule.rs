//! TEST11 — Test without describe block
//!
//! Detects test cases not wrapped in a describe block.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST11"
    name: "Test without describe block"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "Tests should be organized in describe blocks for better grouping and reporting.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find test/it calls without a describe block
        let has_describe = source.contains("describe(");
        let test_pattern = regex::Regex::new(r"\b(?:test|it)\s*\(").unwrap();

        if !has_describe {
            for cap in test_pattern.find_iter(source) {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST11",
                    "test case without describe block".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Wrap related tests in describe blocks"
                )));
                break;
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
    fn test_test11_registered() {
        let rule = JS_TEST11Rule::new();
        assert_eq!(rule.id(), "JS_TEST11");
    }

    #[test]
    fn test_test11_detects_standalone_test() {
        let rule = JS_TEST11Rule::new();
        let smelly = r#"
it('should add numbers correctly', () => {
    expect(1 + 1).toBe(2);
});
"#;
        let issues = with_js_context(smelly, "math.test.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect test without describe");
        assert_eq!(issues[0].rule_id, "JS_TEST11");
    }

    #[test]
    fn test_test11_allows_describe_block() {
        let rule = JS_TEST11Rule::new();
        let clean = r#"
describe('Math', () => {
    it('should add numbers correctly', () => {
        expect(1 + 1).toBe(2);
    });
});
"#;
        let issues = with_js_context(clean, "math.test.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow tests in describe");
    }
}
