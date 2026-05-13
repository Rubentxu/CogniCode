//! RX50 — createContext with undefined default
//!
//! Detects React.createContext() without default value.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX50"
    name: "createContext() with undefined default"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "createContext() without a default value makes all consumers receive undefined until a Provider is mounted.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find createContext with undefined or no argument
        let context_pattern = regex::Regex::new(r"createContext\s*\(\s*(undefined)?\s*\)").unwrap();

        for cap in context_pattern.find_iter(source) {
            let match_str = cap.as_str();
            if match_str.contains("undefined") || match_str == "createContext()" {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_RX50",
                    "createContext with undefined default".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Provide a sensible default value: createContext(defaultValue)"
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
    fn test_rx50_registered() {
        let rule = JS_RX50Rule::new();
        assert_eq!(rule.id(), "JS_RX50");
    }

    #[test]
    fn test_rx50_detects_undefined_default() {
        let rule = JS_RX50Rule::new();
        let smelly = r#"
const ThemeContext = React.createContext(undefined);
"#;
        let issues = with_js_context(smelly, "ThemeContext.js", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect undefined default");
        assert_eq!(issues[0].rule_id, "JS_RX50");
    }

    #[test]
    fn test_rx50_allows_proper_default() {
        let rule = JS_RX50Rule::new();
        let clean = r#"
const ThemeContext = React.createContext({ theme: 'light' });
"#;
        let issues = with_js_context(clean, "ThemeContext.js", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow proper default");
    }
}
