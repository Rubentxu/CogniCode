//! TEST19 — toBeTruthy vs toBe(true) ambiguity
//!
//! Detects ambiguous truthiness assertions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_TEST19"
    name: "toBeTruthy vs toBe(true) ambiguity"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "toBe(true) checks strict equality. toBeTruthy() accepts any truthy value. Using the wrong one causes confusion.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find ambiguous patterns: toBe(true/false) vs toBeTruthy/toBeFalsy
        let tobe_true = regex::Regex::new(r"\.toBe\s*\(\s*(?:true|false)\s*\)").unwrap();
        let tobe_truthy = source.contains("toBeTruthy()") || source.contains("toBeFalsy()");

        if tobe_true.find(source).is_some() && !tobe_truthy {
            // This is informational only - toBe(true) is valid but toBeTruthy() might be intended
            for cap in tobe_true.find_iter(source) {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_TEST19",
                    "Consider if toBe(true) or toBeTruthy() is more appropriate".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use toBe(true) for strict boolean, toBeTruthy() for any truthy value"
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
    fn test_test19_registered() {
        let rule = JS_TEST19Rule::new();
        assert_eq!(rule.id(), "JS_TEST19");
    }

    #[test]
    fn test_test19_detects_tobe_true() {
        let rule = JS_TEST19Rule::new();
        let smelly = r#"
expect(result).toBe(true);
"#;
        let issues = with_js_context(smelly, "test.spec.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect toBe(true)");
        assert_eq!(issues[0].rule_id, "JS_TEST19");
    }

    #[test]
    fn test_test19_allows_proper_usage() {
        let rule = JS_TEST19Rule::new();
        let clean = r#"
expect(result).toBe(true);
expect(result).toBeTruthy();
"#;
        let issues = with_js_context(clean, "test.spec.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow when both exist");
    }
}
