//! SM5 — Commented-out code
//!
//! Detects commented-out code lines.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S125"
    name: "Commented-out code should be removed"
    severity: Info
    category: CodeSmell
    language: "Go"
    params: {}

    explanation: "Commented-out code makes the codebase harder to read and maintain. Remove or uncomment if needed.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find commented lines that look like code
        let code_patterns = [
            r"//\s*(?:func\s+\w+|import\s+|return\s+|if\s+|for\s+|var\s+|const\s+|type\s+|:=\s+)",
            r"//\s*(?:\w+\s*=\s*\w+|\w+\s*\(\s*\))",
        ];

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            for pattern in &code_patterns {
                if let Ok(re) = regex::Regex::new(pattern) {
                    if re.is_match(trimmed) {
                        issues.push(Issue::new(
                            "GO_S125",
                            format!("Commented-out code should be removed: {}", &trimmed[2..].trim()),
                            Severity::Info,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Remove the commented code or uncomment it if needed"
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
    fn test_sm5_registered() {
        let rule = GO_S125Rule::new();
        assert_eq!(rule.id(), "GO_S125");
    }

    #[test]
    fn test_sm5_detects_commented_func() {
        let rule = GO_S125Rule::new();
        let smelly = r#"
// func oldFunction() {
//     return 1
// }
func newFunction() { }
"#;
        let issues = with_go_context(smelly, "test.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect commented code");
        assert_eq!(issues[0].rule_id, "GO_S125");
    }

    #[test]
    fn test_sm5_allows_normal_comment() {
        let rule = GO_S125Rule::new();
        let clean = r#"
// This is a proper comment explaining the code
func foo() { }
"#;
        let issues = with_go_context(clean, "test.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal comments");
    }
}
