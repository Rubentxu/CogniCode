//! S4 — os.Chmod with dangerous permissions
//!
//! Detects potentially dangerous file permission changes.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "GO_S2612"
    name: "Potentially dangerous file permissions (0777, 0666)"
    severity: Major
    category: Vulnerability
    language: "Go"
    params: {}

    explanation: "Setting file permissions to 0777 or 0666 is a security risk as it allows read/write access to everyone.",
    clean_code: Clear,
    impacts: [Security: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find os.Chmod with dangerous permissions
        let chmod_pattern = regex::Regex::new(r"os\.Chmod\([^,]+,\s*(0[0-7]{3})\)").unwrap();

        for cap in chmod_pattern.captures_iter(source) {
            if let Some(perms) = cap.get(1) {
                let perms_str = perms.as_str();
                // Flag 0777, 0666, 0770, 0660 (world/group writable)
                if ["0777", "0666", "0770", "0660"].contains(&perms_str) {
                    let line_num = source[..perms.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "GO_S2612",
                        format!("Potentially dangerous file permissions: {}", perms_str),
                        Severity::Major,
                        Category::Vulnerability,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use more restrictive permissions (e.g., 0644 for files, 0755 for directories)"
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
    fn test_s4_registered() {
        let rule = GO_S2612Rule::new();
        assert_eq!(rule.id(), "GO_S2612");
    }

    #[test]
    fn test_s4_detects_dangerous_perms() {
        let rule = GO_S2612Rule::new();
        let smelly = r#"
os.Chmod(file, 0777)
"#;
        let issues = with_go_context(smelly, "main.go", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect dangerous permissions");
        assert_eq!(issues[0].rule_id, "GO_S2612");
    }

    #[test]
    fn test_s4_allows_safe_perms() {
        let rule = GO_S2612Rule::new();
        let clean = r#"
os.Chmod(file, 0644)
"#;
        let issues = with_go_context(clean, "main.go", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag safe permissions");
    }
}
