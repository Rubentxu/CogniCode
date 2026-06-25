//! RX47 — useRef not used in JSX or effect
//!
//! Detects unused useRef values.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JS_RX47"
    name: "useRef result not used"
    severity: Minor
    category: CodeSmell
    language: "JavaScript"
    params: {}

    explanation: "useRef creates a mutable ref object. If .current is never read or written, the ref is unused.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find useRef and check if .current is used
        let ref_pattern = regex::Regex::new(r"const\s+(\w+)\s*=\s*useRef\s*\(").unwrap();

        for cap in ref_pattern.captures_iter(source) {
            if let Some(ref_name) = cap.get(1) {
                let ref_str = ref_name.as_str();
                let search_from = cap.get(0).unwrap().end();
                let after = &source[search_from..];

                // Check if ref.current is accessed
                let current_access = format!("{}.current", ref_str);
                if !after.contains(&current_access) {
                    let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JS_RX47",
                        format!("useRef '{}' result is never used", ref_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use the ref via ref.current or remove if unnecessary"
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
    fn test_rx47_registered() {
        let rule = JS_RX47Rule::new();
        assert_eq!(rule.id(), "JS_RX47");
    }

    #[test]
    fn test_rx47_detects_unused_ref() {
        let rule = JS_RX47Rule::new();
        let smelly = r#"
const myRef = useRef();
console.log("hello");
"#;
        let issues = with_js_context(smelly, "Component.jsx", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unused ref");
        assert_eq!(issues[0].rule_id, "JS_RX47");
    }

    #[test]
    fn test_rx47_allows_used_ref() {
        let rule = JS_RX47Rule::new();
        let clean = r#"
const myRef = useRef();
useEffect(() => { myRef.current.focus(); });
"#;
        let issues = with_js_context(clean, "Component.jsx", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow used ref");
    }
}
