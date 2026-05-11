//! S3 — os/exec.Command with user input
//!
//! Detects potential command injection via exec.Command.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S1523"
    name: "Command injection vulnerability via exec.Command"
    severity: Major
    category: Vulnerability
    language: "Go"
    params: {}

    explanation: "Using exec.Command with unsanitized user input can lead to command injection attacks. Always validate and sanitize input.",
    clean_code: Clear,
    impacts: [Security: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find exec.Command usage
        let exec_pattern = regex::Regex::new(r"exec\.Command\(").unwrap();

        for cap in exec_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "GO_S1523",
                format!("Potential command injection: exec.Command usage requires input validation"),
                Severity::Major,
                Category::Vulnerability,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Validate and sanitize all input passed to exec.Command. Consider using exec.CommandContext."
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
    fn test_s3_registered() {
        let rule = GO_S1523Rule::new();
        assert_eq!(rule.id(), "GO_S1523");
    }

    #[test]
    fn test_s3_detects_exec_command() {
        let rule = GO_S1523Rule::new();
        let smelly = r#"
cmd := exec.Command("ls", "-la")
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect exec.Command usage");
        assert_eq!(issues[0].rule_id, "GO_S1523");
    }

    #[test]
    fn test_s3_allows_no_exec() {
        let rule = GO_S1523Rule::new();
        let clean = r#"
fmt.Println("Hello")
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag code without exec.Command");
    }
}
