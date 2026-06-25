//! S2612 — Weak file permissions (os.chmod 777)
//!
//! Detects os.chmod with overly permissive permissions like 0o777.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2612"
    name: "File permissions should be set securely"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Setting file permissions to 0o777 (read/write/execute for all) is a security risk as it allows any user to modify or execute the file, potentially leading to privilege escalation.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Pattern to detect chmod with 0o777 or 0o666
        let re = regex::Regex::new(r"os\.chmod\s*\([^,]+,\s*0o[67][67][67]").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if re.is_match(line) {
                issues.push(Issue::new(
                    "PY_S2612",
                    format!("Overly permissive file permissions detected (0o777 or 0o666)"),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Use restrictive permissions like 0o600 for private files or 0o644 for files that need to be read."
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

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_s2612_registered() {
        let rule = PY_S2612Rule::new();
        assert_eq!(rule.id(), "PY_S2612");
    }

    #[test]
    fn test_s2612_detects_chmod_777() {
        let rule = PY_S2612Rule::new();
        let smelly = r#"
import os
os.chmod("/tmp/secret", 0o777)
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect chmod 0o777");
        assert_eq!(issues[0].rule_id, "PY_S2612");
    }

    #[test]
    fn test_s2612_allows_restrictive_permissions() {
        let rule = PY_S2612Rule::new();
        let clean = r#"
import os
os.chmod("/tmp/secret", 0o600)
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag restrictive permissions");
    }
}
