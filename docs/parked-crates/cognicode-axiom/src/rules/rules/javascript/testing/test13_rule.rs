//! TEST13 — beforeAll/afterAll in nested describe
//!
//! Detects lifecycle hooks in nested describe blocks.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST13"
    name: "beforeAll/afterAll in nested describe"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "Lifecycle hooks in nested describes can cause confusion. Keep them at the top level.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find nested describe blocks with beforeAll/afterAll
        let nested_describe = regex::Regex::new(r"describe\s*\([^)]*\)\s*\{[^}]*describe").unwrap();
        let lifecycle_hooks = regex::Regex::new(r"(beforeAll|afterAll)\s*\(").unwrap();

        if nested_describe.is_match(source) {
            for cap in lifecycle_hooks.find_iter(source) {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST13",
                    "Lifecycle hook in nested describe".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Move beforeAll/afterAll to the root describe block"
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
    fn test_test13_registered() {
        let rule = JS_TEST13Rule::new();
        assert_eq!(rule.id(), "JS_TEST13");
    }

    #[test]
    fn test_test13_detects_nested_hook() {
        let rule = JS_TEST13Rule::new();
        let smelly = r#"
describe('outer', () => {
    describe('inner', () => {
        beforeAll(() => { setup(); });
        it('test', () => {});
    });
});
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect nested lifecycle hook");
        assert_eq!(issues[0].rule_id, "JS_TEST13");
    }

    #[test]
    fn test_test13_allows_root_hook() {
        let rule = JS_TEST13Rule::new();
        let clean = r#"
describe('outer', () => {
    beforeAll(() => { setup(); });
    describe('inner', () => {
        it('test', () => {});
    });
});
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow root level hook");
    }
}
