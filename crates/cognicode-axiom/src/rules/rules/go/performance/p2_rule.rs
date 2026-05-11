//! P2 — for i := 0; i < len(x); i++ pattern
//!
//! Detects traditional index loops that could use range.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1736"
    name: "Use range instead of for len loop"
    severity: Minor
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Using 'for i := 0; i < len(x); i++' can be simplified to 'for i, v := range x'.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find for len patterns
        let len_pattern = regex::Regex::new(r"for\s+\w+\s*:=\s*0\s*;\s*\w+\s*<\s*len\(").unwrap();

        for cap in len_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S1736",
                format!("Use range instead of for len loop"),
                Severity:: Minor,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Consider using 'for i, v := range x' instead"
            )));
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    fn with_go_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Go.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Go,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_p2_registered() {
        let rule = GO_S1736Rule::new();
        assert_eq!(rule.id(), "GO_S1736");
    }

    #[test]
    fn test_p2_detects_len_loop() {
        let rule = GO_S1736Rule::new();
        let smelly = r#"
func main() {
    for i := 0; i < len(x); i++ {
        fmt.Println(x[i])
    }
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect len loop");
        assert_eq!(issues[0].rule_id, "GO_S1736");
    }

    #[test]
    fn test_p2_allows_range_loop() {
        let rule = GO_S1736Rule::new();
        let clean = r#"
func main() {
    for i, v := range x {
        fmt.Println(i, v)
    }
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag range loop");
    }
}
