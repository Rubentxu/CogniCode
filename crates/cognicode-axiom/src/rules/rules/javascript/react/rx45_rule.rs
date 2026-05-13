//! RX45 — Component with both state and derived values
//!
//! Detects components that compute derived values during render.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX45"
    name: "Derived value computed during render"
    severity: Minor
    category: Performance
    language: "JavaScript"
    params: {}

    explanation: "Computing derived values during every render is inefficient. Use useMemo for expensive computations.",
    clean_code: Clear,
    impacts: [Performance: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find components with expensive computations
        let expensives = ["filter(", "map(", "reduce(", "sort(", "JSON.parse", "deepClone"];

        for (idx, line) in source.lines().enumerate() {
            if line.contains("function") || line.contains("=>") {
                for exp in &expensives {
                    if line.contains(exp) && !line.contains("useMemo") && !line.contains("useCallback") {
                        let line_num = idx + 1;
                        issues.push(Issue::new(
                            "JS_RX45",
                            format!("Expensive computation '{}' in render", exp),
                            Severity::Minor,
                            Category::Performance,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Wrap in useMemo: const value = useMemo(() => expensive computation, [deps])"
                        )));
                        break;
                    }
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
    fn test_rx45_registered() {
        let rule = JS_RX45Rule::new();
        assert_eq!(rule.id(), "JS_RX45");
    }

    #[test]
    fn test_rx45_detects_expensive_in_render() {
        let rule = JS_RX45Rule::new();
        let smelly = r#"
function Component({ items }) {
    const sorted = items.filter(x => x.active).sort((a, b) => a.id - b.id);
    return <List data={sorted} />;
}
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect expensive computation");
        assert_eq!(issues[0].rule_id, "JS_RX45");
    }

    #[test]
    fn test_rx45_allows_memoized() {
        let rule = JS_RX45Rule::new();
        let clean = r#"
function Component({ items }) {
    const sorted = useMemo(() => items.filter(x => x.active).sort((a, b) => a.id - b.id), [items]);
    return <List data={sorted} />;
}
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow memoized computation");
    }
}
