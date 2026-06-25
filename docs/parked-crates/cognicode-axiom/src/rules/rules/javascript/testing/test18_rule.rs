//! TEST18 — fireEvent vs userEvent
//!
//! Detects fireEvent when userEvent should be used.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST18"
    name: "fireEvent used instead of userEvent"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "userEvent simulates real user behavior more accurately than fireEvent.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find fireEvent usage
        let fireevent_pattern = regex::Regex::new(r"fireEvent\.\w+\s*\(").unwrap();
        let has_userevent = source.contains("userEvent.");

        if !has_userevent {
            for cap in fireevent_pattern.find_iter(source) {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST18",
                    "fireEvent used; consider using userEvent for more realistic tests".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Replace fireEvent with userEvent for better test realism"
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
    fn test_test18_registered() {
        let rule = JS_TEST18Rule::new();
        assert_eq!(rule.id(), "JS_TEST18");
    }

    #[test]
    fn test_test18_detects_fireevent() {
        let rule = JS_TEST18Rule::new();
        let smelly = r#"
fireEvent.click(screen.getByRole('button'));
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect fireEvent");
        assert_eq!(issues[0].rule_id, "JS_TEST18");
    }

    #[test]
    fn test_test18_allows_userevent() {
        let rule = JS_TEST18Rule::new();
        let clean = r#"
userEvent.click(screen.getByRole('button'));
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow userEvent");
    }
}
