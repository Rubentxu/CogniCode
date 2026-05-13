//! RX49 — lazy() without Suspense wrapper
//!
//! Detects React.lazy without Suspense in the component tree.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX49"
    name: "lazy() without Suspense wrapper"
    severity: Major
    category: Bug
    language: "JavaScript"
    params: {}

    explanation: "React.lazy components must be wrapped in Suspense to handle the loading state.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find React.lazy
        let lazy_pattern = regex::Regex::new(r"React\.lazy\s*\(|const\s+\w+\s*=\s*lazy\s*\(").unwrap();

        for cap in lazy_pattern.find_iter(source) {
            let after = &source[cap.end()..cap.end() + 200.min(source.len() - cap.end())];
            let has_suspense_before = source[..cap.start()].contains("<Suspense");
            let has_suspense_after = after.contains("<Suspense") || after.contains("<Suspense");

            if !has_suspense_before && !has_suspense_after {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JS_RX49",
                    "lazy component should be wrapped in Suspense".to_string(),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Wrap in <Suspense fallback={<Loading />}>"
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
    fn test_rx49_registered() {
        let rule = JS_RX49Rule::new();
        assert_eq!(rule.id(), "JS_RX49");
    }

    #[test]
    fn test_rx49_detects_lazy_without_suspense() {
        let rule = JS_RX49Rule::new();
        let smelly = r#"
const LazyComponent = React.lazy(() => import('./Heavy'));
return <LazyComponent />;
"#;
        let issues = with_js_context(smelly, "App.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect lazy without Suspense");
        assert_eq!(issues[0].rule_id, "JS_RX49");
    }

    #[test]
    fn test_rx49_allows_with_suspense() {
        let rule = JS_RX49Rule::new();
        let clean = r#"
const LazyComponent = React.lazy(() => import('./Heavy'));
return <Suspense fallback={<Loading />}><LazyComponent /></Suspense>;
"#;
        let issues = with_js_context(clean, "App.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow with Suspense");
    }
}
