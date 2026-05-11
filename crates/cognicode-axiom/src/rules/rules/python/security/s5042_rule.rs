//! S5042 — Zip bomb detection
//!
//! Detects tarfile.extractall() without member validation.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S5042"
    name: "Archive extraction should validate members to prevent zip bombs"
    severity: Critical
    category: Vulnerability
    language: "Python"
    params: {}

    explanation: "Using extractall() without validating archive members can allow zip bombs or path traversal attacks, potentially exhausting disk space or overwriting critical files.",
    clean_code: Clear,
    impacts: [Security: High, Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect tarfile.extractall without member validation
        let extractall_re = regex::Regex::new(r"extractall\s*\(").unwrap();
        let safe_members_re = regex::Regex::new(r"getmembers\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if extractall_re.is_match(line) && !safe_members_re.is_match(line) {
                issues.push(Issue::new(
                    "PY_S5042",
                    format!("tarfile.extractall() without member validation - zip bomb risk"),
                    Severity::Critical,
                    Category::Vulnerability,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Validate archive members using getmembers() before extraction, or use extract() with a validated member list."
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
    fn test_s5042_registered() {
        let rule = PY_S5042Rule::new();
        assert_eq!(rule.id(), "PY_S5042");
    }

    #[test]
    fn test_s5042_detects_unsafe_extractall() {
        let rule = PY_S5042Rule::new();
        let smelly = r#"
import tarfile
tar = tarfile.open("archive.tar")
tar.extractall("/tmp/extracted")
"#;
        let issues = with_python_context(smelly, "app.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect unsafe extractall");
        assert_eq!(issues[0].rule_id, "PY_S5042");
    }

    #[test]
    fn test_s5042_allows_safe_extraction() {
        let rule = PY_S5042Rule::new();
        let clean = r#"
import tarfile
tar = tarfile.open("archive.tar")
for member in tar.getmembers():
    tar.extract(member, "/tmp/extracted")
"#;
        let issues = with_python_context(clean, "app.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag member-validated extraction");
    }
}
