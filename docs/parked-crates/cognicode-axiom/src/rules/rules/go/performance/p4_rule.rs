//! P4 — Unnecessary fmt.Sprintf("%s", x)
//!
//! Detects unnecessary fmt.Sprintf with %s.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2111"
    name: "fmt.Sprintf(\"%s\", x) can be simplified to x"
    severity: Info
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "fmt.Sprintf(\"%s\", x) is unnecessary when x is already a string.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find fmt.Sprintf with %s
        let sprintf_pattern = regex::Regex::new(r#"fmt\.Sprintf\(\s*["']%s["']\s*,"#).unwrap();

        for cap in sprintf_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S2111",
                format!("fmt.Sprintf(\"%s\", x) can be simplified to x"),
                Severity::Info,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Use the string directly instead of fmt.Sprintf"
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
    fn test_p4_registered() {
        let rule = GO_S2111Rule::new();
        assert_eq!(rule.id(), "GO_S2111");
    }

    #[test]
    fn test_p4_detects_unnecessary_sprintf() {
        let rule = GO_S2111Rule::new();
        let smelly = r#"
func main() {
    s := fmt.Sprintf("%s", name)
}
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unnecessary sprintf");
        assert_eq!(issues[0].rule_id, "GO_S2111");
    }

    #[test]
    fn test_p4_allows_normal_sprintf() {
        let rule = GO_S2111Rule::new();
        let clean = r#"
func main() {
    s := fmt.Sprintf("Hello %s", name)
}
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag sprintf with actual formatting");
    }
}
