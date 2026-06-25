//! TEST15 — spyOn with original not restored
//!
//! Detects spyOn without restore/mockRestore.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST15"
    name: "spyOn without restore"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "spyOn modifies the original. Without cleanup, the spy persists and affects other tests.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find spyOn without afterEach/restore
        let has_spjon = source.contains("spyOn(");
        let has_restore = source.contains("mockRestore()") || source.contains(".mockRestore(");
        let has_after_each = source.contains("afterEach");

        if has_spjon && !has_restore && !has_after_each {
            let spjon_re = regex::Regex::new(r"spyOn\s*\(").unwrap();
            for cap in spjon_re.find_iter(source) {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST15",
                    "spyOn without cleanup may leak to other tests".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Add afterEach with mockRestore or use jest.spyOn with automatic cleanup"
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
    fn test_test15_registered() {
        let rule = JS_TEST15Rule::new();
        assert_eq!(rule.id(), "JS_TEST15");
    }

    #[test]
    fn test_test15_detects_unrestored_spy() {
        let rule = JS_TEST15Rule::new();
        let smelly = r#"
describe('tests', () => {
    it('test1', () => {
        spyOn(obj, 'method');
    });
});
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unrestored spy");
        assert_eq!(issues[0].rule_id, "JS_TEST15");
    }

    #[test]
    fn test_test15_allows_with_cleanup() {
        let rule = JS_TEST15Rule::new();
        let clean = r#"
afterEach(() => { obj.method.mockRestore(); });
describe('tests', () => {
    it('test1', () => {
        spyOn(obj, 'method');
    });
});
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow with cleanup");
    }
}
